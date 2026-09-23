# macOS multi-monitor presentation regression

The older Linux "Match my displays" connection path resolved the remote
desktop sizes but skipped local presentation planning. On macOS this left
both remote desktops in one SDL window even when the display setup showed
"One fullscreen window per screen". The planner's older-host fallback also
returned before applying its presentation selection.

Both paths now use the same display planner and window selection as display
arrangements. Saved older layouts retain their qualified remote modes; they
also get one local window per selected monitor. Monitors excluded by the
plan remain local, and each window uses its remote display's source rectangle
and desktop coordinates for rendering and pointer input.

On macOS, a windowed launch prepares the secondary windows hidden. Entering
fullscreen through the toolbar or the native green button activates the
same selection. Native fullscreen events update the session presentation
state and rebuild the renderer, rather than only updating window geometry.
Leaving fullscreen hides the secondary windows and returns to one window.
Native fullscreen on separate monitors continues to require macOS's
"Displays have separate Spaces" setting; the planner retains its warning
and combined-window behavior when that setting is off.

macOS startup now enables SDL's automatic mouse capture. A stale SDL2 hint
disabled it even though the manual capture-on-leave workaround had already
been removed; SDL could therefore clamp and stop motion at the window edge.
Automatic capture keeps a held-button drag addressed to its starting window. On macOS, pointer events are now routed through the live
logical window bounds before mapping to the host desktop. Crossing to a
second display preserves native capture and the remote button state, rather
than clamping motion to the first display's edge. Button release also sends
its final mapped position before releasing the remote button.

## Verification

The local two-monitor candidate has been confirmed to show separate remote
desktops with working input on both. The capture-enabled build records continuous physical held-button motion
across both display windows in both directions, and the user confirmed that
remote windows follow correctly across the boundary. The complete native
fullscreen transition matrix has not been manually exercised.

- The Mac development build and its bundled Qt suites pass, including new
  display planner regressions for older-host window selection, excluded
  monitors, differing display enumeration order, explicit combined-window
  presentation and the separate-Spaces constraint.
- Captured-drag geometry tests cover both directions across a display seam,
  fractional Retina coordinates, negative and vertical display positions,
  and local gaps/hidden windows.
- Live qualification must verify two distinct macOS stream windows, one per
  connected monitor, using both a saved older Match client layout and an
  arrangement profile. Check the logs for two presentation outputs and two
  Metal targets; a single wide window containing both desktops is a failure.
- Enter and leave fullscreen using the primary window's native green button
  and the streaming toolbar, including a session started windowed. Verify
  that the second window returns and shows only its own remote monitor.
- Verify pointer placement and a drag between remote displays, reconnect,
  and disconnect cleanup. The automated planner/geometry tests do not prove
  AppKit Space transitions or physical pointer input.
