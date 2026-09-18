# macOS client Wacom/pen input -- manual test plan

Status: **no Wacom or other pressure-sensitive tablet was available in the
development environment that produced this plan.** Everything below is a
design-time test plan to run once real hardware and a qualified macOS build
are available, not a report of hardware results. See
`docs/bde/gap-analysis.md` for how this fits the wider Mac-client gap list,
and `apps/client/app/streaming/input/macpen.h` for the implementation this
plan exercises.

## What this covers

The macOS client's new normalized-pen forwarding path: SDL3 pen events
(`SDL_EVENT_PEN_*`, delivered by SDL's Cocoa backend from AppKit
`NSEventTypeTabletPoint`/`NSEventTypeTabletProximity`) translated into the
same `LiSendPenEvent()`/`PLANK_TRANSPORT_INPUT_PEN` ("type7") wire packets
the Linux client's `LinuxWacomInput` already sends. See
`apps/client/app/streaming/input/macpen.h` for the full design rationale
and `protocol/wacom-hid.md` / `protocol/macos-pen-input.md` for the wire
format this exercises.

This does **not** cover macOS raw-HID Wacom forwarding (the exact-format
path Linux uses for anything newer than the first-generation Intuos Pro
family, `protocol/wacom-hid.md`). That remains unimplemented for macOS; see
"Non-goals" below.

## Prerequisites

- A macOS client built per `docs/development/build/macos-client-build-runbook.md`
  on the authorized dedicated development Mac (Apple Silicon, macOS 27,
  SDK 27, Qt 6.10.2, Rust 1.89.0 exactly -- see `docs/bde/gap-analysis.md`
  "macOS client build attempt" for why this could not be produced in this
  pass).
- A PLANK host that advertises `LI_FF_PEN_TOUCH_EVENTS` (the Rocky 9.7
  host already does; see `apps/host/linux/src/input.cpp`
  `PLANK_TRANSPORT_INPUT_PEN` handling and
  `apps/host/linux/src/platform/virtualhid_input.cpp` `pen_update()`).
- A Wacom (or other SDL3-supported) pressure-sensitive tablet connected to
  the Mac client machine, with the vendor driver installed per its normal
  macOS install instructions.
- A pressure-aware application running on the *host* side to observe the
  result -- Pixelmator was the reference app used for the macOS host-side
  normalized-pen work in `protocol/macos-pen-input.md`; any pressure-aware
  Linux app (Krita, GIMP with a pressure-aware brush engine) works
  equally well against the Rocky host.
- `PLANK_MACOS_PEN_INPUT=1` set in the client's environment before launch
  -- this path is opt-in by design until it has passed the checks below on
  real hardware (see "Why opt-in" below).

## Setup

```sh
export PLANK_MACOS_PEN_INPUT=1
open /path/to/plank-client.app --args ...   # or run the built binary directly
```

Confirm in the client's log output:

```
Using SDL3 pen events for PLANK normalized pen transport
```

If instead you see:

```
Host lacks normalized pen support (LI_FF_PEN_TOUCH_EVENTS); macOS pen/tablet input will not be forwarded
```

the connected host does not advertise `LI_FF_PEN_TOUCH_EVENTS` -- confirm
you are connected to a current `apps/host/linux` build, not an older or
unrelated Sunshine/Moonlight host.

## Checks

1. **Proximity.** Bring the pen near the tablet without touching it. The
   host-side app should show a hover cursor/preview at the corresponding
   position. Lift the pen far enough away to leave proximity; the hover
   indicator should disappear (`LI_TOUCH_EVENT_HOVER_LEAVE`).
2. **Position mapping.** Move the pen across the full active area of the
   tablet while hovering. Confirm the remote cursor tracks the full video
   region 0..1 in both axes, including with the client window resized,
   letterboxed (non-matching aspect ratio), and on a Retina display (see
   `PlankPresentation::mapWindowPointToStream`, the same function absolute
   mouse positioning already uses).
3. **Pressure.** Draw a stroke with a pressure-aware brush at light,
   medium, and full pressure. Confirm the stroke width/opacity on the host
   app responds proportionally and reaches both ends of the 0.0..1.0
   range. Compare against the same tablet's native pressure curve on the
   Mac itself (e.g. in Preview or a local pressure-test page) to sanity
   check there is no unexpected compression.
4. **Tilt.** With a tilt-capable pen (most Wacom Pro Pen family), tilt the
   pen in each of the four cardinal directions while drawing in an app
   with tilt-aware brushes (shading/airbrush brushes in Krita or
   Pixelmator). Confirm the brush shape/angle responds and roughly matches
   the physical tilt direction and magnitude. This is the check most
   likely to expose a real bug: `macpen.h`'s tilt encoding assumes SDL's
   `SDL_PEN_AXIS_XTILT`/`SDL_PEN_AXIS_YTILT` share libinput's exact sign
   convention (see `apps/client/app/streaming/input/pentiltencoding.h`),
   which was inferred from documentation, not cross-checked against a real
   device on either platform.
5. **Eraser.** Flip the pen to its eraser end (or press whatever the
   tablet driver maps to "eraser"). Confirm the host app switches to erase
   behavior. Confirm going back to the pen tip switches back cleanly, with
   no stuck erase state.
6. **Barrel buttons.** Press each barrel button in turn while hovering
   (not drawing). Confirm each maps to a distinct host-side action
   (primary/secondary/tertiary -- `LI_PEN_BUTTON_PRIMARY/SECONDARY/TERTIARY`).
   A fourth+ button, if the pen has one, is expected to do nothing (the
   wire format only carries three pen button bits); confirm it fails
   silently rather than misfiring another button.
7. **Distance-based hover feedback.** If the host app shows a hover
   preview whose size/opacity scales with proximity distance (not just a
   binary near/far), confirm it responds smoothly as the pen approaches
   the tablet. This exercises `SDL_PEN_AXIS_DISTANCE`; note in the result
   which direction (0=touching vs 0=farthest) the tablet firmware actually
   reports, since `macpen.h` assumes 0=touching without a source device to
   confirm it against (see the "Known open questions" section below).
8. **Tool swap.** With a two-ended stylus (pen tip and eraser both
   trackable), swap ends mid-air without lifting fully out of proximity.
   Confirm the host sees a clean hover-leave/hover-enter transition
   (`macpen.h`'s `beginOrContinuePen`) rather than a blended or stuck
   state. This also covers the (much less likely) case of two *separate*
   physical pens being brought into range at once.
9. **Focus loss.** Start a stroke (pen down, drawing), then switch away
   from the client window (Cmd+Tab, or click another app) mid-stroke.
   Confirm the host receives a clean cancel (`LI_TOUCH_EVENT_CANCEL_ALL`)
   rather than a stuck tip-down state, and that returning focus to the
   client and starting a new stroke behaves normally.
10. **Disconnect/reconnect.** Disconnect the tablet's USB cable (or
    power it off) mid-session, then reconnect it. Confirm the client
    doesn't crash or wedge, and pen input resumes working after
    reconnection (SDL is expected to assign a new `SDL_PenID`; confirm
    `macpen.h`'s per-ID tracking handles this as a normal tool swap rather
    than misbehaving).
11. **Multiple displays.** Repeat checks 2-3 with the client spanning
    more than one presentation output, to confirm the per-output
    `canvasRect` mapping (`mapWindowPointToNormalizedStream` in
    `input.cpp`) picks the correct output and offset.
12. **Regression: mouse/keyboard unaffected.** With
    `PLANK_MACOS_PEN_INPUT=1` set but no tablet connected, confirm mouse
    and keyboard input behave exactly as before this change (this path
    should be fully inert with no tablet present -- SDL simply never fires
    `SDL_EVENT_PEN_*`).
13. **Regression: feature-flag-off host.** Connect to a host that does
    not advertise `LI_FF_PEN_TOUCH_EVENTS` (or force it off for a test
    build) and confirm the client logs the "Host lacks normalized pen
    support" message and otherwise streams normally.

## Known open questions to resolve during this pass

- **Distance axis direction.** `SDL_PEN_AXIS_DISTANCE` is documented in
  `SDL3/SDL_pen.h` only as "Unidirectional: 0.0 to 1.0" with no stated
  zero point. `macpen.h` assumes 0.0 = touching (matching
  `LiSendPenEvent()`'s own hover-distance convention in `Limelight.h`) by
  analogy, not by testing against a real device. Check 7 above should
  confirm or correct this; if it is backwards, invert the value in
  `SdlInputHandler::handlePenAxisEvent`/`PlankMacPenInput::handleAxis`.
- **Tilt sign convention.** Same caveat as above for
  `SDL_PEN_AXIS_XTILT`/`YTILT` vs. libinput's `tilt_x`/`tilt_y` -- assumed
  identical (both documented as degrees, bidirectional, -90..90), not
  cross-checked. Check 4 above is the direct test.
- **`SDL_GetPenDeviceType()`** (direct vs. indirect pen) is not currently
  consulted. All Wacom-class tablets currently in scope are indirect
  (external tablet, not a touch display), so this has not mattered yet;
  revisit if a direct-touch device (e.g. an iPad used as a Sidecar/tablet
  display) needs different cursor-visibility handling.
- **Multiple simultaneous tablets.** Only one logical pen is representable
  on the wire at a time (see `macpen.h`'s class-level comment); if a
  workflow genuinely needs two tablets active at once, that is a protocol
  change (a per-pen identifier on `PLANK_TRANSPORT_INPUT_PEN`), not a
  client-only fix.

## Why opt-in (`PLANK_MACOS_PEN_INPUT=1`)

Every other piece of this codebase treats "qualified on real hardware" and
"builds and passes a unit test" as different bars (see
`docs/bde/gap-analysis.md` and `AGENTS.md`'s hardware-qualification
language throughout). This path has never touched a real tablet. Keeping
it behind an explicit opt-in environment variable means it cannot regress
any existing macOS client behavior for users who have not deliberately
turned it on, while still being trivially available for the qualification
pass above. Once checks 1-13 pass on real hardware, flip the default in
`apps/client/app/streaming/input/input.cpp`'s `setWindow()` (the single
`qEnvironmentVariableIntValue("PLANK_MACOS_PEN_INPUT")` check) rather than
adding a second mechanism.

## Non-goals for this pass

- **macOS raw-HID Wacom forwarding** (the exact-format path for
  non-first-generation-Intuos-Pro devices; see `protocol/wacom-hid.md` and
  the Linux `linuxrawwacom.cpp`/`LI_FF_RAW_HID_TABLET` path). This would
  need `IOHIDManager`/IOKit entitlements and a macOS-specific transport
  implementation and is out of scope here; the normalized-pen path above
  is the fallback Linux itself uses for tablets that don't qualify for
  raw-HID, and is a reasonable first step for macOS precisely because it
  needs no platform-specific driver code at all.
- **ExpressKeys, ring controls, and pad-only Wacom surfaces.** Not
  representable by the normalized-pen wire format on any platform (see
  `protocol/wacom-hid.md`'s "First-generation Intuos Pro fallback"
  section, which documents the same limitation for the Linux fallback
  path).
- **True barrel rotation** (`SDL_PEN_AXIS_ROTATION`, e.g. an Art Pen's
  twist) and the **slider**/**tangential pressure** axes. No wire field
  exists for any of these; see `macpen.h`'s comment on `handleAxis()`.
