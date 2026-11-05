#!/usr/bin/env python3
"""Play an eight-second generated stereo tone during an authorized live probe.

Usage: play-acceptance-tone.py SSH_TARGET -- CONTROL_COMMAND [ARGS...]
The control command must accept its password on stdin and emit the acceptance
controller's decoded-pixels readiness line. An optional initial
--explicit-generator PATH uses the native play-output-tone binary, which binds
to PLANK Output without reading or changing the default device. It should use --single --audio-energy
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
    arguments = sys.argv[1:]
    generator = None
    if arguments and arguments[0] == '--explicit-generator':
        if len(arguments) < 3:
            raise ValueError(__doc__)
        generator = Path(arguments[1]).resolve()
        if not generator.is_file():
            raise ValueError('Missing explicit device tone generator')
        arguments = arguments[2:]
    if len(arguments) < 3 or arguments[1] != '--':
        raise ValueError(__doc__)
    target = arguments[0]
    if target.startswith('-') or not re.fullmatch(r'[A-Za-z0-9_.@:-]+', target):
        raise ValueError('Invalid SSH target')
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    password = getpass.getpass()
    remote = None
    child = None
    with tempfile.TemporaryDirectory(prefix='plank-tone-source-') as temporary:
        source = Path(temporary) / 'generated-tone.wav'
        if not generator:
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
            subprocess.run(['scp', '-q', str(generator or source), target + ':' + remote], check=True)
            if generator:
                subprocess.run(['ssh', target, 'chmod 700 ' + remote], check=True)
            child = subprocess.Popen(arguments[2:], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
            child.stdin.write(password + '\n'); child.stdin.close(); password = None
            played = False
            for line in child.stdout:
                print(line, end='', flush=True)
                if 'decoded pixels ready' in line and not played:
                    output = []
                    for _ in range(0 if generator else 10):
                        devices = json.loads(subprocess.check_output(['ssh', target,
                            'system_profiler SPAudioDataType -json'], text=True))
                        output = [d['_name'] for group in devices['SPAudioDataType']
                            for d in group.get('_items', [])
                            if d.get('coreaudio_default_audio_output_device') == 'spaudio_yes']
                        if output == ['PLANK Output']:
                            break
                        time.sleep(.1)
                    if not generator and output != ['PLANK Output']:
                        raise RuntimeError('Default output was not PLANK Output; refusing tone')
                    print('synthetic_tone route=PLANK_Output seconds=8 left_hz=440 right_hz=880 explicit_device=' + str(bool(generator)).lower(), flush=True)
                    subprocess.run(['ssh', target, remote if generator else 'afplay ' + remote], check=True, timeout=12)
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
