# Windows cursor, pen and per-monitor presentation

Authorized by the operator on 18 November 2026. Adapt selected SkulleyFX
Windows improvements, preserving attribution and Fernweh's protocol contracts.

Deliver an owned, non-activating Win32 host-cursor surface; timestamp-matched
native Windows pointer samples with WinTab pressure/contact and focus-safe
Ink fallback; and one correctly cropped presentation per selected monitor.
Keep raw HID and normalized pen mutually exclusive, and preserve all personal
display choices. No implicit codec, bit-depth or chroma downgrade is allowed.

The existing Vulkan renderer already supports multiple surfaces and software
decode. Extend Windows session selection and hardware D3D presentation without
forcing Vulkan on every GPU. Validate identity channel mapping and ten-bit
output; a driver unable to satisfy the selected profile must use the exact
software path or fail clearly. Rendering does not create decoder capability.

Validation: focused portable pen/cursor/geometry tests, native Windows app and
regression suites in direct and broker builds, exact-format GPU/pixel checks,
and packaging. Preserve protected builder services and the installed release.
Physical tablets and two-display acceptance are distinct from helper tests or
synthetic surfaces and must be recorded honestly. No feed promotion is part
of an unqualified candidate build.

## Hardware qualification

The operator will prepare and qualify a physical Windows test box. Use the
same candidate MSI and record its SHA256, Windows/GPU driver, monitor resolutions,
scale/refresh settings and tablet model/driver. Retain the selected bookmark
profile and application log with each result.

1. Save two selected client monitors, reconnect and reopen display setup. Check
   each output's crop and pointer alignment with numbered corner markers;
   include a monitor left of the primary, mixed DPI, unequal sizes and a
   different selected primary. Switch windowed/fullscreen repeatedly, including
   the second output; the toolbar stays on the primary and showing secondary
   windows must not take keyboard focus.
2. Run the selected H.264 and HEVC 10-bit 4:4:4 profiles independently. Confirm
   the logged decoder/profile and actual output precision; check fine coloured
   edges and a 10-bit ramp on a capable display. Unsupported hardware decode
   may select exact-profile software decode, but must never select 8-bit or
   4:2:0 automatically. Measure frame pacing separately from pixel correctness.
3. Exercise view-only sharing, owner/control handoff, host cursor shapes and
   hotspots, pen/mouse switching, toolbar clicks, Alt+Tab and minimized windows.
   The remote cursor must neither remain over another app nor intercept input.
4. Check light strokes, full pressure ramps, stationary pressure, tilt, eraser
   and pen buttons with Ink enabled; repeat contact/pressure with Ink disabled
   and WinTab available. Force normalized input using
   `PLANK_WINDOWS_PEN_INPUT=1`, then isolate Ink with `PLANK_WINTAB=0`. Repeat
   with default raw USB selection. Test driver mapping/force-proportions,
   crossing displays and strokes begun outside the focused stream; no duplicate
   contact or resumed held stroke is acceptable. WinTab-only tilt is outside
   this candidate's implementation.
5. Unplug/replug the tablet and one monitor, change DPI/primary, release capture,
   lose focus mid-stroke, disconnect/rejoin and exercise automatic recovery.
   Check cancellation, picture freshness, cursor cleanup and preserved saved
   settings. Repeat Blender/Unreal immersive mouse release and tablet takeover.

Helper/GPU fixtures and compilation do not close these physical acceptance
items. No install, update-feed promotion or sustained-performance qualification
is implied by producing the candidate.
