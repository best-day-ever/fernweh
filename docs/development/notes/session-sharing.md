# Shared desktop control improvements

This is a source candidate for the SDL desktop Client and Linux Host. It has not
been released or installed. The broker and workstation access policy are unchanged.

The pass addresses delayed interaction, competing mouse movements, missing glyphs
in the sharing indicator, and visibility of the active desktop mouse.

- Authorized joins check status immediately, then at most four times per second
  while approval is pending. A pending request alone does not pause existing
  participants. The Host suspends input before permission buttons become usable.
- A panel opened for consent closes when its final pending decision is reflected
  by the Host. Explicit permission management remains suspended until Done.
  The automatic-access participant list is read-only and never pauses input.
- The Host keeps rapid mouse movement with one participant until 250 ms idle.
  Drags retain ownership until every accepted button is released. Disconnect,
  suspension and downgrade release that ownership immediately; denied movement
  does not extend it. Held-key releases remain possible during another user's drag.
- The sharing badge uses a Unicode-capable system font and a compact background.
  Long names are bounded and elided. The rendered preview below uses fictional text.
- A cyan **Remote** marker shows the Host's active desktop mouse separately from
  the local cursor. It never warps the local mouse or captures keyboard/clicks.
  It follows the existing packed-display, scaling and letterbox mapping, hides
  during local movement or overlap, and remains visible in view-only use. Focus
  loss, reconnect and teardown hide it. This is the active desktop pointer,
  rather than an independent named pointer for every participant.

The existing collaboration feature and cursor-position wire format are sufficient.
Raw-HID tablet forwarding retains its owner-only path. The Qt session experiment
has a separate cursor renderer and is outside this SDL presentation change.

## Rendered previews

These are actual rendered UI assets from the Client's focused test, using a
fictional participant name. They demonstrate the glyphs and marker artwork;
they are not a live workstation or two-person streaming acceptance test.

![Sharing badge with Unicode text](session-sharing-hud.png)

![Host mouse marker](session-sharing-pointer.png)

## Verification and remaining acceptance

The Linux component fixture executes production input and retained-state code
against its fake virtual-HID backend. It includes regressions for independent
participants with equal stream lease counters, refused drag stealing, unsolicited
release, disconnect handoff and held-key cleanup. The GTK fixture exercises real
widgets under a disposable X server and verifies that permission controls wait
for Host acknowledgement and that read-only lists do not pause input.

The Mac candidate must pass the full maintained SDK27/macOS15-minimum build,
Client tests, final binary target checks, parent CI and privacy checks. Current candidate receipts: 36 Qt test invocations / 728 cases, 87 Linux
component cases, six actual GTK tests and 117 parent CI cases pass. The final
application target check confirms arm64 with minimum macOS 15.0. These are
build/component checks, separate from the remaining desktop acceptance.

Before release, exercise two actual accounts on one workstation with mouse and
keyboard input, view/control transitions, multi-display resize, focus changes,
rapid join/leave, lock/disconnect and an in-progress drag. Native Windows pointer
presentation and mixed owner-tablet/guest-mouse use also require hands-on checks.
The automatic-access policy must not be confused with owner-consent mode. Source
and package tests do not establish those desktop/hardware outcomes.
