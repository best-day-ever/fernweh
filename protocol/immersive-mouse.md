# Immersive mouse mode

Immersive mouse is a live-session action for applications that wrap, hide or
recenter their pointer, including 3D editors and games. It is independent of
immersive keyboard mode. Ordinary pointing remains absolute. The initial
implementation is the native macOS Qt Client with an X11 Linux Host.

The toolbar mouse button or **Ctrl+Alt+Shift+M** enters/leaves the mode. Mode
entry preserves the Host's current pointer instead of moving it to a possibly
stale Client position. The Client captures movement until the user releases
the mode; releasing a mouse button does not end capture. This also covers
keyboard-started modal tools. The release shortcut remains local. Focus/Space
loss, reconnect, display changes, tablet takeover and teardown cancel capture.
Activation during an ordinary held-button drag is refused without changing that
drag. Mode defaults off for each session and is not a persisted preference.

## Negotiation and transport

Linux topology feature `0x40000000` advertises support. It is optional: older
Hosts retain ordinary absolute input and do not offer the action. Guest
collaboration streams cannot negotiate or use pointer capture.

Native input type10 carries one32-byte `PlankPointerPacket`, defined in
`plank_transport_pointer.h`. Integers and IEEE754 floats are big-endian:

| Offset | Field |
|---|---|
| 0 | Version1, uint8 |
| 1 | Operation, uint8 |
| 2 | Reserved zero, uint16 |
| 4 | Nonzero epoch, uint64 |
| 12 | Nonzero consecutive sequence, uint64 |
| 20 | Encoded frame width, uint16 |
| 22 | Encoded frame height, uint16 |
| 24 | X, float32 |
| 28 | Y, float32 |

`BEGIN=1` starts a fresh epoch at sequence1. Width/height bind its immutable
coordinate transform; advisory X/Y do not position the Host pointer. `MOVE=2`
carries displacement in encoded-frame pixels. `END=3` completes the mode and
`CANCEL=4` retires it during cleanup; both have zero X/Y. Every record shares
the same ordered input queue as buttons, keyboard and wheel input. Pointer
records are never merged across those transitions.

The Host scales MOVE through the frozen input transform, retains fractional
carry and injects exact pixel deltas through its owned XInput slave. It must not
substitute accelerated evdev motion or a different core pointer. Application
warps remain authoritative because each delta starts at the current Host pointer.

Reliable native event6 carries `END_ACK=5`, echoing the epoch/sequence and frame
size with the authoritative final pointer expressed in encoded-frame pixels.
Only this explicit completion can reanchor the Client pointer. The sampled,
unreliable cursor-position stream remains presentation-only.

While completion is pending, the Client preserves subsequent mouse, key,
modifier and wheel ordering in a bounded queue. A stale ACK cannot release a
new epoch. Timeout, malformed data, invalid sequence, changed transform or failed
owned-device delivery terminate/cancel the input transaction without replaying
an uncertain movement. Cleanup never retries a possibly applied delta.

Raw tablet activity cancels capture before the actual pointer report enters the
input queue. A shared enqueue boundary prevents capture from starting halfway
through that handoff. Battery/administrative reports do not take pointer ownership.
