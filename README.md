# RGBCUE

A native desktop frontend for configuring Corsair keyboards and mice on Linux,
in the spirit of the vendor utilities that only ship for Windows.

RGBCUE is a thin UI over [OpenLinkHub](https://github.com/jurkovic-nikola/OpenLinkHub),
which owns all the per-device HID/protocol work. The app only talks to OpenLinkHub's
REST API at `http://127.0.0.1:27003`, so OpenLinkHub must be installed and running
first.

### Artwork

The Scimitar (wireless and wired) and Darkstar line drawings under `assets/`
are original project artwork and are compiled into the binary: the side view
for the dashboard card, and body/button layers on the mouse device page.

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
| `--preview [page]` | Open the ported OpenLinkHub UI shell (mock data, no API). Pages: `dashboard` (default), `device`, `macros`, `settings`. |

## UI preview

The OpenLinkHub web UI is being ported to native egui screens. The shell is
available before it is fully wired up:

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
are real dropdowns. Colour swatches and effect speed remain preview-only;
key-assignment modifiers, toggle delay and retain-original are included; layers
are not wired yet.

Devices can be added to or removed from the dashboard from the dashboard's add
tile or the device page. Dashboard cards can be reordered by dragging their grip
handle. Macros and settings are still visual previews with mock data.

## What you can do

- **Devices** — see the Corsair devices OpenLinkHub detects.
- **Lighting** — solid colour and RGB presets per device/zone.
- **Macros** — create, edit, reorder and delete macros, with actions for keys,
  media, mouse buttons, text, delays and mouse movement, plus repeat, loop and
  hold.
- **Text typing** — configure typing delay and jitter.
- **Capture** — record the mouse position and assign move/teleport actions, armed
  with a hotkey that works regardless of window focus.

## Configuration

App settings are kept under the platform config directory (via the `dirs` crate).
Device, macro and profile data live in OpenLinkHub's own database.
