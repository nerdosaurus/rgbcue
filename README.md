# RGBCUE

A native desktop frontend for configuring Corsair keyboards and mice on Linux,
in the spirit of the vendor utilities that only ship for Windows.

RGBCUE is a thin UI over [OpenLinkHub](https://github.com/jurkovic-nikola/OpenLinkHub),
which owns all the per-device HID/protocol work. The app only talks to OpenLinkHub's
REST API at `http://127.0.0.1:27003`, so OpenLinkHub must be installed and running
first.

### Artwork

The Scimitar (wireless and wired), Darkstar and K65 line drawings under
`assets/` are original project artwork and are compiled into the binary: the
side/top view for the dashboard card, body/button layers on the mouse device
page, and the K65 card drawing on the keyboard card. The cleaned SVG sources
live in `assets/*/svg/`; `assets/tools/render_svgs.py` regenerates the bundled
PNGs from them (needs `rsvg-convert` and ImageMagick).

Artwork extracted from Corsair's iCUE `.rcc` files is proprietary and stays
outside the repo. Devices without bundled art use the drawn icons.

## Build & run

Requires a recent Rust toolchain (Rust 1.99+ / edition 2024).

```
cargo run --release
```

The window is built with `eframe`/`egui` 0.37.

## CLI flags

| Flag | What it does |
| --- | --- |
| `--list-devices` | List the devices OpenLinkHub exposes, then exit. |
| `--rgb <serial>` | Apply the configured RGB preset to that device serial, then exit. |
| `--macros` | List macro profiles, then exit. |
| `--keys` | List available key/media/mouse input actions, then exit. |
| `--preview [page]` | Open the ported OpenLinkHub UI (live API, demo data when the daemon is offline). Pages: `dashboard` (default), `device`, `macros`, `settings`. |

## UI preview

The OpenLinkHub web UI is ported to native egui screens. It still lives behind
`--preview` while the older functional UI remains the default:

```
cargo run --release -- --preview dashboard
```

The **dashboard and sidebar are live**: device cards come from
`/api/devices/` plus per-device detail (`/api/devices/{serial}`), and monitor
cards from `/api/cpuTemp`, `/api/gpuTemp` and `/api/storageTemp` (one card per
disk); coolant and fan speed stay on their device cards. If OpenLinkHub is unreachable
the preview falls back to demo data and the status pill reads **Offline**.

The **device page settings are live** too: profile switching, brightness,
RGB profile/effect, keyboard layout, polling rate and RGB cluster are applied
through the API. For keyboards, the device page renders the live key layout
(`DeviceProfile.Keyboards`) with per-key colours and selection, plus a toolbar
for applying a colour to the current key, row or whole keyboard
(`/api/keyboard/color`), plus keyboard profile switch/save/save-as/delete and a
basic key-assignment editor (`/api/keyboard/updateKeyAssignment`). All selects
are real dropdowns. The device page's **Effect settings** card edits the selected
profile's start/end colours, speed, direction and alternate colours, and
re-applies it. Colour swatches in the preview card remain cosmetic; gradient
effects keep their stops read-only for now. Key-assignment modifiers, toggle
delay and retain-original are included; layers are not wired yet.

Devices can be added to or removed from the dashboard from the dashboard's add
tile or the device page. Dashboard cards can be reordered by dragging their grip
handle. The macro library (playback, actions, running status/indicator, Stop) and
the global settings are wired to the API as well.

## What you can do

- **Devices** — see the Corsair devices OpenLinkHub detects.
- **Lighting** — effect selection, effect colours/speed/direction, brightness,
  per-zone colours on mice and per-key colours on keyboards.
- **Macros** — create, edit, reorder and delete macros, with actions for keys,
  media, mouse buttons, text, delays and mouse movement, plus repeat, loop and
  hold.
- **Text typing** — per-macro typing delay and jitter (0 keeps the daemon default).
- **Capture** — record the mouse position and assign move/teleport actions, armed
  with a hotkey that works regardless of window focus.

## Configuration

The app keeps no local configuration. Device, macro, profile and global settings
are read from and written to OpenLinkHub over its REST API; only the CLI flags
above affect startup.
