//! TOTP (RFC 6238 over RFC 4226 HOTP, HMAC-SHA1, 6 digits, 30 s) for the enrolment
//! flow (bde-linux `docs/plank-broker.md` §16): the broker makes the key, shows it to
//! the client once, checks the user's first code locally and only then writes the
//! token to IPA as the user. Also the `otpauth://` URI and the RFC 4648 base32 the
//! authenticator apps read.

use rand::RngCore;
use ring::hmac;
use zeroize::Zeroizing;

/// IPA's own default key length (`otptoken_add`: `os.urandom(35)`), 56 base32 characters.
pub const KEY_LEN: usize = 35;
pub const DIGITS: u32 = 6;
pub const STEP_SECS: u64 = 30;

const B32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// A fresh random TOTP key.
pub fn new_key() -> Zeroizing<Vec<u8>> {
    let mut k = Zeroizing::new(vec![0u8; KEY_LEN]);
    rand::rngs::OsRng.fill_bytes(&mut k);
    k
}

/// RFC 4648 base32, upper case, no padding.
pub fn base32(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(5) * 8);
    let mut buf: u64 = 0;
    let mut bits = 0u32;
    for &b in data {
        buf = (buf << 8) | b as u64;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(B32[((buf >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(B32[((buf << (5 - bits)) & 31) as usize] as char);
    }
    out
}

/// Inverse of [`base32`] (case-insensitive, padding and spaces ignored).
pub fn base32_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 5 / 8);
    let mut buf: u64 = 0;
    let mut bits = 0u32;
    for c in s.bytes().filter(|c| *c != b'=' && *c != b' ') {
        let v = B32.iter().position(|x| *x == c.to_ascii_uppercase())? as u64;
        buf = (buf << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

/// RFC 4226 HOTP value (HMAC-SHA1, dynamic truncation) as a zero-padded string.
pub fn hotp(key: &[u8], counter: u64, digits: u32) -> String {
    let k = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, key);
    let tag = hmac::sign(&k, &counter.to_be_bytes());
    let h = tag.as_ref();
    let off = (h[h.len() - 1] & 0x0f) as usize;
    let bin = u32::from_be_bytes([h[off] & 0x7f, h[off + 1], h[off + 2], h[off + 3]]);
    let v = bin % 10u32.pow(digits);
    format!("{v:0width$}", width = digits as usize)
}

/// The TOTP code for `unix` seconds.
pub fn code_at(key: &[u8], unix: u64) -> String {
    hotp(key, unix / STEP_SECS, DIGITS)
}

/// Checks `code` against the steps `now-1`, `now`, `now+1` (±30 s clock drift, as
/// the portal does). Constant work per step; `Some(step)` of the match.
pub fn verify(key: &[u8], code: &str, unix: u64) -> Option<u64> {
    if code.len() != DIGITS as usize || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let now = unix / STEP_SECS;
    let mut hit = None;
    for step in [now.saturating_sub(1), now, now + 1] {
        let want = hotp(key, step, DIGITS);
        let diff = want.bytes().zip(code.bytes()).fold(0u8, |d, (a, b)| d | (a ^ b));
        if diff == 0 && hit.is_none() {
            hit = Some(step);
        }
    }
    hit
}

/// Percent-encoding for otpauth URI components (RFC 3986 unreserved kept).
fn pct(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

/// `otpauth://totp/<issuer>:<account>?secret=…&issuer=…&algorithm=SHA1&digits=6&period=30`
/// (Google Authenticator key URI format; IPA's portal tokens use the principal and
/// token uuid as the label instead, which is cosmetic).
pub fn otpauth_uri(issuer: &str, account: &str, secret_b32: &str) -> String {
    format!(
        "otpauth://totp/{}:{}?secret={}&issuer={}&algorithm=SHA1&digits={DIGITS}&period={STEP_SECS}",
        pct(issuer),
        pct(account),
        secret_b32,
        pct(issuer)
    )
}

/// Random RFC 4122 version 4 UUID (lower-case hex), the `ipatokenUniqueID` IPA uses.
pub fn uuid_v4() -> String {
    let mut b = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = hex::encode(b);
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

#[cfg(test)]
mod tests {
    use super::*;

    const RFC_KEY: &[u8] = b"12345678901234567890";

    #[test]
    fn rfc4226_hotp_vectors() {
        let want = ["755224", "287082", "359152", "969429", "338314", "254676", "287922", "162583", "399871", "520489"];
        for (i, w) in want.iter().enumerate() {
            assert_eq!(hotp(RFC_KEY, i as u64, 6), *w, "counter {i}");
        }
    }

    #[test]
    fn rfc6238_sha1_vectors() {
        // RFC 6238 appendix B (SHA1, 8 digits).
        for (t, w) in [
            (59u64, "94287082"),
            (1111111109, "07081804"),
            (1111111111, "14050471"),
            (1234567890, "89005924"),
            (2000000000, "69279037"),
            (20000000000, "65353130"),
        ] {
            assert_eq!(hotp(RFC_KEY, t / 30, 8), w, "t={t}");
        }
        assert_eq!(code_at(RFC_KEY, 59), "287082");
    }

    #[test]
    fn verify_window_is_one_step_each_way() {
        let t = 1_800_000_015;
        let now = code_at(RFC_KEY, t);
        let prev = code_at(RFC_KEY, t - 30);
        let next = code_at(RFC_KEY, t + 30);
        let far = code_at(RFC_KEY, t + 60);
        assert_eq!(verify(RFC_KEY, &now, t), Some(t / 30));
        assert_eq!(verify(RFC_KEY, &prev, t), Some(t / 30 - 1));
        assert_eq!(verify(RFC_KEY, &next, t), Some(t / 30 + 1));
        if far != now && far != prev && far != next {
            assert_eq!(verify(RFC_KEY, &far, t), None);
        }
        assert_eq!(verify(RFC_KEY, "12345", t), None);
        assert_eq!(verify(RFC_KEY, "12345a", t), None);
        assert_eq!(verify(RFC_KEY, "1234567", t), None);
    }

    #[test]
    fn base32_round_trip_and_rfc4648_vectors() {
        for (i, o) in
            [("", ""), ("f", "MY"), ("fo", "MZXQ"), ("foo", "MZXW6"), ("foob", "MZXW6YQ"), ("fooba", "MZXW6YTB")]
        {
            assert_eq!(base32(i.as_bytes()), o);
            assert_eq!(base32_decode(o).unwrap(), i.as_bytes());
        }
        assert_eq!(base32(b"foobar"), "MZXW6YTBOI");
        let k = new_key();
        let s = base32(&k);
        assert_eq!(s.len(), 56, "35-byte key like IPA's default");
        assert_eq!(base32_decode(&s.to_lowercase()).unwrap(), *k);
        assert!(base32_decode("MZ1").is_none());
    }

    #[test]
    fn otpauth_uri_shape() {
        let u = otpauth_uri("BDE Fernweh", "anna", "MZXW6YTBOI");
        assert_eq!(
            u,
            "otpauth://totp/BDE%20Fernweh:anna?secret=MZXW6YTBOI&issuer=BDE%20Fernweh&algorithm=SHA1&digits=6&period=30"
        );
    }

    #[test]
    fn uuid_shape() {
        let u = uuid_v4();
        assert_eq!(u.len(), 36);
        assert_eq!(&u[14..15], "4");
        assert!(matches!(&u[19..20], "8" | "9" | "a" | "b"));
        assert_ne!(u, uuid_v4());
    }
}
