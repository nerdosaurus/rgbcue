# RGBCUE

A desktop app for configuring RGB keyboards and mice on Linux, in the spirit of the
vendor utilities that only ship for Windows. You pick colours and effects per zone,
remap keys and mouse buttons, set mouse speed and polling rate, and keep several
profiles — one per game, or one for work.

There is no installer and no build step. Everything needed to draw the window is
fetched once, checked, and kept inside `vendor/`.

## Run it

```
./rgbcue.sh
```

or

```
python3 main.py
```

On first run it downloads Pillow (drawing) and python-xlib (the X11 window) into
`vendor/`, checking each file against a published SHA-256 first. If the computer is
offline and they are not already there, it says so plainly instead of failing
silently.

The window opens at 1260x800. Everything is drawn with Pillow, so it looks the same
at any size and needs no GTK, Qt or Tk.

### Useful switches

| Switch | What it does |
| --- | --- |
| `--screenshot FILE` | Draw the interface off-screen, save it as a PNG, exit. Needs no display, so it works over SSH and in containers. |
| `--frames N` | How many frames to render before the screenshot (default 120). |
| `--page NAME` | Start on `dashboard`, `actions`, `profiles`, `settings` or `device`. |
| `--device mouse-1` | Start on that device (`kb-1`, `mouse-1`). |
| `--tab lighting` | Start on a tab: `lighting`, `keys`, `performance`, `buttons`, `settings`. |
| `--list-devices` | List the Corsair devices found in the system device list, then exit. |
| `--selftest` | Draw one frame and report its size and colour count. |
| `--audit` | Draw every page and every tab, and report anything cut off, off-screen or overlapping. |
| `--clean` | Ignore saved settings for this run. |
| `--set page=profiles` | Apply a change before drawing, e.g. `--set effect=solid --set brightness=40`. |

## What you can do

**Lighting.** Choose an effect (static, solid, gradient, breathing, rainbow, ripple,
spinner, colour pulse, rain), set the colour and brightness, and turn individual
zones on or off. The lighting page shows a live preview strip of the zones.

**Key remaps** (keyboard). Click a key on the board, then give it a new job:
another key, a mouse button, a media key, a macro, a program or URL to launch, or
disable it. Assigned keys are marked on the board, and unassigned ones go back with
one press. The keyboard is drawn as a full-size ANSI board (main block, navigation
cluster, number pad) with real key widths.

**Performance** (mouse). Up to five DPI stages with a colour each, active-stage
selection, pointer acceleration, angle snapping, lift-off height and polling rate.

**Buttons** (mouse). Click a button in the picture, then give it an action the same
way a keyboard key can be given one.

**Profiles and hardware profiles.** Rename, duplicate, delete and switch profiles.
Each profile keeps its own devices, zones, effects, remaps and DPI stages. Hardware
profiles store a copy of the settings inside the device itself — that part is
modelled in the app, not written to a device.

**Demo devices.** If no Corsair hardware is found, the app shows a sample keyboard
and mouse so everything is still usable. `--list-devices` also detects real Corsair
devices (vendor id `1b1c`) by reading `/sys/bus/hid/devices/*/uevent`, read-only.

## Where settings are kept

```
$XDG_CONFIG_HOME/rgbcue/library.json          (or ~/.config/rgbcue/library.json)
```

Written atomically (a temporary file, then a rename) so a crash mid-save cannot
leave a half-written file. Delete it, or run with `--clean`, to start over.

## One honest limitation

**Settings are saved, but they are not sent to the hardware.** This app does not
implement the USB HID report protocol each Corsair model uses, and doing that
per-model, with no device here to test against, would be guesswork. So the app is a
faithful configuration surface: everything you change is stored and shown, and the
lighting-percentage and zone state are real, but a connected keyboard will keep
whatever lighting it already had.

Reading is real: `detect_hardware()` lists Corsair devices from the kernel's device
list. Writing is the missing half, and it is a self-contained next step — the app
already knows which zone, effect and colour it would send.

## Why Python for the current UI (Rust port underway)

The brief assumed Rust with `eframe`/`egui`. When this app was first built, the
machine had no Rust toolchain or C compiler, so a Rust build was impossible and the
UI was written in Python 3 with Pillow for drawing. That constraint is gone: the
machine now has Rust 1.99.1 and the full C toolchain, and the `app/` crate (eframe
0.37 + egui, in `src/main.rs` + `Cargo.toml`) builds cleanly.

The Python app remains the working UI for now; the Rust crate is the start of the
port. Keeping the Python version is still useful for the automated checks below,
since `--screenshot` and `--audit` run with no display at all.

## Layout of the code

| File | Its job |
| --- | --- |
| `main.py` | Command line, then either draw (screenshot/audit) or open the window. |
| `rgbcue/deps.py` | Fetches, checks and unpacks Pillow and python-xlib into `vendor/`. |
| `rgbcue/model.py` | Data: devices, zones, effects, bindings, profiles; save and load; hardware detection. |
| `rgbcue/theme.py` | Colours, fonts and the `Canvas` drawing helpers (buttons, sliders, icons, toggles). |
| `rgbcue/ui.py` | Every page and tab, and all the input handling. |
| `rgbcue/window.py` | The X11 window: blits each frame, forwards mouse and key events. |
| `tools/check.py` | Clicks every control on every page and reports failures. |
| `tools/behaviour.py` | Checks that actions actually change the model, and that saving round-trips. |
| `tools/view.py` | Prints a PNG as an ASCII brightness map, for looking at a screenshot without an image viewer. |

## Checking it

```
python3 main.py --audit          # every page drawn, nothing clipped or overlapping
python3 main.py --selftest       # one frame, with its colour count
python3 tools/check.py           # clicks every control on every page
python3 tools/behaviour.py       # actions change the right things; save/load round-trips
python3 main.py --screenshot shots/dashboard.png
```
