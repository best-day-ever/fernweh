# Bitrate-derived Host packet pacing

The Host previously floored its rate-derived congestion-window budget at
1 Gbit/s, regardless of a lower selected video bitrate. Its reported pacing
metric did not configure Quinn's packet scheduler. Quinn instead paced from
the window and RTT, including its ordinary gain. A low encoder target therefore
did not bound short transport bursts.

The shared Host now gives Quinn an explicit packet pacing ceiling:

```text
paced video peak = clamp(encoder peak, selected target, selected target * 1.5)
paced QUIC budget = paced video peak * 1.4 + 1,000,000 bits/s
```

The 40% allowance covers the fixed 30% repair policy, packet headers and symbol
rounding; the fixed reserve covers audio/control. Until the encoder supplies its
peak, initialization allows the same bounded 50% peak headroom. A 50 Mbit/s
selection gets a 71 Mbit/s QUIC budget for a 50 Mbit/s encoder peak, 85 Mbit/s for
a 60 Mbit/s peak, or at most 106 Mbit/s even if the encoder reports a much larger
peak. These are ceilings, not rates at which an idle/static stream must send.

This preserves limited headroom for variable-size frames without granting an
arbitrary gigabit budget. The encoder still receives its selected target and
peak settings. The policy runs in the Host, so an existing compatible Client
needs no new setting or wire version.

## Burst bound

The scheduler uses a token bucket with two milliseconds of saved credit,
bounded between two and 256 path MTUs. At the maximum 106 Mbit/s budget for a
50 Mbit/s selection, the bucket holds 26,500 bytes. Over an interval of length
`t`, paced QUIC bytes are bounded by `budget * t / 8 + bucket capacity`.
Packets in a small GSO batch can still leave at NIC speed; the bound controls
batch size and refill rate. UDP/IP/Ethernet headers are outside this accounting.
This is not an exact Ethernet line-rate policer or measured path capacity.

Idle time cannot accumulate more credit. Bitrate reductions clamp saved tokens
before another packet is sent, including when an incoming connection binds its
authenticated endpoint policy. The congestion window derives from the same
budget, but it does not determine the capped refill rate. RTT growth and the
minimum congestion window cannot increase that rate. Loss probes remain
window-exempt but are paced; ACK-only and connection-close packets retain their
normal QUIC exceptions. Controllers that do not request a cap retain ordinary
Quinn pacing.

Sustained encoder overshoot or a path below the selected rate can still exhaust
existing bounded queues. This change neither enlarges those queues nor adds
adaptive bitrate/congestion backoff. The pacing policy alone does not establish
the cause of any reported freeze.

This follows the token-bucket pacing approach described in
[RFC 9002 section 7.7](https://www.rfc-editor.org/rfc/rfc9002.html#section-7.7).
Implementation details and retained upstream licensing are in the
[vendored patch record](../../third_party/quinn-proto-0.11.17/PLANK-PATCH.md).

## Verification and rollout

Regressions cover bounded target/peak budgets, live reductions before ACKs,
authenticated endpoint isolation, idle recovery, fractional-byte refill,
clock/MTU changes and large windows. An encrypted QUIC endpoint/GSO test checks
all sampled interval envelopes at 68.5 Mbit/s while receiving every payload
unchanged. Host packaging requires those tests and the `PLANK sender:
bitrate-paced` binary marker. The native streaming runner requires three
consecutive 900-frame matrices for each of a 150 Mbit/s CBR peak and a 225 Mbit/s
VBR peak, with identical 150 Mbit/s offered load, controlled omissions,
recovery checks and latency deadlines.

The initial rigid target-only 35%-overhead candidate is rejected. It sometimes
passed recovery while discarding queued sender datagrams, and a later repetition
missed the unchanged 100 ms deadline. Waiting for a fully refilled bucket also
failed; the scheduler now wakes when the next datagram fits. The selected
headroom and corrected overhead preserve smooth delivery rather than forcing a
strict video-target-only wire cap. Test deadlines and queue capacities remain
unchanged. Retain failed evidence alongside successful component results.

In existing Kynet telemetry, use `controller_pacing_bps` for the enforced
ceiling. The legacy `effective_pacing_bps` still estimates window/RTT pacing and
can greatly exceed the actual cap on a low-RTT path. Inspect queue evictions as
well as reconstructed frames: successful FEC can hide sender-side drops.

Hardware qualification still needs the exact candidate on an idle authorized
test pair: moving content and large keyframes at 50/150 Mbit/s, audio/input
responsiveness and constrained-link loss/latency observations. Passing loopback
tests does not establish improvement on a live WAN. This source change does not
install packages or change network settings.

Local Apple Silicon component evidence: 279 vendored Quinn tests, 69 transport
unit tests, four telemetry tests and six reliable-data tests pass. The dedicated
native runner passes all six 900-frame CBR/VBR matrices with zero unrecovered
symbols and zero reported sender queue evictions. Both fingerprint and
setup-promoted C ABI trust paths pass video/audio/input/control and closure.
Root CI runs 119 tests with one existing platform skip. These results do not
qualify an installed Linux package, actual codec output or a real WAN path.
