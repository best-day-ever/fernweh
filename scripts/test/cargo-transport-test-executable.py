#!/usr/bin/env python3
"""Select the exact transport unit-test artifact from Cargo JSON, not cache age."""
import json
from pathlib import Path
import sys

def select_executable(lines):
    candidates = set()
    for line in lines:
        message = json.loads(line)
        if (message.get('reason') == 'compiler-artifact'
                and message.get('target', {}).get('name') == 'plank_transport'
                and message.get('profile', {}).get('test') is True
                and message.get('executable')):
            executable = message['executable']
            if not Path(executable).is_absolute():
                raise ValueError('Cargo test executable must be an absolute path')
            candidates.add(executable)
    if len(candidates) != 1:
        raise ValueError('Expected exactly one Cargo-reported transport test executable')
    return candidates.pop()

if __name__ == '__main__':
    try:
        print(select_executable(sys.stdin))
    except (ValueError, json.JSONDecodeError) as error:
        raise SystemExit(str(error))
