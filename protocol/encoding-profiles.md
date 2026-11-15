# PLANK encoding profile negotiation

The C/Objective-C/C++ catalog in
[`plank_encoding_profiles.h`](plank-transport/include/plank_encoding_profiles.h)
is shared by the desktop Client, Linux Host and macOS Host. It owns append-only
bookmark IDs, wire tuples, capture compatibility, explicit-mode limits and
initial bitrates. It is product policy, not a hardware capability declaration:
Hosts still probe their encoders and Clients still test the exact decoder format.
Dynamic Match Client geometry has separate canvas limits, so the explicit
bookmark limits do not constrain Retina desktop sizes. This consolidation does
not change any stored ID, wire schema or negotiated format.

Each PLANK workstation bookmark stores one capture source and one
complete encoding profile. The profile fixes the encoder backend, codec,
precision, chroma sampling, range, and color transform. The client advertises
exactly the saved format and the host must either acknowledge and produce that
same tuple or reject the launch. Neither side silently substitutes a different
backend or format.

| Capture source | Bookmark profile | Backend | Video format | Chroma | Depth | Color transform |
| --- | --- | --- | --- | ---: | ---: | --- |
| NvFBC 8-bit | H.264 8-bit 4:2:2 | `software-cuda` | `0x0002` | 4:2:2 | 8 | Full-range BT.709 YCbCr |
| NvFBC 8-bit | H.264 8-bit 4:4:4 | `software-cuda` | `0x0004` | 4:4:4 | 8 | Identity GBR |
| NvFBC 8-bit | H.264 10-bit 4:2:2 | `software-cuda` | `0x0010` | 4:2:2 | 10 | Full-range BT.709 YCbCr; 8-bit source up-converted |
| NvFBC 8-bit | H.264 10-bit 4:4:4 | `software-cuda` | `0x0008` | 4:4:4 | 10 | Identity GBR; 8-bit source up-converted |
| NvFBC 8-bit | H.264 8-bit 4:4:4 NVENC | `nvenc-direct` | `0x0004` | 4:4:4 | 8 | Identity GBR |
| NvFBC 8-bit | H.265 8-bit 4:4:4 NVENC | `nvenc-direct` | `0x0400` | 4:4:4 | 8 | Identity GBR |
| NvFBC 8-bit | H.265 10-bit 4:4:4 NVENC | `nvenc-direct` | `0x0800` | 4:4:4 | 10 | Identity GBR; 8-bit source up-converted |
| NvFBC 8-bit | H.264 8-bit 4:2:0 NVENC | `nvenc-direct` | `0x0001` | 4:2:0 | 8 | Limited-range BT.709 YCbCr, sRGB transfer |
| NvFBC 8-bit | H.265 10-bit 4:2:0 NVENC | `nvenc-direct` | `0x0200` | 4:2:0 | 10 | Limited-range BT.709 YCbCr, sRGB transfer; 8-bit source up-converted |
| Native X11/XShm 10-bit (Experimental) | H.264 10-bit 4:4:4 — x264 | `software-cuda` | `0x0008` | 4:4:4 | 10 | Identity GBR |
| Native X11/XShm 10-bit (Experimental) | H.265 10-bit 4:4:4 NVENC | `nvenc-direct` | `0x0800` | 4:4:4 | 10 | Identity GBR |
| ScreenCaptureKit | H.265 10-bit 4:2:0 | `videotoolbox` | `0x0200` | 4:2:0 | 10 | Full-range BT.709 YCbCr |
| ScreenCaptureKit | H.265 10-bit 4:4:4 | `videotoolbox` | `0x0800` | 4:4:4 | 10 | Full-range BT.709 YCbCr |

Identity GBR is full range with `matrix_coefficients=0`, `Y=G`, `U=B`, and
`V=R`; no YCbCr matrix conversion is applied. Native X11/XShm 10-bit capture
never offers an 8-bit profile. The qualified H.264 10-bit 4:4:4 software path
remains the default. Native X11/XShm 10-bit capture retains its Experimental
designation; NVENC is identified as the encoder backend rather than labeled as
an experimental encoding profile.

The historical `software-cuda` wire token names the shared software-encoder
backend and remains protocol-stable. For Native X11/XShm capture, the packed
RGB10 image is already in CPU memory: the host converts it directly into
x264's planar GBR10 input. If the negotiated encode size differs from the X11
canvas, the same CPU pass performs center-aligned bilinear scaling and GBR10
plane generation; the frame is not copied to CUDA and back. The host advertises
this mode only after its encoder probe produces an H.264 High 4:4:4 Predictive
10-bit SPS with full-range identity GBR color metadata.

For the NvFBC HEVC 10-bit NVENC tuple, CUDA expands each 8-bit BGRA component
to the MSB-aligned 10-bit identity plane before NVENC. This preserves all 256
source code values and gives the 10-bit transform/quantization pipeline more
headroom, but it does not claim native 10-bit capture precision. H.264 10-bit
NVENC is not offered because the qualified Ampere and Turing hosts do not
support it.

H.264 NVENC (4:4:4 and 4:2:0) is limited to a maximum encoded dimension of
`4096x2160`. The Client disables `5120x2160` in both bookmark editors while an H.264 NVENC
profile is selected and moves an existing incompatible selection to
`4096x2160`. Software x264 and HEVC NVENC profiles retain the qualified
`5120x2160` mode. Bookmark persistence independently rejects an explicit
virtual-display mode that exceeds the selected H.264 NVENC profile limit.

## NVENC 4:2:0 (bandwidth profiles)

The two 4:2:0 NVENC tuples exist for remote (WAN) use, where the 4:4:4
identity profiles need more bandwidth than the link has. They are a visible
bookmark choice, never a fallback: a 4:4:4 bookmark still fails rather than
dropping to 4:2:0. Wire modes are `h264-8-420-nvenc` (H.264 High, video format
`0x0001`) and `hevc-10-420-nvenc` (HEVC Main10, `0x0200`), NvFBC only, feature
`0x2000000`. The client sends `chroma=0` and `encoder_csc_mode=2`
(`COLORSPACE_REC_709 << 1`, limited range); the host accepts nothing else for
these modes. CUDA converts BGRA to NV12 or MSB-aligned P010 with the BT.709
matrix and rounded limited-range code values (8-bit black/white 16/235,
chroma 128). The VUI carries BT.709 primaries and matrix, limited range and the
sRGB transfer (13), because the captured desktop is sRGB-encoded: clients
present the stream with the same tone curve as the 4:4:4 identity profiles.

The host advertises each mode only after its own startup probe encodes an IDR
whose SPS proves the profile (H.264 High / HEVC Main10), 4:2:0, the bit depth
and that exact VUI; it then also advertises `SCM_HEVC_MAIN10` for the HEVC
mode. H.264 NVENC keeps the `4096x2160` limit below. The client validates the
decoded test frame's depth, 4:2:0 geometry, BT.709 matrix and primaries,
limited range and sRGB transfer before it accepts a hardware path; VideoToolbox
decodes both on Apple Silicon. `tests/protocol/nvfbc-nvenc-420-v1.json` is the
negotiation vector.

`tests/protocol/nvfbc-hevc10-nvenc-v1.json` is the exact negotiation and
pipeline test vector for this tuple. Hosts must reject it when feature `0x2000`
is absent or when the direct NVENC HEVC Rext 10-bit 4:4:4 probe fails.

Decoder choice is internal and profile-specific. The client attempts a real
exact-format hardware test frame first, validates decoded bit depth, chroma
geometry, full-range identity metadata, and renderer import. H.264 may fall back
to FFmpeg software decoding of the same bookmark format; HEVC cannot. It never changes codec,
depth, or chroma to make a hardware probe succeed. On the qualified Intel NUC:

- H.264 4:4:4 profiles use software decode because the iHD VA-API driver does
  not expose an H.264 4:4:4 decode profile.
- H.265 8-bit 4:4:4 may use VA-API Main 4:4:4 with an AYUV surface imported as
  packed RGB and identity-channel reordered in the presentation shader.
- H.265 10-bit 4:4:4 may use VA-API Main 4:4:4 10 with a Y410 surface imported
  as XR30.

HEVC requires an exact hardware path on both desktop Clients. A failed test
frame or import gate stops the connection with a clear hardware-decoding error;
it does not silently start a software HEVC stream. H.264 retains exact-format
software decoding when hardware cannot support the selected profile, including
NVENC H.264. If neither permitted path works, the connection fails clearly.
