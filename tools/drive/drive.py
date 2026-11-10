"""Drives the game's real window on a private Xvfb with fake keys and mouse (XTEST).
usage: drive.py OUTDIR -- game args…   Steps are below; each prints what it did."""
import os, subprocess, sys, time
from Xlib import X, XK, display
from Xlib.ext import xtest
out = sys.argv[1]; game_args = sys.argv[sys.argv.index('--') + 1:]
DISP = ':7'
xvfb = subprocess.Popen(['Xvfb', DISP, '-screen', '0', '1600x900x24', '-nolisten', 'tcp'], stderr=subprocess.DEVNULL)
time.sleep(1.0)
env = dict(os.environ, DISPLAY=DISP, VK_ICD_FILENAMES='/usr/share/vulkan/icd.d/lvp_icd.json')
open(f'{out}/game.log', 'w').close(); log = open(f'{out}/game.log', 'a')
game = subprocess.Popen(['target/release/cube_game', '--trace'] + game_args, env=env, stdout=log, stderr=subprocess.STDOUT)
d = display.Display(DISP)
def key(name, down):
    xtest.fake_input(d, X.KeyPress if down else X.KeyRelease, d.keysym_to_keycode(XK.string_to_keysym(name))); d.sync()
def hold(names, seconds):
    for n in names: key(n, True)
    time.sleep(seconds)
    for n in names: key(n, False)
def tap(name): hold([name], 0.08)
def mouse(dx, dy, steps=20):
    for _ in range(steps):
        xtest.fake_input(d, X.MotionNotify, detail=True, x=dx // steps, y=dy // steps); d.sync(); time.sleep(0.02)
def shot(name): subprocess.run(['import', '-display', DISP, '-window', 'root', f'{out}/{name}.png'])
def mark(text):
    log.flush(); print(text, flush=True)
    with open(f'{out}/game.log', 'a') as f: f.write(f'## {text}\n')
try:
    time.sleep(float(os.environ.get('BOOT', '25')))
    xtest.fake_input(d, X.MotionNotify, x=800, y=450); d.sync(); time.sleep(0.3)
    # No window manager here, so nothing gives the window the keyboard: do it by hand.
    win = max(d.screen().root.query_tree().children, key=lambda w: w.get_geometry().width)
    win.set_input_focus(X.RevertToParent, X.CurrentTime); d.sync(); time.sleep(0.5)
    for line in os.environ['STEPS'].split(';'):
        line = line.strip()
        if not line: continue
        mark(line); eval(line)
finally:
    game.terminate(); time.sleep(0.5); xvfb.terminate()
