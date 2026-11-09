#!/usr/bin/env python3
"""Compile the actual patched pointer policy and exercise retained SDL builds.

The source argument is the pinned SDL 3.4.2 source tree. No window, event tap,
input posting, permissions or shared dependency cache is touched by these tests.
"""
from pathlib import Path
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SDL = Path(sys.argv.pop(1))
SOURCE = Path('src/video/cocoa/SDL_cocoawindow.m')
MOUSE_SOURCE = Path('src/video/cocoa/SDL_cocoamouse.m')
PATCH = Path('packaging/client/macos/sdl-patches/0001-preserve-posted-pointer-position.patch')


class PostedPointer(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='plank-sdl-')
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.root = self.base / 'root'
        self.deps = self.base / 'deps'
        self.source = self.deps / 'src/SDL3-3.4.2'
        (self.source / SOURCE).parent.mkdir(parents=True)
        shutil.copyfile(SDL / SOURCE, self.source / SOURCE)
        shutil.copyfile(SDL / MOUSE_SOURCE, self.source / MOUSE_SOURCE)
        (self.source / 'CMakeLists.txt').touch()
        for relative in (PATCH, Path('scripts/build/ensure-macos-client-sdl.sh'),
                         Path('scripts/build/macos-client-target.sh'), Path('scripts/build/build-paths.sh')):
            (self.root / relative).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / relative, self.root / relative)
        self.bin = self.deps / 'install/bin'
        self.bin.mkdir(parents=True)
        self.library = self.deps / 'install/lib/libSDL3.0.dylib'
        self.stamp = self.deps / 'build-SDL3-3.4.2/.plank-posted-pointer-build'
        self.log = self.base / 'cmake-calls'
        self.env = dict(os.environ, PLANK_MAC_CLIENT_DEPS=str(self.deps),
                        TEST_CMAKE_LOG=str(self.log), TEST_LIBRARY=str(self.library))
        fake = self.bin / 'cmake'
        fake.write_text('''#!/usr/bin/env bash
set -euo pipefail
printf '%s\\n' "$*" >> "$TEST_CMAKE_LOG"
if [[ $1 == --build && ${TEST_FAIL_BUILD:-0} == 1 ]]; then exit 9; fi
if [[ $1 == --install ]]; then
    mkdir -p "$(dirname "$TEST_LIBRARY")" "$2"
    printf 'rebuilt SDL\\n' > "$TEST_LIBRARY"
fi
''')
        fake.chmod(0o755)

    def ensure(self, success=True):
        result = subprocess.run(['bash', str(self.root / 'scripts/build/ensure-macos-client-sdl.sh'),
                                 str(self.root)], env=self.env, capture_output=True, text=True)
        if success:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0)
        return result

    def test_missing_stamp_builds_once_and_reuses_verified_library(self):
        self.ensure()
        self.assertTrue(self.stamp.exists())
        first_calls = self.log.read_text()
        self.ensure()
        self.assertEqual(self.log.read_text(), first_calls)

    def test_changed_patch_rebuilds_only_sdl(self):
        self.ensure()
        with (self.root / PATCH).open('a') as patch:
            patch.write('\n')
        self.ensure()
        calls = self.log.read_text()
        self.assertEqual(calls.count('--install '), 2)
        self.assertNotIn('ffmpeg', calls)

    def test_replaced_library_forces_rebuild(self):
        self.ensure()
        self.library.write_text('old or replaced library')
        self.ensure()
        self.assertEqual(self.library.read_text(), 'rebuilt SDL\n')
        self.assertEqual(self.log.read_text().count('--install '), 2)

    def test_failed_rebuild_removes_success_stamp(self):
        self.ensure()
        self.library.write_text('old library')
        self.env['TEST_FAIL_BUILD'] = '1'
        self.ensure(success=False)
        self.assertFalse(self.stamp.exists())
        self.assertEqual(self.library.read_text(), 'old library')
        del self.env['TEST_FAIL_BUILD']
        self.ensure()
        self.assertTrue(self.stamp.exists())

    def test_unknown_sources_fail_before_build(self):
        (self.source / SOURCE).write_text('unknown source\n')
        self.ensure(success=False)
        self.assertFalse(self.log.exists())

    def test_production_posted_event_coordinates_and_physical_fallback(self):
        self.ensure()
        source = (self.source / SOURCE).read_text()
        # Compile the production helper from the external patch, rather than
        # a duplicate policy. SDL's complete Cocoa TU is checked by its build.
        start = source.index('static bool Cocoa_GetPostedMousePosition(')
        end = source.index('// PLANK posted-pointer policy end.', start)
        policy = source[start:end]
        mouse_source = (self.source / MOUSE_SOURCE).read_text()
        start = mouse_source.index('static void Cocoa_ReconcileButtonState(')
        end = mouse_source.index('\nvoid Cocoa_HandleMouseEvent(', start)
        # Replace only the asynchronous OS query with a deterministic test
        # boundary; the actual SDL reconciliation function is compiled intact.
        reconcile = mouse_source[start:end].replace('[NSEvent pressedMouseButtons]',
                                                    'globalButtons')
        program = self.base / 'posted-pointer.mm'
        program.write_text('''#import <AppKit/AppKit.h>
#include <cassert>
#include <initializer_list>
#include <unistd.h>
@interface PointerEvent : NSEvent {
@public CGEventRef quartz;
@public NSPoint point;
}
@end
@implementation PointerEvent
- (CGEventRef)CGEvent { return quartz; }
- (NSPoint)locationInWindow { return point; }
- (NSTimeInterval)timestamp { return 1; }
@end
using Uint32 = unsigned int;
using Uint8 = unsigned char;
static Uint32 globalButtons;
static int releasedButtons;
static const int SDL_BUTTON_LEFT = 1, SDL_GLOBAL_MOUSE_ID = 0;
static Uint32 SDL_GetMouseState(void*, void*) { return 3; }
static double Cocoa_GetEventTimestamp(double time) { return time; }
static void* SDL_GetMouseFocus() { return nullptr; }
static void SDL_SendMouseButton(double, void*, int, Uint8 button, bool down) {
    assert(!down);
    releasedButtons |= 1 << (button - 1);
}
''' + policy + reconcile + '''
int main() {
    @autoreleasepool {
        PointerEvent *event = [PointerEvent new];
        float x = -7, y = -8;
        // Unknown event sources retain upstream behavior without changing x/y.
        assert(!Cocoa_GetPostedMousePosition(event, 900, &x, &y));
        assert(x == -7 && y == -8);
        event->quartz = CGEventCreateMouseEvent(nullptr, kCGEventMouseMoved,
                                               CGPointMake(0, 0), kCGMouseButtonLeft);
        assert(event->quartz);
        CGEventSetIntegerValueField(event->quartz, kCGEventSourceUnixProcessID, 0);
        globalButtons = 0;
        Cocoa_ReconcileButtonState(event);
        assert(releasedButtons == 3); // Physical stale-state recovery remains.
        releasedButtons = 0;
        globalButtons = 1;
        Cocoa_ReconcileButtonState(event);
        assert(releasedButtons == 0); // A physically held button stays held.
        globalButtons = 0;
        CGEventSetIntegerValueField(event->quartz, kCGEventSourceUnixProcessID, 42);
        Cocoa_ReconcileButtonState(event);
        assert(releasedButtons == 0); // Posted drag does not release remotely held buttons.
        for (int pid : {0, -1}) {
            CGEventSetIntegerValueField(event->quartz, kCGEventSourceUnixProcessID, pid);
            assert(!Cocoa_GetPostedMousePosition(event, 900, &x, &y));
            assert(x == -7 && y == -8);
        }
        // Remote and in-process posts both carry their own position. Exercise
        // motion, drag, and click-only input at corners/center and outside the
        // window; the downstream image mapper, not this policy, clamps edges.
        for (int pid : {42, int(getpid())}) {
            CGEventSetIntegerValueField(event->quartz, kCGEventSourceUnixProcessID, pid);
            for (CGEventType type : {kCGEventMouseMoved, kCGEventLeftMouseDragged,
                                    kCGEventRightMouseDragged, kCGEventOtherMouseDragged,
                                    kCGEventLeftMouseDown, kCGEventLeftMouseUp,
                                    kCGEventRightMouseDown, kCGEventRightMouseUp}) {
                CGEventSetType(event->quartz, type);
                for (NSPoint point : {NSMakePoint(0, 0), NSMakePoint(1600, 900),
                                     NSMakePoint(800.5, 450.25), NSMakePoint(-20, 950)}) {
                    event->point = point;
                    assert(Cocoa_GetPostedMousePosition(event, 900, &x, &y));
                    assert(x == point.x && y == 900 - point.y);
                }
            }
        }
        CFRelease(event->quartz);
    }
}
''')
        binary = self.base / 'posted-pointer'
        result = subprocess.run(['xcrun', 'clang++', '-std=c++17', '-fobjc-arc',
                                 '-mmacosx-version-min=15.0', '-Wall', '-Wextra', '-Werror',
                                 str(program), '-framework', 'AppKit', '-framework', 'CoreGraphics',
                                 '-o', str(binary)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        subprocess.run([str(binary)], check=True)


if __name__ == '__main__':
    unittest.main()
