#!/usr/bin/env python3
"""Read Quartz display bounds, pixel modes and cursor position without posting input.

Run on a Mac, or send this file to its Python interpreter over SSH stdin. No
screen pixels, settings or persistent target files are read or written.
"""
import argparse
import ctypes as c
import json
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--cursor-only', action='store_true')
parser.add_argument('--watch-seconds', type=float, default=0, help='Watch cursor changes for 1 through 60 seconds')
parser.add_argument('--interval', type=float, default=.05, help='Polling interval, .02 through 1 second')
args = parser.parse_args()
if args.watch_seconds != 0 and not 1 <= args.watch_seconds <= 60:
    parser.error('--watch-seconds must be 1 through 60')
if not .02 <= args.interval <= 1:
    parser.error('--interval must be .02 through 1 second')
cg = c.CDLL('/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics')
cf = c.CDLL('/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation')


class Point(c.Structure):
    _fields_ = [('x', c.c_double), ('y', c.c_double)]


class Size(c.Structure):
    _fields_ = [('width', c.c_double), ('height', c.c_double)]


class Rect(c.Structure):
    _fields_ = [('origin', Point), ('size', Size)]


cg.CGDisplayBounds.argtypes = [c.c_uint32]
cg.CGDisplayBounds.restype = Rect
cg.CGDisplayCopyDisplayMode.argtypes = [c.c_uint32]
cg.CGDisplayCopyDisplayMode.restype = c.c_void_p
for name in ['CGDisplayModeGetWidth', 'CGDisplayModeGetHeight',
             'CGDisplayModeGetPixelWidth', 'CGDisplayModeGetPixelHeight']:
    function = getattr(cg, name)
    function.argtypes = [c.c_void_p]
    function.restype = c.c_size_t
cg.CGEventCreate.argtypes = [c.c_void_p]
cg.CGEventCreate.restype = c.c_void_p
cg.CGEventGetLocation.argtypes = [c.c_void_p]
cg.CGEventGetLocation.restype = Point
cf.CFRelease.argtypes = [c.c_void_p]
result = {}
if not args.cursor_only and not args.watch_seconds:
    displays = (c.c_uint32 * 32)()
    count = c.c_uint32()
    if cg.CGGetActiveDisplayList(32, displays, c.byref(count)):
        raise RuntimeError('Display enumeration failed')
    items = []
    for identifier in displays[:count.value]:
        bounds = cg.CGDisplayBounds(identifier)
        mode = cg.CGDisplayCopyDisplayMode(identifier)
        item = {'id': identifier, 'bounds': [bounds.origin.x, bounds.origin.y,
            bounds.size.width, bounds.size.height],
            'main': bool(cg.CGDisplayIsMain(identifier)),
            'mirror_of': cg.CGDisplayMirrorsDisplay(identifier)}
        if mode:
            item['mode'] = {name: getattr(cg, 'CGDisplayModeGet' + name)(mode)
                for name in ['Width', 'Height', 'PixelWidth', 'PixelHeight']}
            cf.CFRelease(mode)
        items.append(item)
    result['active_displays'] = items
def read_cursor():
    current = cg.CGEventCreate(None)
    if not current:
        raise RuntimeError('Cursor query failed')
    position = cg.CGEventGetLocation(current)
    cf.CFRelease(current)
    return [position.x, position.y]


if args.watch_seconds:
    deadline = time.monotonic() + args.watch_seconds
    previous = None
    while time.monotonic() < deadline:
        point = read_cursor()
        if point != previous:
            print(json.dumps({'monotonic_ns': time.monotonic_ns(), 'cursor': point}), flush=True)
            previous = point
        time.sleep(args.interval)
else:
    result['cursor'] = read_cursor()
    print(json.dumps(result))
