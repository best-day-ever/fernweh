#!/usr/bin/env python3
"""Check the broker boundary in an unstripped native developer executable."""
import argparse
from pathlib import Path
import subprocess
import sys

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("executable", type=Path)
parser.add_argument("--broker", choices=("0", "1"), required=True)
args = parser.parse_args()
try:
    result = subprocess.run(["nm", "-C", str(args.executable)], capture_output=True, text=True)
except OSError:
    sys.exit("Client broker profile check requires the native nm tool")
if result.returncode:
    sys.exit("Cannot inspect the Client symbol table")

names = ("RemoteBroker", "PlankBrokerClient", "OnboardingController", "FernwehUpdater",
         "BrokerSessionStore", "BrokerDeviceKey", "PlankPasskeyHelper", "BetaDiagnostics")
present = {name for name in names if name in result.stdout}
if args.broker == "0" and present:
    sys.exit("Disabled Client still contains broker backend symbols")
if args.broker == "1" and not set(names[:3]).issubset(present):
    sys.exit("Enabled Client is missing its broker implementation")
print(f"client_broker_profile=pass enabled={args.broker}")
