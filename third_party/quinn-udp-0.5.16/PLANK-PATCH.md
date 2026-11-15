# Vendored Quinn UDP socket integration

Base: crates.io `quinn-udp 0.5.16`, archive SHA-256
`af66907df18639dcf4db56ca65490cabc4b27a97dbadd96f2926cca73298f016`.
Original MIT/Apache licenses, VCS provenance, source and tests are retained.
This is the UDP release already locked with Quinn 0.11.12 and quinn-proto 0.11.19.

The opt-in `plank-bounded-receive-buffer` feature requests 4 MiB of receive
capacity when Unix or Windows initializes a Quinn UDP socket. It preserves an
existing capacity at least that large. Rejected requests retain the working
socket and its original capacity; clamped requests use the actual OS capacity.
Diagnostics report the requested and actual capacity rather than claiming the
request changed a kernel limit. No system limits, privileges, pacing, packet
contents, FEC or deadlines change. Without the feature, socket behavior is
unchanged. PLANK enables the feature explicitly in its transport dependency.

The native loss gate checks the product endpoints as well as the proxy for
kernel receive drops on Linux. A proxy-only zero does not establish controlled
loss when an endpoint overflows. Keep exact-loss evidence separate from frame
recovery and retain any failed trial.

Run the real socket regressions with:

```sh
cargo test --locked --manifest-path third_party/quinn-udp-0.5.16/Cargo.toml \
  --features plank-bounded-receive-buffer --test tests plank_receive_buffer -- --nocapture
```

These tests check the OS-clamped capacity and preservation of an existing
larger request. Product and probe consumers use the same patched crate through
the transport manifest. Remove this override only after an upstream socket
configuration contract replaces it and native transport qualification passes.
