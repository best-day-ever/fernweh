#!/usr/bin/env python3
"""Offline behavioral tests for the opt-in live acceptance harness."""
import base64
import datetime
import importlib.util
import json
from pathlib import Path
import time
import unittest
from unittest.mock import patch

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.x509.oid import NameOID

path = Path(__file__).resolve().parents[2] / "scripts/test/live-broker-acceptance.py"
spec = importlib.util.spec_from_file_location("acceptance", path)
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)

HOST = "mac.example.org"
PIN = "ab" * 32
SECRET = "never-output-this-token"


class Broker:
    def __init__(self):
        self.signatures = set()
        self.logged_out = False
        self.claim_change = None

    def request(self, method, path, body=b"", headers=None):
        data = json.loads(body) if body else {}
        if path == "/v1/auth/start":
            self.key = serialization.load_der_public_key(base64.b64decode(data["device_key"]))
            return 200, {"state": "challenge", "conversation_id": "conversation"}
        if path == "/v1/auth/respond":
            assert data["responses"] == ["private-password", "123456"]
            return 200, {"state": "authenticated", "session_token": SECRET, "device_bound": True}
        try:
            signature = base64.b64decode(headers["X-Plank-Device-Proof"])
            timestamp = int(headers["X-Plank-Device-Time"])
            assert abs(timestamp - int(time.time())) < 60
            assert signature not in self.signatures and not self.logged_out
            message = f"plank-device-proof-v1\n{method}\n{path}\n{timestamp}\n{m.digest(SECRET.encode())}\n{m.digest(body)}"
            self.key.verify(signature, message.encode(), ec.ECDSA(hashes.SHA256()))
            self.signatures.add(signature)
        except Exception:
            return 401, {"state": "denied"}
        if path == "/v1/logout":
            self.logged_out = True
            return 200, {"state": "logged_out"}
        if path == "/v1/hosts":
            return 200, {"hosts": [{"id": HOST, "online": True, "connectable": True, "shared_desktop": True}]}
        if path == "/v1/hosts/unassigned.example.org/connect":
            return 403, {"state": "denied"}
        if path.endswith("/keepalive"):
            return 200, {"state": "ok", "expires_in": 90}
        if path.endswith("/connect"):
            now = int(time.time())
            claims = {"v": 1, "host": HOST, "cert_sha256": PIN, "subject": "fixture", "local_user": "studio",
                      "iat": now, "exp": now + 60, "nonce": "cd" * 32}
            if self.claim_change:
                claims.update(self.claim_change)
            token = "v1." + base64.urlsafe_b64encode(m.encode(claims)).decode().rstrip("=") + ".c2ln"
            return 200, {"route": "relay", "endpoint": "broker.example.org", "port": 29042,
                         "host_cert_sha256": PIN, "username": "fixture", "local_username": "studio",
                         "auth_method": "broker_capability", "gssapi_token": "", "broker_token": token}
        raise AssertionError("unexpected request")


class Host:
    def __init__(self, endpoint, port, pins, kind):
        assert (endpoint, port, pins, kind) == ("broker.example.org", 29042, [PIN], "leaf")

    def request(self, method, path, body, headers=None):
        data = json.loads(body)
        if path == "/plank/auth/start":
            assert set(data) == {"username", "broker_token"}
            return 200, {"state": "authenticated", "session_token": SECRET, "shared_desktop": True}
        assert headers == {"Authorization": "Bearer " + SECRET}
        if path == "/plank/display":
            return 200, {"generation": 2, "capture": {"id": "virtual", "width": 1920, "height": 1080,
                         "encoding_profile": {"encoding_mode": "hevc-10-420-videotoolbox"}}}
        assert path == "/plank/launch" and data["schema_version"] == 6
        return 200, {"state": "connecting", "transport_token": SECRET, "udp_port": 28989}


class Tests(unittest.TestCase):
    def runner(self, broker=None, **options):
        return m.Acceptance(broker or Broker(), HOST, "fixture", "private-password", "123456",
                            expected_route="relay", local_user="studio", denied_host="unassigned.example.org",
                            host_factory=Host, **options)

    def test_protocol_proofs_denials_admission_and_logout(self):
        broker = Broker()
        result = self.runner(broker).run()
        self.assertTrue(result["ok"], result)
        self.assertTrue(broker.logged_out)
        self.assertEqual(result["scope"], "broker_and_host_admission")
        self.assertIn("unassigned_host_denied", result["checks"])
        for secret in (SECRET, "private-password", "123456", "broker_token", "session_token"):
            self.assertNotIn(secret, json.dumps(result))

    def test_capability_audience_expiry_and_unknown_fields_fail_then_logout(self):
        for changes in ({"host": "other.example.org"}, {"cert_sha256": "00" * 32}, {"subject": "other"},
                        {"local_user": "root"}, {"exp": int(time.time()) + 61}, {"exp": 1},
                        {"unexpected": "value"}, {"v": True}, {"nonce": "bad"}):
            with self.subTest(changes=changes):
                broker = Broker()
                broker.claim_change = changes
                result = self.runner(broker).run()
                self.assertFalse(result["ok"])
                self.assertEqual(result["failed_check"], "mac_capability_contract")
                self.assertTrue(broker.logged_out)

    def test_transport_error_is_sanitized_and_session_revoked(self):
        def fail(*args, **kwargs):
            raise ValueError(SECRET)
        runner = self.runner()
        runner.host_factory = fail
        result = runner.run()
        self.assertFalse(result["ok"])
        self.assertTrue(runner.broker.logged_out)
        self.assertNotIn(SECRET, json.dumps(result))

    def test_relay_receiver_uses_leased_port_and_stdin_only(self):
        captured = {}
        class Child:
            returncode = 0
            def __init__(self, command, **kwargs):
                captured["command"] = command
                captured["environment"] = kwargs["env"]
            def communicate(self, input=None, timeout=None):
                if input is not None:
                    captured["launch"] = json.loads(input)
                return b"remote_receiver_pixels_ready=1\nremote_decode=pass disconnected=1", b""
            def poll(self):
                return 0
        with patch.object(m.subprocess, "Popen", Child), patch.object(m.socket, "gethostbyname", return_value="203.0.113.8"), \
                patch.dict(m.os.environ, {"FW_PASSWORD": "private-password", "SSLKEYLOGFILE": "/must/not/write"}):
            result = self.runner(receiver="/receiver", audio_optional=True).run()
        self.assertTrue(result["ok"], result)
        self.assertEqual(result["scope"], "broker_host_and_quic_video")
        self.assertEqual(captured["launch"]["udp_port"], 29042)
        self.assertNotIn(SECRET, " ".join(captured["command"]))
        self.assertNotIn("FW_PASSWORD", captured["environment"])
        self.assertNotIn("SSLKEYLOGFILE", captured["environment"])

    def test_totp_known_vector(self):
        self.assertEqual(m.totp(base64.b32encode(b"12345678901234567890").decode(), 59), "287082")

    def test_spki_and_leaf_pins_are_distinct(self):
        key = ec.generate_private_key(ec.SECP256R1())
        name = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, "fixture")])
        now = datetime.datetime.now(datetime.timezone.utc)
        cert = (x509.CertificateBuilder().subject_name(name).issuer_name(name).public_key(key.public_key())
                .serial_number(1).not_valid_before(now).not_valid_after(now + datetime.timedelta(days=1))
                .sign(key, hashes.SHA256())).public_bytes(serialization.Encoding.DER)
        spki = key.public_key().public_bytes(serialization.Encoding.DER, serialization.PublicFormat.SubjectPublicKeyInfo)
        m.PinnedHTTPS("fixture", 443, [m.digest(spki)]).check_certificate(cert)
        m.PinnedHTTPS("fixture", 443, [m.digest(cert)], kind="leaf").check_certificate(cert)
        with self.assertRaises(m.Failure):
            m.PinnedHTTPS("fixture", 443, [m.digest(cert)]).check_certificate(cert)

    def test_bad_pin_never_sends_credentials(self):
        class Connection:
            def __init__(self, *args, **kwargs):
                self.sock = self
            def connect(self):
                pass
            def getpeercert(self, binary_form):
                return b"wrong certificate"
            def request(self, *args, **kwargs):
                self.fail("request must not be called")
            def close(self):
                pass
        with patch.object(m.http.client, "HTTPSConnection", Connection):
            with self.assertRaisesRegex(m.Failure, "tls_pin"):
                m.PinnedHTTPS("fixture", 443, [PIN], kind="leaf").request("POST", "/auth", SECRET.encode())


if __name__ == "__main__":
    unittest.main()
