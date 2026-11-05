#!/usr/bin/env python3
"""Live, opt-in broker/Mac admission qualification. Requires Python cryptography.

Credentials arrive on stdin as JSON or through FW_USER/FW_PASSWORD/FW_OTP
or FW_TOTP_SECRET. No secrets are written to disk or included in output.
"""
import argparse
import base64
import hashlib
import hmac
import http.client
import json
import os
import re
import resource
import socket
import ssl
import struct
import subprocess
import sys
import time
from urllib.parse import urlsplit

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec


class Failure(Exception):
    """Only fixed, non-secret check names may reach the report."""


def encode(value):
    return json.dumps(value, separators=(",", ":")).encode()


def digest(value):
    return hashlib.sha256(value).hexdigest()


def totp(secret, now=None):
    secret = secret.strip().upper()
    key = base64.b32decode(secret + "=" * (-len(secret) % 8))
    mac = hmac.new(key, struct.pack(">Q", int(now if now is not None else time.time()) // 30), hashlib.sha1).digest()
    offset = mac[-1] & 15
    return f"{(struct.unpack('>I', mac[offset:offset + 4])[0] & 0x7fffffff) % 1000000:06d}"


class Device:
    def __init__(self):
        self.key = ec.generate_private_key(ec.SECP256R1())

    def public(self):
        return base64.b64encode(self.key.public_key().public_bytes(
            serialization.Encoding.DER, serialization.PublicFormat.SubjectPublicKeyInfo)).decode()

    def headers(self, token, method, path, body, now=None):
        timestamp = int(time.time()) if now is None else now
        message = f"plank-device-proof-v1\n{method}\n{path}\n{timestamp}\n{digest(token.encode())}\n{digest(body)}"
        signature = self.key.sign(message.encode(), ec.ECDSA(hashes.SHA256()))
        return {"Authorization": "Bearer " + token, "X-Plank-Device-Time": str(timestamp),
                "X-Plank-Device-Proof": base64.b64encode(signature).decode()}


class PinnedHTTPS:
    """Pin the peer before request() can send headers/body. No proxy or redirects."""
    def __init__(self, hostname, port, pins, kind="spki", timeout=15):
        self.hostname, self.port, self.pins = hostname, port, set(pins)
        self.kind, self.timeout = kind, timeout

    def check_certificate(self, der):
        material = der
        if self.kind == "spki":
            material = x509.load_der_x509_certificate(der).public_key().public_bytes(
                serialization.Encoding.DER, serialization.PublicFormat.SubjectPublicKeyInfo)
        if digest(material) not in self.pins:
            raise Failure("tls_pin")

    def request(self, method, path, body=b"", headers=None):
        # Pinning deliberately replaces CA/name validation for these private
        # endpoints. SSLContext() does not enable SSLKEYLOGFILE from the environment.
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        context.check_hostname = False
        context.verify_mode = ssl.CERT_NONE
        context.minimum_version = ssl.TLSVersion.TLSv1_3
        context.maximum_version = ssl.TLSVersion.TLSv1_3
        connection = http.client.HTTPSConnection(self.hostname, self.port, timeout=self.timeout, context=context)
        try:
            connection.connect()
            self.check_certificate(connection.sock.getpeercert(binary_form=True))
            request_headers = {"Content-Type": "application/json", "Connection": "close"}
            request_headers.update(headers or {})
            connection.request(method, path, body=body, headers=request_headers)
            response = connection.getresponse()
            raw = response.read(1024 * 1024 + 1)
            if len(raw) > 1024 * 1024:
                raise Failure("response_size")
            value = json.loads(raw) if raw else {}
            if not isinstance(value, dict):
                raise Failure("response_shape")
            return response.status, value
        except Failure:
            raise
        except Exception:
            # HTTP, TLS and parsing errors can contain response/credential bytes.
            raise Failure("transport") from None
        finally:
            connection.close()


class Acceptance:
    def __init__(self, broker, host, username, password, otp, expected_route=None,
                 local_user=None, denied_host=None, host_factory=PinnedHTTPS,
                 receiver=None, seconds=8, audio_optional=False):
        self.broker, self.host = broker, host
        self.username, self.password, self.otp = username, password, otp
        self.expected_route, self.local_user, self.denied_host = expected_route, local_user, denied_host
        self.host_factory = host_factory
        self.receiver, self.seconds, self.audio_optional = receiver, seconds, audio_optional
        self.device, self.token = Device(), None
        self.results = {"ok": False, "scope": "broker_and_host_admission", "checks": []}

    def require(self, name, condition):
        if not condition:
            raise Failure(name)
        self.results["checks"].append(name)

    def call(self, method, path, body=None, headers=None):
        raw = b"" if body is None else encode(body)
        if headers is None:
            headers = self.device.headers(self.token, method, path, raw)
        return self.broker.request(method, path, raw, headers)

    def run(self):
        try:
            self.exercise()
            self.results["ok"] = True
        except Failure as error:
            self.results["failed_check"] = str(error)
        except Exception:
            self.results["failed_check"] = "unexpected_error"
        finally:
            # Always attempt revocation once a session exists, even after a failed check.
            if self.token:
                try:
                    status, reply = self.call("POST", "/v1/logout", {})
                    self.require("logout", status == 200 and reply.get("state") == "logged_out")
                    status, _ = self.call("GET", "/v1/hosts")
                    self.require("logged_out_session_denied", status == 401)
                except Exception:
                    self.results["ok"] = False
                    self.results["cleanup_failed"] = True
            self.password = self.otp = self.token = None
        return self.results

    def exercise(self):
        status, challenge = self.broker.request("POST", "/v1/auth/start", encode({
            "username": self.username, "method": "password_otp", "device_key": self.device.public()}))
        self.require("auth_challenge", status == 200 and challenge.get("state") == "challenge"
                     and isinstance(challenge.get("conversation_id"), str))
        status, reply = self.broker.request("POST", "/v1/auth/respond", encode({
            "conversation_id": challenge["conversation_id"],
            "responses": [self.password, self.otp() if callable(self.otp) else self.otp]}))
        # Retain any returned token before assertions so even malformed replies get cleanup.
        self.token = reply.get("session_token") if isinstance(reply.get("session_token"), str) else None
        self.password = self.otp = None
        self.require("mfa_device_bound", status == 200 and reply.get("state") == "authenticated"
                     and reply.get("device_bound") is True and bool(self.token))
        status, _ = self.call("GET", "/v1/hosts", headers={"Authorization": "Bearer " + self.token})
        self.require("missing_device_proof_denied", status == 401)
        status, _ = self.call("GET", "/v1/hosts", headers=Device().headers(self.token, "GET", "/v1/hosts", b""))
        self.require("wrong_device_denied", status == 401)
        status, _ = self.call("GET", "/v1/hosts", headers=self.device.headers(
            self.token, "GET", "/v1/hosts", b"", int(time.time()) - 600))
        self.require("stale_device_proof_denied", status == 401)
        proof = self.device.headers(self.token, "GET", "/v1/hosts", b"")
        status, hosts = self.call("GET", "/v1/hosts", headers=proof)
        self.require("host_list", status == 200 and isinstance(hosts.get("hosts"), list))
        entries = [h for h in hosts["hosts"] if h.get("id") == self.host]
        self.require("shared_host_available", len(entries) == 1 and entries[0].get("online") is True
                     and entries[0].get("connectable") is True and entries[0].get("shared_desktop") is True)
        status, _ = self.call("GET", "/v1/hosts", headers=proof)
        self.require("replayed_device_proof_denied", status == 401)
        if self.denied_host:
            status, _ = self.call("POST", f"/v1/hosts/{self.denied_host}/connect", {})
            self.require("unassigned_host_denied", status == 403)
        path = f"/v1/hosts/{self.host}"
        status, connect = self.call("POST", path + "/connect", {})
        self.require("mac_connect", status == 200 and connect.get("auth_method") == "broker_capability"
                     and connect.get("gssapi_token") == "" and connect.get("username") == self.username
                     and isinstance(connect.get("local_username"), str))
        route = connect.get("route")
        self.require("route", route in ("direct", "relay") and
                     (self.expected_route is None or route == self.expected_route))
        self.results["route"] = route
        self.require("endpoint", isinstance(connect.get("endpoint"), str) and bool(connect["endpoint"])
                     and type(connect.get("port")) is int and 1 <= connect["port"] <= 65535)
        certificate = connect.get("host_cert_sha256", "")
        self.require("host_certificate_pin", isinstance(certificate, str)
                     and re.fullmatch("[0-9a-f]{64}", certificate) is not None)
        capability = connect.get("broker_token", "")
        self.validate_capability(capability, certificate, connect["local_username"])
        target = self.host_factory(connect["endpoint"], connect["port"], [certificate], kind="leaf")
        status, admission = target.request("POST", "/plank/auth/start", encode({
            "username": self.username, "broker_token": capability}))
        self.require("host_signed_admission", status == 200 and admission.get("state") == "authenticated"
                     and admission.get("shared_desktop") is True
                     and isinstance(admission.get("session_token"), str) and bool(admission["session_token"]))
        status, renewal = self.call("POST", path + "/keepalive", {})
        self.require("keepalive", status == 200 and renewal.get("state") == "ok"
                     and type(renewal.get("expires_in")) is int and renewal["expires_in"] > 0)
        if self.receiver:
            self.stream(target, admission["session_token"], connect, path)

    def stream(self, target, token, connect, broker_path):
        headers = {"Authorization": "Bearer " + token}
        status, topology = target.request("POST", "/plank/display", encode({
            "schema_version": 3, "width": 1920, "height": 1080, "scale": 1,
            "encoding_mode": "hevc-10-420-videotoolbox"}), headers)
        self.require("virtual_display", status == 200 and isinstance(topology.get("capture"), dict))
        capture = topology["capture"]
        status, launch = target.request("POST", "/plank/launch", encode({
            "schema_version": 6, "clipboard": False, "microphone": False, "camera": False,
            "capture_generation": topology["generation"], "capture_id": capture["id"],
            "width": capture["width"], "height": capture["height"],
            "encoding_mode": capture["encoding_profile"]["encoding_mode"],
            "frame_rate": 60, "bitrate_kbps": 20000, "max_udp_payload_size": 1200}), headers)
        self.require("stream_launch", status == 200 and launch.get("state") == "connecting"
                     and isinstance(launch.get("transport_token"), str))
        if connect["route"] == "relay":
            launch["udp_port"] = connect["port"]
        address = socket.gethostbyname(connect["endpoint"])
        command = [self.receiver, connect["host_cert_sha256"], "--remote", address, str(self.seconds)]
        if self.audio_optional:
            command.append("--audio-optional")
        environment = {k: v for k, v in os.environ.items() if not k.startswith("FW_") and k != "SSLKEYLOGFILE"}
        child = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, env=environment)
        try:
            data = encode(launch)
            deadline = time.monotonic() + self.seconds + 35
            while True:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise Failure("receiver_timeout")
                try:
                    stdout, _ = child.communicate(input=data, timeout=min(25, remaining))
                    break
                except subprocess.TimeoutExpired:
                    data = None
                    status, reply = self.call("POST", broker_path + "/keepalive", {})
                    self.require("stream_keepalive", status == 200 and reply.get("state") == "ok")
            self.require("quic_decoded_pixels", child.returncode == 0 and
                         b"remote_receiver_pixels_ready=1" in stdout and b"remote_decode=pass" in stdout)
            self.require("stream_disconnected", b"disconnected=1" in stdout)
            self.results["scope"] = "broker_host_and_quic_video"
            self.results["audio_required"] = not self.audio_optional
        finally:
            if child.poll() is None:
                child.terminate()
            try:
                child.communicate(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.communicate()

    def validate_capability(self, token, certificate, local_user):
        try:
            version, encoded, signature = token.split(".")
            if version != "v1" or not re.fullmatch(r"[A-Za-z0-9_-]+", encoded) or not re.fullmatch(r"[A-Za-z0-9_-]+", signature):
                raise ValueError()
            claims = json.loads(base64.urlsafe_b64decode(encoded + "=" * (-len(encoded) % 4)))
            fields = {"v", "host", "cert_sha256", "subject", "local_user", "iat", "exp", "nonce"}
            valid = set(claims) == fields and type(claims["v"]) is int and claims["v"] == 1
            valid = valid and claims["host"] == self.host and claims["subject"] == self.username
            valid = valid and claims["cert_sha256"] == certificate and claims["local_user"] == local_user
            valid = valid and (self.local_user is None or local_user == self.local_user)
            valid = valid and type(claims["iat"]) is int and type(claims["exp"]) is int
            valid = valid and 0 < claims["exp"] - claims["iat"] <= 60
            valid = valid and abs(claims["iat"] - int(time.time())) <= 60 and claims["exp"] >= int(time.time())
            valid = valid and re.fullmatch("[0-9a-f]{64}", claims["nonce"]) is not None
        except Exception:
            valid = False
        # This checks the envelope. The pinned Host, with its configured public
        # key, performs the actual ECDSA verification in host_signed_admission.
        self.require("mac_capability_contract", valid)


def hex_pin(value):
    if not re.fullmatch("[0-9a-fA-F]{64}", value):
        raise argparse.ArgumentTypeError("expected SHA256 hex")
    return value.lower()


def hostname(value):
    if not re.fullmatch(r"[a-z0-9][a-z0-9.-]{0,252}", value) or ".." in value:
        raise argparse.ArgumentTypeError("expected lower-case host name")
    return value


def main():
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--broker", required=True, help="HTTPS broker origin, including port")
    parser.add_argument("--broker-pin", required=True, action="append", type=hex_pin, help="trusted broker SPKI SHA256 hex")
    parser.add_argument("--host", required=True, type=hostname)
    parser.add_argument("--expect-route", choices=("direct", "relay"))
    parser.add_argument("--local-user", help="expected configured shared local user")
    parser.add_argument("--denied-host", type=hostname, help="host deliberately not assigned to fixture")
    parser.add_argument("--credentials-env", action="store_true", help="use FW_* environment instead of JSON stdin")
    parser.add_argument("--receiver", help="path to preview-receive-live: enables virtual display and actual QUIC decoding")
    parser.add_argument("--seconds", type=int, default=8, choices=range(1, 121), metavar="1..120")
    parser.add_argument("--audio-optional", action="store_true", help="allow unavailable Host audio; video remains required")
    args = parser.parse_args()
    try:
        url = urlsplit(args.broker)
        if url.scheme != "https" or not url.hostname or url.username or url.password or url.path not in ("", "/") or url.query or url.fragment:
            raise Failure("broker_origin")
        if args.credentials_env:
            credentials = {k: os.environ.pop(v, "") for k, v in {
                "username": "FW_USER", "password": "FW_PASSWORD", "otp": "FW_OTP", "totp_secret": "FW_TOTP_SECRET"}.items()}
        else:
            raw = sys.stdin.buffer.read(16385)
            if len(raw) > 16384:
                raise Failure("credentials_size")
            credentials = json.loads(raw)
        user, password = credentials["username"], credentials["password"]
        otp = credentials.get("otp")
        if not otp:
            seed = credentials["totp_secret"]
            totp(seed)  # Validate the seed before beginning an auth conversation.
            otp = lambda seed=seed: totp(seed)
            seed = None
        if not isinstance(user, str) or not user or not isinstance(password, str) or not password or \
                (not callable(otp) and (not isinstance(otp, str) or not re.fullmatch(r"\d{6}", otp))):
            raise Failure("credentials_shape")
        credentials.clear()
        runner = Acceptance(PinnedHTTPS(url.hostname, url.port or 443, args.broker_pin), args.host,
                            user, password, otp, args.expect_route, args.local_user, args.denied_host,
                            receiver=args.receiver, seconds=args.seconds, audio_optional=args.audio_optional)
        password = otp = None
        result = runner.run()
    except Failure as error:
        result = {"ok": False, "failed_check": str(error)}
    except Exception:
        result = {"ok": False, "failed_check": "invalid_input"}
    print(json.dumps(result, sort_keys=True))
    return 0 if result["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
