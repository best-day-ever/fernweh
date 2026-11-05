#!/usr/bin/env python3
"""Play an eight-second generated stereo tone during an authorized live probe.

Usage: play-acceptance-tone.py SSH_TARGET -- CONTROL_COMMAND [ARGS...]
The control command must accept its password on stdin and emit the acceptance
controller's decoded-pixels readiness line. It should use --single --audio-energy
--single-seconds 12. Playback is refused unless the target's default output is
PLANK Output. No remote volume or route settings are changed.
"""
import getpass
import json
import math
from pathlib import Path
import re
import resource
import struct
import subprocess
import sys
import tempfile
import time
import wave


def main():
    if len(sys.argv) < 4 or sys.argv[2] != '--':
        raise ValueError(__doc__)
    target = sys.argv[1]
    if target.startswith('-') or not re.fullmatch(r'[A-Za-z0-9_.@:-]+', target):
        raise ValueError('Invalid SSH target')
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    password = getpass.getpass()
    remote = None
    child = None
    with tempfile.TemporaryDirectory(prefix='plank-tone-source-') as temporary:
        source = Path(temporary) / 'generated-tone.wav'
        with wave.open(str(source), 'wb') as audio:
            audio.setnchannels(2); audio.setsampwidth(2); audio.setframerate(48000)
            audio.writeframes(b''.join(struct.pack('<hh',
                int(3276 * math.sin(2 * math.pi * 440 * i / 48000)),
                int(3276 * math.sin(2 * math.pi * 880 * i / 48000))) for i in range(48000 * 8)))
        try:
            remote = subprocess.check_output(['ssh', target, 'mktemp /tmp/plank-acceptance-tone.XXXXXX'], text=True).strip()
            if not re.fullmatch(r'/tmp/plank-acceptance-tone\.[A-Za-z0-9]+', remote):
                remote = None
                raise RuntimeError('Unexpected remote temporary path')
            subprocess.run(['scp', '-q', str(source), target + ':' + remote], check=True)
            child = subprocess.Popen(sys.argv[3:], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
            child.stdin.write(password + '\n'); child.stdin.close(); password = None
            played = False
            for line in child.stdout:
                print(line, end='', flush=True)
                if 'decoded pixels ready' in line and not played:
                    output = []
                    for _ in range(10):
                        devices = json.loads(subprocess.check_output(['ssh', target,
                            'system_profiler SPAudioDataType -json'], text=True))
                        output = [d['_name'] for group in devices['SPAudioDataType']
                            for d in group.get('_items', [])
                            if d.get('coreaudio_default_audio_output_device') == 'spaudio_yes']
                        if output == ['PLANK Output']:
                            break
                        time.sleep(.1)
                    if output != ['PLANK Output']:
                        raise RuntimeError('Default output was not PLANK Output; refusing tone')
                    print('synthetic_tone route=PLANK_Output seconds=8 left_hz=440 right_hz=880', flush=True)
                    subprocess.run(['ssh', target, 'afplay ' + remote], check=True, timeout=12)
                    played = True
            if not played or child.wait(timeout=10):
                raise RuntimeError('Tone playback or receiver gate failed')
        finally:
            password = None
            if child and child.poll() is None:
                child.terminate(); child.wait(timeout=10)
            if remote:
                subprocess.run(['ssh', target, 'rm -- ' + remote], check=True)
    print('tone_source_cleanup=pass')


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(f'tone_acceptance_failed: {error}', file=sys.stderr)
        raise SystemExit(1)
