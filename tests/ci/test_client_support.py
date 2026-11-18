import importlib.util
import json
import os
from pathlib import Path
import stat
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("client_support", ROOT / "scripts/diagnostics/client-support.py")
support = importlib.util.module_from_spec(spec)
spec.loader.exec_module(support)


class ClientSupportTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.root.chmod(0o700)

    def tearDown(self):
        self.temporary.cleanup()

    def test_request_is_private_bounded_and_replaces_the_previous_request(self):
        first = support.operate(self.root, "artist", "request", now=100)
        second = support.operate(self.root, "artist", "request", ttl=900, now=200)
        self.assertNotEqual(first["request_id"], second["request_id"])
        self.assertEqual(second["expires_unix"], 1100)
        files = list((self.root / support.REQUESTS).glob("*.json"))
        self.assertEqual(len(files), 1)
        self.assertEqual(files[0].stat().st_mode & 0o777, 0o600)
        self.assertEqual(support.read_json(files[0], 1024), second)
        for ttl in [0, -1, 901]:
            with self.assertRaises(ValueError):
                support.operate(self.root, "artist", "request", ttl=ttl)

    def test_operator_cannot_redirect_requests_through_a_symlink(self):
        (self.root / support.REQUESTS).symlink_to(self.root, target_is_directory=True)
        with self.assertRaises(ValueError):
            support.operate(self.root, "artist", "request")

    def test_network_requests_are_separate_and_return_scores_without_payloads(self):
        request = support.operate(self.root, "artist", "request", now=100, network=True)
        self.assertTrue((self.root / "network-requests").is_dir())
        self.assertFalse((self.root / support.REQUESTS).exists())
        directory = self.root / "network-reports"
        directory.mkdir(mode=0o700)
        prefix = support.hashlib.sha256(b"artist").hexdigest()
        value = {"received_unix":100,"client_session":"1"*64,"username":"artist","bundle":{
            "request_id":request["request_id"],"report":{"version":"1.2.44","platform":"macos","report_id":"2"*32},
            "network":{"assessment":{"score":90,"grade":"excellent"}}}}
        path=directory/(prefix+"-"+request["request_id"]+".json")
        path.write_text(json.dumps(value));path.chmod(0o600)
        result=support.operate(self.root,"artist","list",now=100,network=True)
        self.assertEqual(result[0]["network"]["score"],90)
        self.assertEqual(result[0]["log_bytes"],0)
        self.assertEqual(support.operate(self.root,"artist","list",now=100),[])

    def test_list_and_read_are_scoped_to_the_account_and_request(self):
        directory = self.root / support.REPORTS
        directory.mkdir(mode=0o700)
        for user, request_id in [("artist", "a"*32), ("artist", "b"*32), ("other", "a"*32)]:
            prefix = support.hashlib.sha256(user.encode()).hexdigest()
            value = {"received_unix": 100, "client_session": "1"*64, "username": user,
                     "bundle": {"request_id": request_id, "log_tail": "renderer reset\n",
                                "report": {"version": "1.2.39", "platform": "windows", "report_id": "2"*32}}}
            path = directory / (prefix + "-" + request_id + ".json")
            path.write_text(json.dumps(value))
            path.chmod(0o600)
        listing = support.operate(self.root, "artist", "list", now=100)
        self.assertEqual(len(listing), 2)
        self.assertNotIn("log_tail", json.dumps(listing))
        bundles = support.operate(self.root, "artist", "read", request_id="a"*32, now=100)
        self.assertEqual(len(bundles), 1)
        self.assertEqual(bundles[0]["username"], "artist")
        self.assertEqual(bundles[0]["bundle"]["log_tail"], "renderer reset\n")
        self.assertEqual(support.operate(self.root, "artist", "list", now=100+support.RETENTION+1), [])
        self.assertEqual(support.operate(self.root, "artist", "read", request_id="a"*32,
                                         now=100+support.RETENTION+1), [])

    def test_untrusted_identifiers_and_public_files_are_refused(self):
        for user in ["../../other", "$(command)", "", "artist name"]:
            with self.assertRaises(ValueError):
                support.operate(self.root, user, "request")
        with self.assertRaises(ValueError):
            support.operate(self.root, "artist", "read", request_id="../other")
        path = self.root / "public.json"
        path.write_text("{}")
        path.chmod(0o644)
        with self.assertRaises(ValueError):
            support.read_json(path, 1024)


if __name__ == "__main__":
    unittest.main()
