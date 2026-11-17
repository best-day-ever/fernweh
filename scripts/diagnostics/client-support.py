#!/usr/bin/env python3
"""Request and retrieve bounded Client diagnostics through administrator SSH."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import shlex
import stat
import subprocess
import sys
import tempfile
import time

REQUESTS = "support-requests"
REPORTS = "support-reports"
MAX_BUNDLE = 193 * 1024
RETENTION = 14 * 86400


def private_directory(path, create=False):
    if create:
        path.mkdir(mode=0o700, exist_ok=True)
    mode = path.lstat().st_mode
    if not stat.S_ISDIR(mode) or mode & 0o077:
        raise ValueError("diagnostic directories must be private, real directories")


def read_json(path, limit):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd, "rb") as stream:
        mode = os.fstat(stream.fileno())
        if not stat.S_ISREG(mode.st_mode) or mode.st_mode & 0o077 or mode.st_size > limit:
            raise ValueError("unsafe diagnostic file")
        data = stream.read(limit + 1)
        if len(data) > limit:
            raise ValueError("oversized diagnostic file")
        return json.loads(data)


def operate(directory, user, command, ttl=600, request_id=None, now=None):
    now = int(time.time()) if now is None else now
    if not re.fullmatch(r"[A-Za-z0-9_.@-]{1,128}", user):
        raise ValueError("invalid account name")
    private_directory(directory)
    prefix = hashlib.sha256(user.encode()).hexdigest()
    if command == "request":
        if not 1 <= ttl <= 900:
            raise ValueError("request lifetime must be 1 to 900 seconds")
        folder = directory / REQUESTS
        private_directory(folder, create=True)
        request = {"schema": 1, "request_id": secrets.token_hex(16),
                   "issued_unix": now, "expires_unix": now + ttl}
        fd, temporary = tempfile.mkstemp(prefix=".request-", dir=folder)
        try:
            with os.fdopen(fd, "w") as stream:
                json.dump(request, stream)
                stream.flush()
                os.fsync(stream.fileno())
            os.replace(temporary, folder / (prefix + ".json"))
        finally:
            if os.path.exists(temporary):
                os.unlink(temporary)
        return request
    if request_id is not None and not re.fullmatch(r"[0-9a-f]{32}", request_id):
        raise ValueError("invalid request ID")
    folder = directory / REPORTS
    if not folder.exists():
        return []
    private_directory(folder)
    result = []
    # This spool is bounded to 128 completed bundles by the receiver.
    for path in sorted(folder.glob(prefix + "-*.json"))[:128]:
        value = read_json(path, MAX_BUNDLE)
        if now - value["received_unix"] > RETENTION:
            continue
        bundle = value["bundle"]
        if request_id is not None and bundle["request_id"] != request_id:
            continue
        if command == "read":
            result.append(value)
        else:
            report = bundle["report"]
            result.append({"request_id": bundle["request_id"],
                           "received_unix": value["received_unix"],
                           "client_session": value["client_session"],
                           "version": report["version"], "platform": report["platform"],
                           "report_id": report["report_id"],
                           "log_bytes": len(bundle["log_tail"].encode())})
    return sorted(result, key=lambda r: r["received_unix"], reverse=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, default=Path("/var/lib/plank-broker/diagnostics"))
    parser.add_argument("--user", required=True, help="exact authenticated broker account")
    parser.add_argument("--ssh", help="administrator SSH destination; runs as the broker service account")
    parser.add_argument("--output", type=Path, help="write JSON to a new owner-only local file")
    commands = parser.add_subparsers(dest="command", required=True)
    request = commands.add_parser("request", help="ask signed-in clients for this account to upload a log tail")
    request.add_argument("--ttl", type=int, default=600)
    commands.add_parser("list", help="list received bundle metadata")
    read = commands.add_parser("read", help="retrieve received bundles for one request")
    read.add_argument("request_id")
    args = parser.parse_args()
    try:
        if args.ssh:
            remote = ["sudo", "-n", "-u", "plank-broker", "python3", "-", "--directory", str(args.directory),
                      "--user", args.user, args.command]
            if args.command == "request":
                remote += ["--ttl", str(args.ttl)]
            elif args.command == "read":
                remote.append(args.request_id)
            child = subprocess.run(["ssh", "-T", "--", args.ssh, shlex.join(remote)],
                                   input=Path(__file__).read_bytes(), stdout=subprocess.PIPE,
                                   check=True, timeout=30)
            data = child.stdout
        else:
            data = (json.dumps(operate(args.directory, args.user, args.command,
                                     getattr(args, "ttl", 600), getattr(args, "request_id", None)),
                               indent=2) + "\n").encode()
        if args.output:
            fd = os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
            with os.fdopen(fd, "wb") as stream:
                stream.write(data)
            print(json.dumps({"saved": str(args.output), "bytes": len(data)}))
        else:
            sys.stdout.buffer.write(data)
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        # Keep account data and raw bundle contents out of failure diagnostics.
        print("diagnostic operation failed: " + type(error).__name__, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
