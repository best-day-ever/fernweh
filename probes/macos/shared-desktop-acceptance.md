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

The later signed candidate containing bounded shared audio taps passed the full
controller with audio required. Three clients shared one source IP, with four
connections total after leave/rejoin. Every receiver decoded 1920×1080 HEVC
10-bit 4:2:0 into `pf20` pixels and received contiguous five-millisecond audio
packets with 240 samples and no reported missing samples:

| Receiver | Duration | Decoded frames | Audio packets | Initial video minus audio |
| --- | ---: | ---: | ---: | ---: |
| Departing | 10 s | 44 | 1,979 | 1.500 ms |
| Replacement | 10 s | 11 | 1,979 | 8.500 ms |
| Survivor | 35 s | 73 | 6,980 | 2.822 ms |
| Keeper | 50 s | 99 | 9,952 | −17.500 ms |

All four acknowledged bitrate control and disconnected cleanly. The two
remaining streams continued after the first left; the replacement's different
geometry and 4:4:4 request returned the unchanged active workspace. Final
authenticated topology remained available after all streams drained. This is
actual direct TLS/QUIC video decoding and audio packet transport evidence; it
does not qualify audible playback, non-silent content, or the Internet relay.

## Optional decoded test-tone gate

Build the receiver with `-DPLANK_PREVIEW_AUDIO_ENERGY`, the prepared libopus
include path and static archive. This adds no product dependency; ordinary
preview builds retain their existing behavior. Use a deployment target compatible
with both prepared archives and the receiver machine, for example:

```sh
xcrun clang -mmacosx-version-min=27.0 -fobjc-arc -Wall -Wextra -Werror \
  -DPLANK_PREVIEW_AUDIO_ENERGY -I"$OPUS_PREFIX/include" \
  -Iprotocol/plank-transport/include probes/macos/preview-receive.m \
  "$TRANSPORT_ARCHIVE" "$OPUS_PREFIX/lib/libopus.a" \
  -framework Foundation -framework CoreMedia -framework CoreVideo \
  -framework VideoToolbox -framework Security -lpthread -lm -o "$ENERGY_RECEIVER"
```

`--audio-energy` requires valid decoded stereo Opus, measurable energy, and two
whole one-second windows dominated by 440 Hz on the left and 880 Hz on the right.
It reports both expected-tone and opposite-tone energy fractions per channel.
Silence, channel swaps and mono mixing fail the synthetic fixture
`tests/audio/macos-preview-audio-energy.c`. Compile that fixture with the same
prepared libopus archive; it needs no capture device or permission.

For an authorized short waveform trial, prefix the ordinary controller command
with `python3 probes/macos/play-acceptance-tone.py "$SSH_TARGET" --`, and give the
controller `--single --single-seconds 12 --audio-energy` plus the energy-enabled
receiver. The wrapper generates an eight-second source WAV, waits for decoded
video, verifies the Host default output is PLANK Output, and then plays the
source using `afplay`. It does not change volume or output settings. Both local
and remote source files are removed afterwards. Captured PCM is reduced to
statistics in memory and never saved. Coordinate this playback with people
sharing the desktop before running it.

For a session where another remote-access application competes for the default
output, use the explicit-device generator without changing any shared setting:

```sh
xcrun clang -mmacosx-version-min=15.4 -fobjc-arc -Wall -Wextra -Werror \
  probes/macos/play-output-tone.m -framework Foundation -framework AudioToolbox \
  -framework CoreAudio -o "$OUTPUT_TONE"
python3 probes/macos/play-acceptance-tone.py --explicit-generator "$OUTPUT_TONE" \
  "$SSH_TARGET" -- python3 probes/macos/shared-desktop-acceptance.py \
  --address "$HOST_ADDRESS" --username "$LOCAL_USER" \
  --certificate "$TRUSTED_LEAF_PEM" --sha256 "$TRUSTED_LEAF_SHA256" \
  --receiver "$ENERGY_RECEIVER" --single --single-seconds 12 --audio-energy
```

This temporary native generator uses AudioQueue bound to the PLANK Output UID.
It synthesizes 440/880 Hz stereo PCM directly for eight seconds, changes no
default device or volume, and is removed from the target after the trial. This
qualifies the explicit output-device path, independently of default routing.

On 2026-11-06 the unchanged strict waveform gate passed through the installed
signed Host: 570,720 decoded PCM frames, seven qualifying one-second windows,
left/right RMS 0.070782/0.070764, expected-tone fractions 0.999928/0.999974 and
opposite-tone fractions below the printed six-decimal precision. The same
12-second connection decoded 22 video frames and received 2,378 audio packets;
bitrate control and clean disconnect passed. The temporary generator was
removed. This verifies non-silent stereo waveform capture, Apple Opus encoding,
QUIC transport and decoding. It does not establish subjective speaker playback.

An earlier default-routed `afplay` trial had non-silent decoded audio but failed
the strict tone gate. A later attempt refused playback when the observed default
output was another remote-access application's device. The explicit-device pass
isolates a working Host capture/codec path; default-output selection must still
be qualified with that competing application's connection closed.

After closing the competing remote-access connection, the default-routed
`afplay` trial also passed the unchanged strict stereo gate on 2026-11-06:
571,200 decoded PCM frames, seven qualifying windows, left/right RMS
0.070522/0.070813, expected-tone fractions 0.999904/0.999952 and opposite-tone
fractions below printed precision. The 12-second stream decoded 22 video frames
and received 2,380 audio packets; bitrate acknowledgement and graceful
disconnect passed. Host counters reported no audio send drops. No tone player or
temporary tone file remained afterwards, and existing render-worker and Host
PIDs were unchanged.

The default output was already PLANK Output before this final trial and remained
so afterwards; the system-alert output remained the physical speakers. This
checks preservation of the observed pre-test routing state, not restoration
from PLANK Output to a physical device. Default-route competition with another
remote-access application remains an operational concern; no shared volume or
device settings were changed by these probes.
