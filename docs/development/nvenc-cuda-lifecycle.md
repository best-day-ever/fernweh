# Direct CUDA NVENC session lifetime

The direct Linux CUDA encoder must finish teardown on its encode thread. It must
not advertise `ASYNC_TEARDOWN`: CUDA current-context bindings are CPU-thread-local,
and transferring an owning session to a detached destructor does not transfer
its context stack. A replacement session could otherwise borrow that context
without retaining it while the detached session releases its last reference.

Each direct encoder now retains the selected device's primary context and pushes
its own balanced binding, including when a primary context was already current.
The caller's previous binding is restored after the encoder, CUDA conversion
stream, converters and allocations have been released. Initialization failures
unwind only the references and bindings that were actually acquired.

Each conversion owns a scope guard that synchronizes partial upload/kernel
submissions before convert() returns an error. The caller can release the
captured source frame immediately on return, before the device is destroyed,
so a destructor-only barrier would be too late. The normal path performs one
barrier and returns any synchronization failure; early-return cleanup reports
its own error without replacing the original dispatch error.

Device teardown also synchronizes conversion work as a final resource barrier. It then destroys NVENC before freeing its
registered input allocation. The existing per-frame NVENC unmap and bitstream
unlock paths remain in force. Capture handle ownership is separate; this change
does not borrow the NvFBC-owned context or extend a capture handle through teardown.

`CudaStreamWork.*` covers partial uploads/kernels, frame reuse and failure
propagation without a GPU. `CudaPrimaryContext.*` exercises the production context owner with a stateful
CUDA function table: empty/current bindings, non-default devices, independent and
nested references, every acquisition failure, cleanup diagnostics and repeated
replacement. It is part of the native Host package regression gate. Those tests
do not establish driver/hardware acceptance.

Qualify the identical RPM on the pinned driver: fresh greeter-to-desktop login,
input, disconnect/reconnect, sign-out, display modesets and repeated sessions.
Check worker PIDs and coredumps before/after every transition; supervisor recovery
must not conceal a crash. Retain the prior RPM and crash evidence. Driver/kernel
changes are outside this fix.

References: [CUDA context management](https://docs.nvidia.com/cuda/cuda-driver-api/cuda_driver_api/group__CUDA__CTX.html),
[NVENC resource lifetime](https://docs.nvidia.com/video-technologies/video-codec-sdk/13.0/nvenc-video-encoder-api-prog-guide/index.html).

On a memory-constrained native builder, set `PLANK_TEST_BUILD_JOBS=1` for the
coverage-enabled Host test target. The default remains2; values other than1/2
fail closed. This controls compilation only and preserves every regression gate.
