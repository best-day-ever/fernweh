# Packaged SDL Cocoa input patch

`0001-preserve-posted-pointer-position.patch` applies to the pinned SDL3.4.2
sources. It preserves native event coordinates for input posted by another
process, including remote-control applications running a nested Client session.
Clicks without a preceding motion update SDL's absolute pointer before button
delivery. Posted drags also retain their button state instead of reconciling it
against the unrelated global pressed-button query. Physical stale-button
recovery, relative coordinates and other platforms retain upstream behavior.

Physical events retain SDL's macOS26 fullscreen position workaround from
[upstream commit c39d772](https://github.com/libsdl-org/SDL/commit/c39d772a07b57e841be4f46e81f72b4d3b6316a8).
The distinction uses the public Quartz event source process field, also used
for posted-event handling by [Chromium's Cocoa view](https://chromium.googlesource.com/experimental/chromium/src/+/HEAD/content/app_shim_remote_cocoa/render_widget_host_view_cocoa.mm).
This selects coordinates only; it grants no input permission or trust.

`scripts/build/ensure-macos-client-sdl.sh` applies the patch and incrementally
builds SDL. Both bootstrap and the Client build call it, so a retained unpatched
dependency cache cannot silently supply the packaged library. Its stamp includes
the patch hash, deployment target and installed library hash; failed builds
remove the stamp. Unknown source modifications fail before patching/building.

The focused test compiles the actual patched coordinate policy with native
Quartz events without posting input or creating a window, and tests dependency
rebuild/reuse/failure behavior with an isolated fake builder:

```bash
agent-heavy-run python3 tests/packaging/test-macos-sdl-posted-pointer.py \
  "$PLANK_MAC_CLIENT_DEPS/src/SDL3-3.4.2"
```

Installed acceptance must cover physical and nested remote pointer movement,
dragging, left/right clicks, a click without prior motion, and corner/center
positions. Repeat in fullscreen/windowed mode and after a fullscreen Space
switch. Component checks do not establish a remote-control application's
event-source behavior or replace this live qualification.
