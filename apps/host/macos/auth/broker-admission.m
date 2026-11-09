// SPDX-License-Identifier: GPL-3.0-or-later
#import "broker-admission.h"
#import <Security/Security.h>
#import <CommonCrypto/CommonDigest.h>
#include <fcntl.h>
#include <sys/file.h>
#include <sys/stat.h>
#include <unistd.h>
#include <pwd.h>
#include <membership.h>
#include <errno.h>

static BOOL safeName(NSString *value, NSUInteger limit, NSString *characters) {
    return [value isKindOfClass:NSString.class] && value.length && value.length <= limit &&
        [value rangeOfCharacterFromSet:[[NSCharacterSet characterSetWithCharactersInString:characters]
            invertedSet]].location == NSNotFound;
}

static NSData *decodeURL(NSString *text) {
    if (!safeName(text, 16384, @"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_")) return nil;
    NSString *base = [[text stringByReplacingOccurrencesOfString:@"-" withString:@"+"]
        stringByReplacingOccurrencesOfString:@"_" withString:@"/"];
    if (base.length % 4 == 1) return nil;
    while (base.length % 4) base = [base stringByAppendingString:@"="];
    NSData *data = [[NSData alloc] initWithBase64EncodedString:base options:0];
    NSString *canonical = [[[data base64EncodedStringWithOptions:0]
        stringByReplacingOccurrencesOfString:@"+" withString:@"-"]
        stringByReplacingOccurrencesOfString:@"/" withString:@"_"];
    canonical = [canonical stringByReplacingOccurrencesOfString:@"=" withString:@""];
    return [canonical isEqual:text] ? data : nil;
}

static BOOL integer(id value, uint64_t *result) {
    if (![value isKindOfClass:NSNumber.class] ||
        CFGetTypeID((__bridge CFTypeRef)value) == CFBooleanGetTypeID()) return NO;
    double number = [value doubleValue];
    if (number < 0 || number > 9007199254740991.0 || number != [value unsignedLongLongValue]) return NO;
    *result = [value unsignedLongLongValue];
    return YES;
}

// flock plus write-before-admission closes concurrent requests and restarts.
// A torn/corrupt journal fails closed rather than silently discarding replays.
static BOOL consumeNonce(NSString *path, NSString *nonce, uint64_t expiry, uint64_t now) {
    int fd = open(path.fileSystemRepresentation, O_RDWR | O_CREAT | O_NOFOLLOW | O_CLOEXEC, 0600);
    if (fd < 0) return NO;
    BOOL accepted = NO;
    struct stat st;
    if (flock(fd, LOCK_EX | LOCK_NB) || fstat(fd, &st) || !S_ISREG(st.st_mode) || st.st_nlink != 1 ||
        st.st_uid != geteuid() || (st.st_mode & 0777) != 0600 || st.st_size < 0 || st.st_size > 1048576) goto done;
    {
        NSMutableData *bytes = [NSMutableData dataWithLength:(NSUInteger)st.st_size];
        size_t offset = 0;
        while (offset < bytes.length) {
            ssize_t n = pread(fd, (char *)bytes.mutableBytes + offset, bytes.length - offset, (off_t)offset);
            if (n < 0 && errno == EINTR) continue;
            if (n <= 0) goto done;
            offset += (size_t)n;
        }
        NSString *journal = [[NSString alloc] initWithData:bytes encoding:NSUTF8StringEncoding];
        if (!journal || (journal.length && ![journal hasSuffix:@"\n"])) goto done;
        NSMutableDictionary *entries = [NSMutableDictionary new];
        for (NSString *line in [journal componentsSeparatedByString:@"\n"]) {
            if (!line.length) continue;
            NSArray *parts = [line componentsSeparatedByString:@" "];
            if (parts.count != 2 || [parts[0] length] != 64 ||
                !safeName(parts[0], 64, @"0123456789abcdef") ||
                !safeName(parts[1], 16, @"0123456789")) goto done;
            uint64_t exp = [parts[1] longLongValue];
            if (!exp || exp > 9007199254740991ULL) goto done;
            if (exp >= now || now - exp <= 5) entries[parts[0]] = @(exp);
        }
        if (entries[nonce] || entries.count >= 4096) goto done;
        // Never rewrite live records: a crash must not erase a previously
        // accepted nonce. Truncate only when every earlier admission expired.
        if (!entries.count) {
            if (ftruncate(fd, 0) || fsync(fd)) goto done;
            st.st_size = 0;
        }
        NSData *updated = [[NSString stringWithFormat:@"%@ %llu\n", nonce,
            (unsigned long long)expiry] dataUsingEncoding:NSASCIIStringEncoding];
        if (st.st_size + updated.length > 1048576) goto done;
        offset = 0;
        while (offset < updated.length) {
            ssize_t n = pwrite(fd, (const char *)updated.bytes + offset, updated.length - offset,
                st.st_size + (off_t)offset);
            if (n < 0 && errno == EINTR) continue;
            if (n <= 0) goto done;
            offset += (size_t)n;
        }
        if (fsync(fd)) goto done;
        accepted = YES;
    }
done:
    close(fd);
    return accepted;
}

@implementation PLANKMacBrokerAdmission {
    SecKeyRef _key;
    NSString *_host, *_localUser, *_digest, *_replayPath;
}
- (instancetype)initWithPublicKey:(NSData *)publicKey host:(NSString *)host
                       localUser:(NSString *)localUser certificateDER:(NSData *)certificateDER
                      replayPath:(NSString *)replayPath {
    if (publicKey.length != 65 || ((const uint8_t *)publicKey.bytes)[0] != 4 ||
        !safeName(host, 253, @"abcdefghijklmnopqrstuvwxyz0123456789.-") ||
        !safeName(localUser, 64, @"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-") ||
        !certificateDER.length || certificateDER.length > 65536 || !replayPath.isAbsolutePath) return nil;
    self = [super init];
    if (!self) return nil;
    _key = SecKeyCreateWithData((__bridge CFDataRef)publicKey, (__bridge CFDictionaryRef)@{
        (__bridge id)kSecAttrKeyType:(__bridge id)kSecAttrKeyTypeECSECPrimeRandom,
        (__bridge id)kSecAttrKeyClass:(__bridge id)kSecAttrKeyClassPublic,
        (__bridge id)kSecAttrKeySizeInBits:@256}, NULL);
    if (!_key || !SecKeyIsAlgorithmSupported(_key, kSecKeyOperationTypeVerify,
        kSecKeyAlgorithmECDSASignatureMessageX962SHA256)) return nil;
    unsigned char digest[CC_SHA256_DIGEST_LENGTH];
    CC_SHA256(certificateDER.bytes, (CC_LONG)certificateDER.length, digest);
    NSMutableString *hex = [NSMutableString new];
    for (size_t i = 0; i < sizeof(digest); ++i) [hex appendFormat:@"%02x", digest[i]];
    _digest = hex; _host = [host copy]; _localUser = [localUser copy]; _replayPath = [replayPath copy];
    return self;
}
- (void)dealloc { if (_key) CFRelease(_key); }
- (BOOL)verifyToken:(NSString *)token subject:(NSString *)subject now:(uint64_t)now
           identity:(PLANKMacAccountIdentity *)identity {
    if (identity) memset(identity, 0, sizeof(*identity));
    if (!identity || ![token isKindOfClass:NSString.class] || token.length > 16384 || !now ||
        !safeName(subject, 255, @"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-@")) return NO;
    NSArray<NSString *> *parts = [token componentsSeparatedByString:@"."];
    if (parts.count != 3 || ![parts[0] isEqual:@"v1"]) return NO;
    NSData *payload = decodeURL(parts[1]), *signature = decodeURL(parts[2]);
    if (!payload.length || payload.length > 8192 || signature.length < 64 || signature.length > 80) return NO;
    NSData *message = [[NSString stringWithFormat:@"v1.%@", parts[1]] dataUsingEncoding:NSASCIIStringEncoding];
    if (!SecKeyVerifySignature(_key, kSecKeyAlgorithmECDSASignatureMessageX962SHA256,
        (__bridge CFDataRef)message, (__bridge CFDataRef)signature, NULL)) return NO;
    id claims = [NSJSONSerialization JSONObjectWithData:payload options:0 error:NULL];
    if (![claims isKindOfClass:NSDictionary.class] || [claims count] != 8) return NO;
    uint64_t version = 0, issued = 0, expires = 0;
    if (!integer(claims[@"v"], &version) || version != 1 ||
        !integer(claims[@"iat"], &issued) || !integer(claims[@"exp"], &expires) ||
        expires <= issued || expires - issued > 60 || issued > now + 5 ||
        (expires < now && now - expires > 5) ||
        ![claims[@"host"] isEqual:_host] || ![claims[@"cert_sha256"] isEqual:_digest] ||
        ![claims[@"subject"] isEqual:subject] || ![claims[@"local_user"] isEqual:_localUser] ||
        !safeName(claims[@"nonce"], 64, @"0123456789abcdef") || [claims[@"nonce"] length] != 64) return NO;
    // Resolve only the administrator-selected LOCAL record. The signed broker
    // subject is an audit identity, never an OS username/UID supplied by a peer.
    struct passwd record, *found = NULL;
    char buffer[16384];
    if (getpwnam_r(_localUser.UTF8String, &record, buffer, sizeof(buffer), &found) || !found ||
        !record.pw_uid || record.pw_uid != geteuid()) return NO;
    PLANKMacAccountIdentity account = {.uid = record.pw_uid};
    if (mbr_uid_to_uuid(record.pw_uid, account.uuid) || !plank_macos_account_identity_valid(account)) return NO;
    if (!consumeNonce(_replayPath, claims[@"nonce"], expires, now)) return NO;
    *identity = account;
    return YES;
}
@end
