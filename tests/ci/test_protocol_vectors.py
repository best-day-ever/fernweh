"""Shared protocol vectors that the Linux Host keeps a byte-identical copy of."""
from pathlib import Path
import json
import unittest

ROOT = Path(__file__).resolve().parents[2]
HOST_FIXTURES = ROOT/'apps/host/linux/tests/fixtures/protocol'
# The host unit tests read their own copy; the client and the protocol
# document use tests/protocol. Both must describe the same contract.
HOST_COPIES = ('display-arrangement-v1.json', 'output-topology-v13-arrangement.json')


class HostProtocolVectorTests(unittest.TestCase):
    def test_host_copies_are_byte_identical(self):
        if not HOST_FIXTURES.is_dir():
            self.skipTest('apps/host/linux is not checked out')
        for name in HOST_COPIES:
            with self.subTest(name=name):
                self.assertEqual((ROOT/'tests/protocol'/name).read_bytes(),
                                 (HOST_FIXTURES/name).read_bytes())

    def test_arrangement_vectors_share_one_capability_fixture(self):
        vectors = json.loads((ROOT/'tests/protocol/display-arrangement-v1.json').read_text())
        topology = json.loads((ROOT/'tests/protocol/output-topology-v13-arrangement.json').read_text())
        self.assertEqual(vectors['feature'], 0x8000000)
        self.assertEqual(topology['feature_flags'] & 0x8000000, 0x8000000)
        fleet = dict(vectors['capabilities']['fleet-hybrid'])
        published = dict(topology['display_capabilities'])
        self.assertEqual(fleet, published)


if __name__ == '__main__':
    unittest.main()
