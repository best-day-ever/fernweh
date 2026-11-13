"""Cross-runtime compatibility of the independently built diagnostic collectors."""
import json
from pathlib import Path
import unittest


class DiagnosticContractTests(unittest.TestCase):
    def test_client_vocabulary_matches_broker_contract(self):
        root = Path(__file__).resolve().parents[2]
        broker = json.loads((root / 'protocol/fixtures/client-lifecycle-vocabulary.json').read_text())
        client = json.loads((root / 'apps/client/tests/betadiagnostics/lifecycle-vocabulary.json').read_text())
        self.assertEqual(client, broker)
