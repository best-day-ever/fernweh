# Vendored Quinn backport

This directory contains crates.io `quinn-proto 0.11.17` with PLANK's narrow
DATAGRAM accounting, MTU-boundary and telemetry repairs described below, plus
the opt-in controller feature described later.

The source crate is pinned by `Cargo.lock`. Its original MIT and Apache-2.0
licenses and crates.io VCS provenance are retained in the vendored directory.

The send-buffer backport corrects two defects present in the released code:

- send-buffer pruning now includes the memory needed by the new DATAGRAM;
- pruning relies on `DatagramBuffer::pop_front()` as the sole payload-byte
  decrement instead of decrementing the counter twice.

It also rejects a single DATAGRAM that cannot fit in the configured send
buffer and adds a focused regression test. The first and third changes are
adapted from upstream Quinn commits
`88c4e96d119e1ada071356986415de8294a89d65` and
`c50f83bc4f5df16aa71d05e2d20e8f2b04ae4f62`. The double-decrement correction
is specific to the `DatagramBuffer` refactor shipped in 0.11.17.

PLANK also retains the upstream `instinctual/plank` MTU recovery fix
(`9afebe0`): a queued DATAGRAM whose size equals the recovered payload limit
remains valid. Larger packets are dropped and counted separately from
send-buffer evictions. The vendored Quinn tests cover an unchanged MTU and a
reduced MTU at the exact size boundary.

The additional `plank-bbr-default` feature makes Quinn's experimental BBR
controller the default only when PLANK explicitly enables it. Without that
feature, the vendored crate retains upstream's CUBIC default. This narrow hook
exists because Kyber 0.28.0 does not expose Quinn's congestion controller in
its public connection options; it is not a proposed product API.

Both the production transport crate and its standalone probes consume this one
canonical copy. Do not create a second probe-local vendor tree.

Remove this override only after Kyber resolves to a released Quinn version
that contains equivalent fixes and passes the PLANK 5% and 10% loss
qualification tests.

## Explicit sender pacing ceiling

PLANK's Host controller supplies an optional `Controller::pacing_rate()` in
bits/s. Quinn enforces it in the packet scheduler with a token bucket separate
from the congestion window. Controllers returning `None` retain window/RTT
pacing. Controller metrics remain telemetry rather than scheduler configuration.

The capped bucket holds eight milliseconds of traffic, bounded between two and
256 path MTUs. It cannot accumulate idle credit beyond that capacity. Refill
preserves fractional bytes; rate reductions and authenticated policy binding
clamp saved credit before the next transmission. The ceiling also applies to
ack-eliciting loss probes, which remain exempt from the congestion window.
ACK-only and connection-close packets retain their normal QUIC exceptions.

The Host uses the encoder peak, bounded to 1.5 times the selected video target,
then adds 40% for repair/headers and 1 Mbit/s for audio/control. Excessive encoder
peak settings cannot raise the ceiling beyond that bounded headroom. This is a paced QUIC
byte budget with a bounded burst allowance, not an Ethernet line-rate policer.
Pacer regressions cover idle recovery, rate changes, fractional credit, MTU and
clock changes. An encrypted endpoint/GSO regression verifies the envelope over
sampled intervals even with a congestion window large enough to disable the
ordinary window-derived pacer.
