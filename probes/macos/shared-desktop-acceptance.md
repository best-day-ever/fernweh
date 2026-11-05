# Live shared desktop acceptance

Build `preview-receive` with the normal macOS preview qualification build, then
run the controller against an authorized Host with permissions granted:

```sh
python3 probes/macos/shared-desktop-acceptance.py \
  --address "$HOST_ADDRESS" --username "$LOCAL_USER" \
  --certificate "$TRUSTED_LEAF_PEM" --sha256 "$TRUSTED_LEAF_SHA256" \
  --receiver "$PREVIEW_RECEIVER" --width 1920 --height 1080
```

The password is read from a terminal prompt or one line of stdin. The controller
checks the independently obtained leaf certificate before sending credentials.
Tokens stay in memory and receiver stdin. No screen images or audio files are
written. The receiver decodes HEVC frames using VideoToolbox and validates the
negotiated dimensions. It sends bitrate control and graceful disconnect, but no
keyboard, pointer, clipboard, camera or microphone input.

Use `--single` for an initial eight-second stream. The normal run opens three
concurrent streams, disconnects one while the others continue, reconnects a
replacement and requests a different geometry/profile to verify active workspace
retention. The final session verifies topology remains accessible. Audio packet
arrival is required by default; `--audio-optional` explicitly waives that gate
and reports the observed count. Packet checks do not qualify audible playback.

For a broker relay, the receiver also accepts a Host launch JSON reply directly:
`preview-receive LEAF_SHA256 --remote RELAY_IPV4 SECONDS`. Replace `udp_port` in
the reply with the broker's reserved UDP port in memory before sending JSON on
stdin. The TLS control/authentication and broker admission remain the caller's
responsibility. Receiver stdout contains readiness, decode and transport counters;
it does not print tokens. Check exit status zero plus `remote_decode=pass` and
`disconnected=1`.

## Recorded scope, 2026-11-05

An authorized headless Apple Silicon canary passed direct TLS/QUIC capture with
1920×1080 HEVC 10-bit 4:2:0. The initial eight-second stream decoded 35 frames.
The concurrent run decoded 12, 11, 37 and 56 frames across four connections, with
three active simultaneously. All sessions acknowledged bitrate changes and
closed cleanly. Disconnecting one preserved the others; reconnect and active
workspace retention passed. These runs used `--audio-optional`: all audio packet
counts were zero, so audio was not qualified. No input injection, Internet relay,
physical display restoration or visual color-quality claim follows from this
counter-only evidence. Re-run against the final signed candidate after changes.

After the installed output device was registered with Core Audio and system-audio
consent granted, a follow-up single-stream run passed with audio required:
13 decoded video frames, 1,580 contiguous five-millisecond audio packets and a
23.822 ms initial source timestamp difference over eight seconds. Bitrate control
and graceful disconnect passed. This establishes audio packet delivery, not
non-silent content or audible playback; concurrent audio still requires the
updated signed Host candidate.
