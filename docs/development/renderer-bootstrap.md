# SDL renderer startup

The SDL session must initialize its live decoder from decoder state, without
requiring a `Shown`, pixel-size or display-change event. The initial native
surface can otherwise remain unpainted while transport continues receiving
video. Windows fullscreen transitions also destroy the decoder before changing
window state; a missing resize notification must not strand that session.

`PlankSessionEvents::waitForEvent` dispatches a renderer reset directly when a
live session has a window and no decoder. It defers while minimized, and the
caller excludes transport reconnects. The reset does not need room in SDL's
queue and leaves queued control and close events intact. Once the decoder
exists, ordinary event dispatch resumes. Deferred Wayland mapping uses the same
bootstrap and still maps only after decoder selection.

The `rendererbootstrap` native SDL suite removes initial window notifications
and demonstrates that the previous event wait has nothing to dispatch. The
state-driven path initializes a real software renderer, verifies its pixels,
and presents on the same surface. It also covers decoder teardown without a
subsequent resize, preserved control/close events and a saturated event queue.
The maintained Windows and Mac runners include the suite. A rejected or
filtered window-flush barrier does not increment its pending count; completion
is bounded at zero. Both producer and consumer run on the session event thread.
The regression covers queue-full/filter rejection and subsequent recovery. This regression
does not claim to reproduce a particular graphics driver's notification loss
or qualify hardware decoding and full streaming on every Windows GPU.

Run the focused suite with the platform's pinned Qt and SDL dependencies:

```sh
qmake apps/client/tests/rendererbootstrap/rendererbootstrap.pro CONFIG+=release
make
SDL_VIDEODRIVER=dummy SDL_RENDER_DRIVER=software ./rendererbootstrap
```

On Windows use the native MSVC qmake/nmake environment; omit `SDL_VIDEODRIVER`
to exercise the Windows window backend. Keep live login, reconnect, software
and hardware profile selection, fullscreen and input checks separate from
this event/bootstrap regression.
