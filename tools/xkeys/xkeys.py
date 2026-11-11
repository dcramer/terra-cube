#!/usr/bin/env python3
"""Real key presses and mouse clicks on an X display, for testing the game's window with no
person at it: `xkeys.py 'w:2 space 1 i:0.5 click wait:1'`. Each word is a key (held for the
seconds after the colon, 0.08 if none), `click` / `rclick`, `move:DX,DY` or `wait:SECONDS`.
Uses libX11 and libXtst through ctypes; nothing to install. DISPLAY says which display."""
import ctypes, sys, time

x11, xtst = ctypes.CDLL("libX11.so.6"), ctypes.CDLL("libXtst.so.6")
x11.XOpenDisplay.restype = ctypes.c_void_p
x11.XStringToKeysym.restype = ctypes.c_ulong
display = x11.XOpenDisplay(None)
if not display:
    sys.exit("xkeys: no display (set DISPLAY)")
display = ctypes.c_void_p(display)
# With no window manager nothing has the keyboard: give it to the window under the pointer
# (the first real window on the screen if the pointer is over none).
root = x11.XDefaultRootWindow(display)
r, child = ctypes.c_ulong(), ctypes.c_ulong()
a, b, c, d, m = ctypes.c_int(), ctypes.c_int(), ctypes.c_int(), ctypes.c_int(), ctypes.c_uint()
x11.XQueryPointer(display, ctypes.c_ulong(root), ctypes.byref(r), ctypes.byref(child), ctypes.byref(a), ctypes.byref(b), ctypes.byref(c), ctypes.byref(d), ctypes.byref(m))
if not child.value:
    parent, kids, n = ctypes.c_ulong(), ctypes.POINTER(ctypes.c_ulong)(), ctypes.c_uint()
    x11.XQueryTree(display, ctypes.c_ulong(root), ctypes.byref(r), ctypes.byref(parent), ctypes.byref(kids), ctypes.byref(n))
    child.value = kids[n.value - 1] if n.value else 0
if child.value:
    x11.XSetInputFocus(display, child, 1, 0)
    x11.XFlush(display)

NAMES = {"space": "space", "shift": "Shift_L", "ctrl": "Control_L", "esc": "Escape", "tab": "Tab", "f1": "F1"}


def key(name, down):
    sym = x11.XStringToKeysym(NAMES.get(name, name).encode())
    code = x11.XKeysymToKeycode(display, ctypes.c_ulong(sym))
    if not code:
        sys.exit(f"xkeys: no key `{name}`")
    xtst.XTestFakeKeyEvent(display, code, down, 0)
    x11.XFlush(display)


for word in " ".join(sys.argv[1:]).split():
    name, _, arg = word.partition(":")
    if name == "wait":
        time.sleep(float(arg or 1))
    elif name in ("click", "rclick"):
        button = 1 if name == "click" else 3
        xtst.XTestFakeButtonEvent(display, button, 1, 0)
        x11.XFlush(display)
        time.sleep(0.08)
        xtst.XTestFakeButtonEvent(display, button, 0, 0)
        x11.XFlush(display)
    elif name == "move":
        dx, dy = (int(v) for v in arg.split(","))
        xtst.XTestFakeRelativeMotionEvent(display, dx, dy, 0)
        x11.XFlush(display)
    else:
        key(name, 1)
        time.sleep(float(arg or 0.08))
        key(name, 0)
    time.sleep(0.05)
