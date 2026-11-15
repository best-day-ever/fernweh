# PLANK 1.4.18 integration candidate

This draft incorporates the coordinated upstream frontend, Native10, transport,
metrics and installer changes while retaining Fernweh's platform adapters.
Current Fernweh main is merged into the parent, Client and Linux Host with
ancestry preserved. Source integration and installed streaming acceptance remain
separate gates. No release, installer feed or deployment pin is changed.

## Reconciled source

- Qt display planning retains saved monitor identities, primary selection,
  crops, physical/virtual backing, shared views and screen-change Apply/Keep.
  Layout acknowledgement precedes decoder probing; shutdown cancels pending
  acknowledgements. Bookmark sizing uses selected monitors while profile
  planning retains every monitor probe.
- Qt text/file clipboard callbacks use the GUI owner, with focus, generation,
  permission and lifetime checks. File selection and permission changes use
  actual Qt controls rather than relying on SDL event polling.
- Windows retains its SDL decoder, pacer, D3D resources, lifecycle callbacks and
  microphone adapter. Retired AV1 protocol branches are removed from retained
  renderers; no retired common-C API is restored.
- Local disconnect/toolbar shortcuts bypass the bounded native input queue,
  retain focus/permission/lifetime checks, and preserve toolbar pin/idle behavior.
  Narrow launcher toolbars can shorten the version label without clipping status.
- Shared packet/render/recovery timestamps use one clock. Hardware-first decoder
  selection retains software fallback only for the exact selected format.
- Linux capture uses one C++/NVCC image definition and retained CUDA declarations.
  Native10 metadata crosses the real compiler boundary correctly. The local C
  ABI is version15, retaining Fernweh setup-admission/file-clipboard fields.
- Mac display operations run outside the admission lock and retain the first
  shared owner's workspace until the last drain. Metrics remain off by default;
  startup and installer configuration follow the optional-settings contract.
- Native build phases do not install operating-system packages. Dependency
  provisioning belongs to disposable build environments. Tests select the exact
  Cargo artifact from the shared target rather than guessing from cached files. The custom listener uses the same canonical bounded
  datagram burst capacity as generic peers, preserving large keyframes and FEC.

## Verification and remaining gates

The [integration ledger](upstream-integration-2026-11-14.md) and
[review receipt](review.md) record the native build and regression evidence.
The draft reviews are [parent6](https://github.com/best-day-ever/fernweh/pull/6),
[Client5](https://github.com/best-day-ever/fernweh-client/pull/5), and
[Linux Host7](https://github.com/best-day-ever/fernweh-host-linux/pull/7).
Both Client profiles pass their complete native Mac and Windows gates. Linux
package/loss qualification remains open after a failed repeat and measured
endpoint receive-buffer loss; retain its evidence and thresholds before promotion.

Installed streaming is still separate: macOS15/27 Client and26.7/27 Host sessions,
Windows streaming, shared control/tablets and Blender/Unreal immersive workflows
need actual acceptance. Device-free Native10/CUDA tests do not establish live
30-bit X11 capture. The synthetic Mac root handoff fixture needs a suitable
privileged test environment; ordinary unprivileged display-lease, sharing and
cancellation fixtures do not replace it. Hardware sleep/lid acceptance remains
open. Keep the reviews in draft until their required gates are satisfied.
