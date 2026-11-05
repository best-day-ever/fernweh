#!/usr/bin/env python3
"""Independent shared-desktop streams over actual TLS and QUIC, synthetic devices."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--server', required=True)
parser.add_argument('--receiver', required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('fixture', root/'tests/auth/macos-https-auth.py')
fixture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixture)
with tempfile.TemporaryDirectory(prefix='plank-shared-desktop-') as directory:
    cert = fixture.create_identity(directory, root/'probes/macos/https-cert.cnf')
    children = []
    with tempfile.TemporaryFile(mode="w+") as server_log, subprocess.Popen(
            [args.server, directory, 'shared-desktop'], stdout=subprocess.PIPE,
            stderr=server_log, text=True) as server:
        try:
            line = server.stdout.readline()
            port = int(re.search(r'macos_https_auth_ready port=(\d+)', line)[1])
            fingerprint = hashlib.sha256(Path(directory, 'cert.der').read_bytes()).hexdigest()
            def post(value, token, path):
                body = json.dumps(value).encode()
                raw = (f'POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\n'
                       f'Authorization: Bearer {token}\r\nContent-Length: {len(body)}\r\n\r\n').encode()+body
                return fixture.request(cert, port, {}, raw=raw)
            def authenticate():
                status, challenge = fixture.request(cert, port, {'username':'synthetic'})
                assert status == 200
                status, reply = fixture.request(cert, port, {'conversation_id':challenge['conversation_id'],
                    'responses':['test']}, '/plank/auth/respond')
                assert status == 200 and reply['state'] == 'authenticated' and reply['shared_desktop'] is True
                return reply['session_token']
            def prepare(token, width, height, mode='hevc-10-420-videotoolbox'):
                status, topology = post({'schema_version':3, 'width':width, 'height':height,
                    'scale':1, 'encoding_mode':mode}, token, '/plank/display')
                assert status == 200, (status, topology)
                return topology
            def launch(token, topology, hold):
                capture = topology['capture']
                status, stream = post({'schema_version':6, 'clipboard':False, 'microphone':False, 'camera':False,
                    'capture_generation':topology['generation'], 'capture_id':capture['id'],
                    'width':capture['width'], 'height':capture['height'],
                    'encoding_mode':capture['encoding_profile']['encoding_mode'],
                    'frame_rate':60, 'bitrate_kbps':50000, 'max_udp_payload_size':1200}, token, '/plank/launch')
                assert status == 200, (status, stream)
                child = subprocess.Popen([args.receiver, fingerprint, '--shared-no-media' if hold else '--no-media'],
                    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
                children.append(child)
                child.stdin.write(json.dumps(stream)); child.stdin.close(); child.stdin = None
                if hold:
                    assert child.stdout.readline().strip() == 'shared_receiver_ready=1'
                return child
            # Same relay address + local account must not supersede either token.
            first = authenticate()
            second = authenticate()
            topology = prepare(first, 3840, 2160)
            # A second setup cannot resize even before the first stream launches.
            assert prepare(second, 1920, 1080) == topology
            keeper = launch(first, topology, True)
            # Different requested geometry/profile adopts the active workspace.
            assert prepare(second, 1920, 1080, 'hevc-10-444-videotoolbox') == topology
            departing = launch(second, topology, False)
            out, err = departing.communicate(timeout=15)
            assert departing.returncode == 0, err
            assert keeper.poll() is None, 'one disconnect stopped the other stream'
            third = authenticate()
            assert prepare(third, 1920, 1080) == topology
            replacement = launch(third, topology, False)
            out, err = replacement.communicate(timeout=15)
            assert replacement.returncode == 0, err
            out, err = keeper.communicate(timeout=20)
            assert keeper.returncode == 0, err
            # Last drain restores display; an independent future session may resize.
            deadline = time.monotonic()+4
            while True:
                token = authenticate()
                current = prepare(token, 1920, 1080)
                if current['capture']['width'] == 1920:
                    break
                assert time.monotonic() < deadline, 'last stream did not release display'
                time.sleep(.1)
            print('macos_shared_desktop=PASS actual_runtime=1 tls=1 quic=1 peers=3 synthetic_devices=1')
        finally:
            for child in children:
                if child.poll() is None:
                    child.terminate()
                child.wait(timeout=10)
            server.terminate()
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait(timeout=5)
            if any(child.returncode for child in children) or server.returncode != 0:
                server_log.seek(0)
                print(server_log.read())
