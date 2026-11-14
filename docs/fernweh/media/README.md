# README images

The two PNGs are captures of **Fernweh's actual QML display setup views**, rendered
on macOS with Qt 6.10.2 and a deliberately fictional `DisplaySetup` model. They
are not illustrations of a proposed interface or captures of a live session.
The banner is a separate original SVG illustration.

| Asset | What it shows |
| --- | --- |
| [display-matcher.png](display-matcher.png) | Real `DisplaySetupDialog` overview: 4K primary, exact-size virtual MacBook display, portrait monitor kept local |
| [display-advanced.png](display-advanced.png) | Real dialog's advanced page: manual arrangement, exact dimensions/coordinates, output backing and presentation |
| [fernweh-banner.svg](fernweh-banner.svg) | Decorative Fernweh README header; no product UI or capability evidence |

## Source and privacy

Captured 14 November 2026 from Client
`19adb7e8247da99f5dbe5c2e99e4f5b3d0560bb7` with broker support disabled.
`deployment/configure.py` generates the same selected QRC/shared QML used by the
product. The capture program loads that resource's `DisplaySetupDialog.qml`,
`DisplaySetupPanel.qml`, `DisplayAdvancedPage.qml`, theme and controls unchanged.

[DisplayFixture.qml](capture/DisplayFixture.qml) provides static model data in
place of `DisplaySetupController`; [Capture.qml](capture/Capture.qml) supplies a
plain window. No network, actual display probe, account, preference store or
Host process is used. The model is illustrative and is not evidence that the
planner/Host produced or qualified that layout. No user names, addresses,
credentials or artist work appear in the images.

Both pages were visually inspected after rendering. They demonstrate UI/source
presence, not installed-session acceptance. The dimensions are 1840 × 1360 for
the overview and 1840 × 1800 for the advanced page, captured at 2× scale without
postprocessing. PNGs contain image data only, without embedded text metadata.

## Reproduce

Initialize the pinned Client. Use the platform's pinned Qt 6.10.2 toolchain;
the original captures use Apple Silicon and SDK 27 with a macOS 15 target.
No product build, dependency install or broker endpoint is needed.

```sh
QMAKE=/path/to/Qt/6.10.2/macos/bin/qmake \
  bash docs/fernweh/media/capture/render.sh "$PWD/apps/client"
```

On the shared BDE development Mac, put `agent-heavy-run` before `bash`.
The script builds a small Qt capture program in its own temporary directory,
uses the offscreen/software scenegraph, writes both PNGs here and removes its
temporary build on exit. The two PNGs are intentional documentation assets;
executables and generated QRC/build files do not belong in Git.

When changing the pinned Client or views, render again, inspect both full
images, and update this provenance. Preserve honest sample-data captions.
