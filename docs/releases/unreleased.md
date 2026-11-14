# Unreleased BDE Fernweh changes

## Immersive mouse candidate

The Mac Client offers a live-session mouse toggle, **Ctrl+Alt+Shift+M**, with
supporting X11 Linux Hosts. It forwards continuous pixel movement for Blender,
Unreal Engine and games, including tools started from the keyboard. Releasing
the mode restores ordinary pointing after an ordered Host acknowledgement.
Focus loss, reconnect, display changes and tablet takeover release capture.
See [usage](../user/immersive-mouse.md) and the
[wire contract](../../protocol/immersive-mouse.md).

This is candidate source, not a published installer. Native Mac build and
component checks pass; installed application/session acceptance is pending.

Linux Host builds now use the assigned native Rocky9.7 canary. The BDE build
entry point refuses Mac/OrbStack execution and reuses native dependency caches.

## Shared desktop wording

The shared-desktop action reads **Join session**. Its explanation, joining status
and retry messages no longer imply that every join needs the desktop owner's
approval. The Host still enforces its configured access and approval policy.

This source-only change is queued for the next Client release. It does not bump
`packaging/VERSION`, build or publish an installer, update a feed, or change the
Linux Host or broker.

Validation: the native Qt collaboration suite passed all 13 cases, including
join/cancel choices, permission handling and display placement. The parent CI
suite passed 117 cases with one platform skip. The actual QML dialog was rendered
and visually checked with generic fixture text:

![Shared desktop dialog with Join session action](../bde/screenshots/join-session.png)
