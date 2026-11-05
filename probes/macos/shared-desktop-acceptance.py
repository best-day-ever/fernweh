#!/usr/bin/env python3
"""Live shared-Host acceptance. Password and transport tokens never enter files/argv.

Run only against an authorized Host with TCC already approved. The test prepares
one virtual workspace and opens capture streams; it posts no keyboard/mouse
input and writes no captured pixels. Supply the exact public leaf PEM and its
SHA-256 obtained through a separately trusted channel.
"""
import argparse
import getpass
import hashlib
import hmac
import http.client
import json
from pathlib import Path
import re
import resource
import select
import socket
import ssl
import subprocess
import sys
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--address', required=True, help='Host IPv4 address or DNS name')
    parser.add_argument('--port', type=int, default=28989)
    parser.add_argument('--username', required=True)
    parser.add_argument('--certificate', type=Path, required=True, help='Trusted public leaf PEM')
    parser.add_argument('--sha256', required=True, help='Trusted leaf DER SHA-256')
    parser.add_argument('--receiver', type=Path, required=True)
    parser.add_argument('--width', type=int, default=1920)
    parser.add_argument('--height', type=int, default=1080)
    parser.add_argument('--single', action='store_true', help='One 8s stream before concurrent acceptance')
    parser.add_argument('--audio-optional', action='store_true', help='Report audio count without requiring playback')
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    if not re.fullmatch('[0-9a-f]{64}', args.sha256) or not 1 <= args.port <= 65535:
        parser.error('Invalid certificate fingerprint or port')
    address = socket.gethostbyname(args.address)
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    context.minimum_version = ssl.TLSVersion.TLSv1_3
    # Exact leaf pin is the identity; private Host certificates have a generic
    # SAN rather than studio DNS. Trust only that leaf, never a global CA edit.
    context.check_hostname = False
    context.load_verify_locations(cafile=str(args.certificate))
    context.verify_flags |= ssl.VERIFY_X509_PARTIAL_CHAIN
    password = getpass.getpass() if sys.stdin.isatty() else sys.stdin.readline(4097).rstrip('\r\n')
    if not password or len(password.encode()) > 4096:
        parser.error('A bounded nonempty password is required on stdin')

    def request(path, body=None, token=None):
        connection = http.client.HTTPSConnection(address, args.port, context=context, timeout=15)
        try:
            connection.connect()
            if not hmac.compare_digest(hashlib.sha256(connection.sock.getpeercert(binary_form=True)).hexdigest(), args.sha256):
                raise RuntimeError('Host leaf certificate pin changed')
            headers = {'Content-Type':'application/json'}
            if token:
                headers['Authorization'] = 'Bearer '+token
            data = None if body is None else json.dumps(body).encode()
            connection.request('GET' if body is None else 'POST', path, body=data, headers=headers)
            reply = connection.getresponse()
            content = reply.read(65537)
            if len(content) > 65536 or reply.getheader('Cache-Control') != 'no-store':
                raise RuntimeError('Invalid control response framing')
            result = json.loads(content)
            if reply.status != 200:
                raise RuntimeError(f'{path}: HTTP {reply.status}')
            return result
        finally:
            connection.close()

    def authenticate():
        challenge = request('/plank/auth/start', {'username':args.username})
        if challenge.get('state') != 'challenge':
            raise RuntimeError('Host did not issue a password challenge')
        success = request('/plank/auth/respond', {'conversation_id':challenge['conversation_id'], 'responses':[password]})
        if success.get('state') != 'authenticated' or success.get('shared_desktop') is not True:
            raise RuntimeError('Authenticated shared desktop was not granted')
        return success['session_token']

    def prepare(token, width, height, mode='hevc-10-420-videotoolbox'):
        return request('/plank/display', {'schema_version':3, 'width':width, 'height':height,
            'scale':1, 'encoding_mode':mode}, token)

    children = []
    summaries = []
    def launch(token, topology, seconds, label):
        capture = topology['capture']
        reply = request('/plank/launch', {'schema_version':6, 'clipboard':False, 'microphone':False, 'camera':False,
            'capture_generation':topology['generation'], 'capture_id':capture['id'],
            'width':capture['width'], 'height':capture['height'],
            'encoding_mode':capture['encoding_profile']['encoding_mode'],
            'frame_rate':60, 'bitrate_kbps':20000, 'max_udp_payload_size':1200}, token)
        command = [str(args.receiver), args.sha256, '--remote', address, str(seconds)]
        if args.audio_optional:
            command.append('--audio-optional')
        child = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, text=True)
        children.append((label,child))
        child.stdin.write(json.dumps(reply)); child.stdin.close(); child.stdin = None
        if not select.select([child.stdout], [], [], 20)[0]:
            raise RuntimeError(f'{label}: no decoded pixels within 20s')
        if child.stdout.readline().strip() != 'remote_receiver_pixels_ready=1':
            raise RuntimeError(f'{label}: receiver failed before decoded pixels')
        print(f'{label}: decoded pixels ready', flush=True)
        return child

    def finish(child, label):
        out, err = child.communicate(timeout=75)
        if child.returncode:
            # Receiver diagnostics contain only stages and numeric counters.
            raise RuntimeError(f'{label}: receiver failed exit {child.returncode}; {err.strip()}')
        if 'remote_decode=pass' not in out or 'disconnected=1' not in out:
            raise RuntimeError(f'{label}: incomplete decoder/disconnect evidence')
        summaries.append((label,out.strip()))
        print(f'{label}: clean disconnect verified', flush=True)

    try:
        if args.single:
            first = authenticate()
            topology = prepare(first, args.width, args.height)
            finish(launch(first, topology, 8, 'single'), 'single')
            for label, summary in summaries:
                print(label+': '+summary)
            print('single_host_acceptance=pass real_tls=1 real_quic=1 real_decoded_pixels=1')
            return
        first, second, third = authenticate(), authenticate(), authenticate()
        topology = prepare(first, args.width, args.height)
        if prepare(second, args.width, args.height) != topology:
            raise RuntimeError('Pending setup changed another client workspace')
        keeper = launch(first, topology, 50, 'keeper')
        departing = launch(second, topology, 10, 'departing')
        survivor = launch(third, topology, 35, 'survivor')
        finish(departing, 'departing')
        if keeper.poll() is not None or survivor.poll() is not None:
            raise RuntimeError('One disconnect ended another stream')
        rejoin = authenticate()
        alternate = (3840,2160) if args.width != 3840 else (1920,1080)
        if prepare(rejoin, *alternate, 'hevc-10-444-videotoolbox') != topology:
            raise RuntimeError('Late join changed an active workspace')
        replacement = launch(rejoin, topology, 10, 'replacement')
        finish(replacement, 'replacement')
        finish(survivor, 'survivor')
        finish(keeper, 'keeper')
        check = authenticate()
        final = request('/plank/topology', token=check)
        if not final.get('capture'):
            raise RuntimeError('Post-disconnect topology unavailable')
        print('shared_host_acceptance=pass real_tls=1 real_quic=1 real_decoded_pixels=1 max_concurrent=3 reconnect=1 stable_workspace=1')
        print('audio_required='+str(not args.audio_optional).lower())
        for label, summary in summaries:
            print(label+': '+summary)
    finally:
        password = None
        for _, child in children:
            if child.poll() is None:
                child.terminate()
            try:
                child.wait(timeout=10)
            except subprocess.TimeoutExpired:
                child.kill(); child.wait(timeout=5)

if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(f'acceptance_failed: {error}', file=sys.stderr)
        raise SystemExit(1)
