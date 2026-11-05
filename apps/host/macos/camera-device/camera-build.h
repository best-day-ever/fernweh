// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
// The signed Host build explicitly chooses whether to ship its camera extension.
// Camera support must not be inferred from signing credentials being unavailable.
#ifndef PLANK_MACOS_CAMERA_EXTENSION
#define PLANK_MACOS_CAMERA_EXTENSION 1
#endif
#if PLANK_MACOS_CAMERA_EXTENSION != 0 && PLANK_MACOS_CAMERA_EXTENSION != 1
#error PLANK_MACOS_CAMERA_EXTENSION must be 0 or 1
#endif
