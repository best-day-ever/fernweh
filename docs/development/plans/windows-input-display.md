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
