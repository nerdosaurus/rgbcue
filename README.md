# RGBCUE

A native desktop frontend for configuring Corsair keyboards and mice on Linux,
in the spirit of the vendor utilities that only ship for Windows.

RGBCUE is a thin UI over [OpenLinkHub](https://github.com/jurkovic-nikola/OpenLinkHub),
which owns all the per-device HID/protocol work. The app only talks to OpenLinkHub's
REST API at `http://127.0.0.1:27003`, so OpenLinkHub must be installed and running
first.

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
available before it is wired up:

```
cargo run --release -- --preview dashboard
```

It uses mock devices and local widget state, and never contacts OpenLinkHub.
Implemented screens: dashboard, device (settings + lighting), macros, settings,
plus placeholder pages for the rest of the sidebar. The old functional UI is
still the default until the port replaces it.

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
