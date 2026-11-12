import argparse
import ctypes
import ctypes.util
import os
import subprocess
import sys
import time

from PIL import Image

NOT_BUILT_EXIT = 3
WINDOW_WAIT_S = 30
ON_SCREEN_ONLY = 1
EXCLUDE_DESKTOP = 16
UTF8 = 0x08000100
SINT32 = 3

cg = ctypes.CDLL(ctypes.util.find_library("CoreGraphics"))
cf = ctypes.CDLL(ctypes.util.find_library("CoreFoundation"))
cg.CGWindowListCopyWindowInfo.restype = ctypes.c_void_p
cg.CGWindowListCopyWindowInfo.argtypes = [ctypes.c_uint32, ctypes.c_uint32]
cf.CFArrayGetCount.restype = ctypes.c_long
cf.CFArrayGetCount.argtypes = [ctypes.c_void_p]
cf.CFArrayGetValueAtIndex.restype = ctypes.c_void_p
cf.CFArrayGetValueAtIndex.argtypes = [ctypes.c_void_p, ctypes.c_long]
cf.CFDictionaryGetValue.restype = ctypes.c_void_p
cf.CFDictionaryGetValue.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
cf.CFStringCreateWithCString.restype = ctypes.c_void_p
cf.CFStringCreateWithCString.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_uint32]
cf.CFNumberGetValue.restype = ctypes.c_bool
cf.CFNumberGetValue.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p]
cf.CFRelease.argtypes = [ctypes.c_void_p]


def cf_int(dictionary, key):
    value = cf.CFDictionaryGetValue(dictionary, key)
    if not value:
        return None
    out = ctypes.c_int32()
    cf.CFNumberGetValue(value, SINT32, ctypes.byref(out))
    return out.value


def window_ids(pid):
    owner = cf.CFStringCreateWithCString(None, b"kCGWindowOwnerPID", UTF8)
    number = cf.CFStringCreateWithCString(None, b"kCGWindowNumber", UTF8)
    layer = cf.CFStringCreateWithCString(None, b"kCGWindowLayer", UTF8)
    windows = cg.CGWindowListCopyWindowInfo(ON_SCREEN_ONLY | EXCLUDE_DESKTOP, 0)
    found = []
    for i in range(cf.CFArrayGetCount(windows)):
        entry = cf.CFArrayGetValueAtIndex(windows, i)
        if cf_int(entry, owner) == pid and cf_int(entry, layer) == 0:
            found.append(cf_int(entry, number))
    cf.CFRelease(windows)
    return found


def wait_for_window(child):
    deadline = time.time() + WINDOW_WAIT_S
    while time.time() < deadline:
        if child.poll() is not None:
            return None
        ids = window_ids(child.pid)
        if ids:
            return ids[0]
        time.sleep(0.25)
    return None


def to_scene(path, size):
    width, height = size
    shot = Image.open(path).convert("RGB")
    scale = shot.width / (width + 0.0)
    if scale != 1:
        shot = shot.resize((width, round(shot.height / scale)), Image.LANCZOS)
    top = max(0, shot.height - height)
    shot.crop((0, top, width, top + height)).save(path, optimize=True)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--bin", required=True)
    p.add_argument("--url")
    p.add_argument("--bind")
    p.add_argument("--home", required=True)
    p.add_argument("--size", default="1200x800")
    p.add_argument("--theme", default="dark")
    p.add_argument("--route", required=True)
    p.add_argument("--settle", type=float, default=3.0)
    p.add_argument("--out", required=True)
    p.add_argument("--extra", action="append", default=[])
    a = p.parse_args()
    size = tuple(int(n) for n in a.size.split("x"))
    daemon = ["--url", a.url] if a.url else ["--bind", a.bind]
    cmd = [a.bin, *daemon, "--home", a.home, "--size", a.size, "--theme", a.theme, "--route", a.route, *a.extra]
    child = subprocess.Popen(cmd)
    try:
        window = wait_for_window(child)
        if window is None:
            code = child.poll()
            if code == NOT_BUILT_EXIT:
                return NOT_BUILT_EXIT
            sys.exit(f"capture_native: no window for {a.route} (native exit {code})")
        time.sleep(a.settle)
        subprocess.run(["screencapture", "-l", str(window), "-o", "-x", a.out], check=True)
        to_scene(a.out, size)
        return 0
    finally:
        if child.poll() is None:
            child.terminate()
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
        os.sync()


if __name__ == "__main__":
    sys.exit(main())
