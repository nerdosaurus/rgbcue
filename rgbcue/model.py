"""What the app knows about devices, and how that is saved and loaded.

Two kinds of device are modelled, because they need different controls:

  * a keyboard - per-zone lighting plus a key map (every key, remappable)
  * a mouse    - per-zone lighting, DPI stages, polling rate, lift-off height,
                 angle snapping, and a button map

Real Corsair hardware is looked for on the machine (Linux exposes USB devices
under /sys/bus/hid).  When none is found the app runs with a demo keyboard and
mouse, so the interface can still be explored and set up.  Nothing is written to
the hardware yet: see the note in README.md.
"""

from __future__ import annotations

import copy
import json
import os
import re
from dataclasses import dataclass, field, asdict

CORSAIR_VENDOR_ID = "1b1c"

# A small table of USB product ids, used only to give a friendlier name than the
# kernel's own string.  Anything not in here keeps the name the kernel reports.
# These ids are best-effort and are not needed for the app to work.
KNOWN_PRODUCT_IDS = {
    # This machine's actual hardware (lsusb, 2026-11-08).
    "1baf": ("K65 RGB MINI 60%", "keyboard"),
    "1b8b": ("Scimitar RGB Elite", "mouse"),
    # Present, but not something this app configures: kind None -> detection skips.
    "0c10": ("Commander PRO", None),
    "0c17": ("H115i Platinum", None),
    "2b00": ("Slipstream Wireless Receiver", None),
    # Friendly names for other Corsair models (best-effort).
    "1b13": ("K70 RGB", "keyboard"),
    "1b49": ("K70 RGB MK.2", "keyboard"),
    "1b2d": ("K95 RGB Platinum", "keyboard"),
    "1b33": ("K66", "keyboard"),
    "1b3d": ("K68", "keyboard"),
    "1b55": ("K95 RGB Platinum XT", "keyboard"),
    "1b4a": ("M65 RGB Elite", "mouse"),
    "1b50": ("Harpoon RGB", "mouse"),
    "1b5e": ("Harpoon RGB Wireless", "mouse"),
    "1b17": ("M65 PRO RGB", "mouse"),
    "1b3e": ("Ironclaw", "mouse"),
}


# ---------------------------------------------------------------------------
# actions a key or button can be given
# ---------------------------------------------------------------------------
ACTION_KINDS = [
    ("default", "Default (its own function)"),
    ("disabled", "Disabled"),
    ("key", "Remap to another key"),
    ("media", "Media key"),
    ("macro", "Run a macro"),
    ("app", "Launch an application"),
    ("dpi", "Mouse: change DPI stage"),
    ("profile", "Switch profile"),
]

MEDIA_KEYS = ["Play / Pause", "Next track", "Previous track", "Stop", "Volume up",
              "Volume down", "Mute", "Calculator", "Browser back"]

REMAP_KEYS = (["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M",
               "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z",
               "0", "1", "2", "3", "4", "5", "6", "7", "8", "9"] +
              ["Esc", "Tab", "Caps Lock", "Space", "Enter", "Backspace", "Delete",
               "Insert", "Home", "End", "Page Up", "Page Down", "Up", "Down",
               "Left", "Right", "Left Shift", "Left Ctrl", "Left Alt",
               "Right Shift", "Right Ctrl", "Right Alt", "Print Screen",
               "Scroll Lock", "Pause"])


@dataclass
class Binding:
    """What one key or button does."""
    kind: str = "default"          # one of ACTION_KINDS
    detail: str = ""               # target key, macro name, app path, stage number
    label: str = ""                # how it reads in the list

    def describe(self, own_label: str) -> str:
        if self.kind == "default":
            return f"{own_label} (default)"
        if self.kind == "disabled":
            return "Disabled"
        if self.kind == "key":
            return f"Remapped to {self.detail}"
        if self.kind == "media":
            return f"Media: {self.detail}"
        if self.kind == "macro":
            return f"Macro: {self.detail}"
        if self.kind == "app":
            return f"Launch {self.detail}"
        if self.kind == "dpi":
            return f"DPI stage {self.detail}"
        if self.kind == "profile":
            return f"Profile: {self.detail}"
        return own_label


@dataclass
class Effect:
    """A lighting effect, described the way iCUE's effect picker does."""
    kind: str = "static"           # static, solid, breath, rainbow, wave, gradient, ripple, spiral
    colour: tuple[int, int, int] = (235, 64, 64)
    colour2: tuple[int, int, int] = (80, 140, 245)
    speed: int = 50                # 0-100
    brightness: int = 80           # 0-100


EFFECTS = [
    ("static", "Static colour", "One steady colour on every lit zone"),
    ("solid", "Solid", "A different steady colour per zone"),
    ("breath", "Breathing", "Pulses between two colours"),
    ("rainbow", "Rainbow", "Colours sweep through the full spectrum"),
    ("wave", "Wave", "A colour band travels across the device"),
    ("gradient", "Gradient", "Blends between two colours across the keys"),
    ("ripple", "Ripple", "A ring of light spreading from each key press"),
    ("spiral", "Spiral", "Colour spirals around the device"),
]


# ---------------------------------------------------------------------------
# keyboard layout: a full-size ANSI keyboard, as three blocks side by side.
# Each key is (label, width in units); "gap" starts a new block.
# ---------------------------------------------------------------------------
KEYBOARD_ROWS: list[list[tuple[str, float]]] = [
    # function row, navigation cluster, number pad
    [("Esc", 1), ("F1", 1), ("F2", 1), ("F3", 1), ("F4", 1), ("F5", 1), ("F6", 1),
     ("F7", 1), ("F8", 1), ("F9", 1), ("F10", 1), ("F11", 1), ("F12", 1),
     ("gap", 0.5), ("PrtSc", 1), ("ScrLk", 1), ("Pause", 1),
     ("gap", 3.0), ("NumLk", 1), ("/", 1), ("*", 1), ("-", 1)],
    [("`", 1), ("1", 1), ("2", 1), ("3", 1), ("4", 1), ("5", 1), ("6", 1), ("7", 1),
     ("8", 1), ("9", 1), ("0", 1), ("-", 1), ("=", 1), ("Backspace", 2),
     ("gap", 0.5), ("Ins", 1), ("Home", 1), ("PgUp", 1),
     ("gap", 1.0), ("7", 1), ("8", 1), ("9", 1), ("+", 1)],
    [("Tab", 1.5), ("Q", 1), ("W", 1), ("E", 1), ("R", 1), ("T", 1), ("Y", 1), ("U", 1),
     ("I", 1), ("O", 1), ("P", 1), ("[", 1), ("]", 1), ("\\", 1.5),
     ("gap", 0.5), ("Del", 1), ("End", 1), ("PgDn", 1),
     ("gap", 1.0), ("4", 1), ("5", 1), ("6", 1), ("+", 1)],
    [("Caps", 1.75), ("A", 1), ("S", 1), ("D", 1), ("F", 1), ("G", 1), ("H", 1), ("J", 1),
     ("K", 1), ("L", 1), (";", 1), ("'", 1), ("Enter", 2.25),
     ("gap", 4.5), ("1", 1), ("2", 1), ("3", 1)],
    [("LShift", 2.25), ("Z", 1), ("X", 1), ("C", 1), ("V", 1), ("B", 1), ("N", 1), ("M", 1),
     (",", 1), (".", 1), ("/", 1), ("RShift", 2.75),
     ("gap", 2.5), ("Up", 1),
     ("gap", 1.0), ("0", 2), (".", 1)],
    [("LCtrl", 1.25), ("LWin", 1.25), ("LAlt", 1.25), ("Space", 6.25), ("RAlt", 1.25),
     ("RWin", 1.25), ("Menu", 1.25), ("RCtrl", 1.25),
     ("gap", 0.5), ("Left", 1), ("Down", 1), ("Right", 1)],
]

MOUSE_BUTTONS = ["Left click", "Right click", "Middle click", "Back", "Forward",
                 "DPI up", "DPI down", "Profile switch", "Sniper"]


@dataclass
class Zone:
    """A lit part of a device. Mice and keyboards have several."""
    name: str
    on: bool = True
    colour: tuple[int, int, int] = (235, 64, 64)


@dataclass
class Device:
    id: str
    name: str
    kind: str                        # "keyboard" or "mouse"
    connected: bool = False
    transport: str = "USB"
    firmware: str = "3.24.56"
    zones: list[Zone] = field(default_factory=list)
    effect: Effect = field(default_factory=Effect)
    bindings: dict[str, Binding] = field(default_factory=dict)
    # mouse only
    dpi_stages: list[int] = field(default_factory=lambda: [800, 1600, 3200])
    active_stage: int = 1
    polling_rate: int = 1000          # Hz
    lift_off: float = 1.2             # mm
    angle_snapping: bool = False
    # keyboard only
    win_lock: bool = False

    def lit_colour(self, t: float) -> tuple[int, int, int]:
        """The colour the device is showing at time t, for the preview."""
        e = self.effect
        b = max(0.0, min(1.0, e.brightness / 100.0))
        speed = 0.25 + e.speed / 100.0 * 1.75
        if e.kind == "breath":
            import math
            phase = (math.sin(t * speed * 2.0) + 1) / 2
            c = _mixc(e.colour, e.colour2, phase)
        elif e.kind == "rainbow":
            import colorsys
            h = (t * speed * 0.15) % 1.0
            r, g, bl = colorsys.hsv_to_rgb(h, 0.85, 1.0)
            c = (int(r * 255), int(g * 255), int(bl * 255))
        elif e.kind in ("wave", "gradient", "spiral", "ripple"):
            import math
            phase = (math.sin(t * speed * 1.6) + 1) / 2
            c = _mixc(e.colour, e.colour2, phase)
        elif e.kind == "solid":
            c = self.zones[0].colour if self.zones else e.colour
        else:
            c = e.colour
        return tuple(int(v * b) for v in c)


def _mixc(a, b, t):
    t = max(0.0, min(1.0, t))
    return (int(a[0] + (b[0] - a[0]) * t), int(a[1] + (b[1] - a[1]) * t), int(a[2] + (b[2] - a[2]) * t))


def demo_keyboard() -> Device:
    return Device(
        id="kb-1",
        name="RGB Mechanical Keyboard",
        kind="keyboard",
        connected=False,
        transport="USB",
        zones=[Zone("Main keys"), Zone("WASD", colour=(80, 140, 245)), Zone("Underglow", colour=(160, 120, 240)),
               Zone("Indicators", colour=(245, 208, 70))],
        effect=Effect("static", (150, 70, 200), (80, 140, 245), 50, 80),
        bindings={
            "Caps": Binding("key", "Esc", "Esc"),
            "PrtSc": Binding("macro", "Take screenshot", ""),
            "Scroll": Binding("disabled", "", ""),
        },
        dpi_stages=[], active_stage=0, polling_rate=1000,
    )


def demo_mouse() -> Device:
    return Device(
        id="mouse-1",
        name="RGB Gaming Mouse",
        kind="mouse",
        connected=False,
        transport="USB",
        zones=[Zone("Logo"), Zone("Scroll wheel", colour=(80, 140, 245)),
               Zone("Side strip", colour=(160, 120, 240)), Zone("DPI indicator", colour=(245, 208, 70))],
        effect=Effect("rainbow", (235, 64, 64), (80, 140, 245), 40, 80),
        dpi_stages=[800, 1600, 3200],
        active_stage=1,
    )


# ---------------------------------------------------------------------------
# profiles
# ---------------------------------------------------------------------------
@dataclass
class Profile:
    name: str
    kind: str = "software"           # software, hardware or default
    linked_to: str = ""              # a program it switches to, when linked
    devices: list[Device] = field(default_factory=list)
    keyboard: dict = field(default_factory=dict)   # e.g. "win_lock": bool


@dataclass
class Actions:
    macros: list[dict] = field(default_factory=list)

    def __post_init__(self):
        if not self.macros:
            self.macros = [
                {"name": "Copy", "keys": "Ctrl + C", "delay": 0},
                {"name": "Paste", "keys": "Ctrl + V", "delay": 0},
                {"name": "Take screenshot", "keys": "Print Screen", "delay": 120},
            ]


@dataclass
class AppSettings:
    start_at_login: bool = True
    start_minimised: bool = False
    minimise_to_tray: bool = True
    enable_sdk: bool = True
    polling_rate_hz: int = 1000
    language: str = "English"
    check_for_updates: bool = False
    show_demo_devices: bool = True
    brightness: int = 100


@dataclass
class Library:
    """Everything the app holds: profiles, actions and preferences."""
    profiles: list[Profile] = field(default_factory=list)
    active_profile: int = 0
    actions: Actions = field(default_factory=Actions)
    settings: AppSettings = field(default_factory=AppSettings)
    version: int = 1

    @staticmethod
    def fresh() -> "Library":
        kb, mouse = demo_keyboard(), demo_mouse()
        profiles = [
            Profile("Default", "default", "", [copy.deepcopy(kb), copy.deepcopy(mouse)]),
        ]
        return Library(profiles=profiles, active_profile=0)

    def profile(self) -> Profile:
        self.active_profile = max(0, min(len(self.profiles) - 1, self.active_profile))
        return self.profiles[self.active_profile]

    def device(self, device_id: str) -> Device | None:
        for d in self.profile().devices:
            if d.id == device_id:
                return d
        return None


# ---------------------------------------------------------------------------
# real hardware
# ---------------------------------------------------------------------------
def detect_hardware() -> list[Device]:
    """Look for Corsair devices the kernel has already found. Read-only."""
    found: list[Device] = []
    base = "/sys/bus/hid/devices"
    if not os.path.isdir(base):
        return found
    seen: set[str] = set()
    try:
        names = sorted(os.listdir(base))
    except OSError:
        return found
    for entry in names:
        uevent = os.path.join(base, entry, "uevent")
        try:
            text = open(uevent, "r", encoding="utf-8", errors="replace").read()
        except OSError:
            continue
        hid_id = re.search(r"HID_ID=[0-9A-Fa-f]+:([0-9A-Fa-f]+):([0-9A-Fa-f]+)", text)
        if not hid_id:
            continue
        # The kernel writes leading zeros (00001B1C), but the table and the
        # vendor constant use the bare hex form (1b1c). Normalise by dropping
        # leading zeros before comparing.
        vendor = hid_id.group(1).lstrip("0").lower() or "0"
        product = hid_id.group(2).lstrip("0").lower() or "0"
        if vendor != CORSAIR_VENDOR_ID:
            continue
        name_m = re.search(r"HID_NAME=(.+)", text)
        raw_name = name_m.group(1).strip() if name_m else f"Corsair device {product}"
        friendly, kind = KNOWN_PRODUCT_IDS.get(product, (None, None))
        if friendly:
            raw_name = friendly
        if kind is None:
            low = raw_name.lower()
            if "mouse" in low or "m65" in low or "harpoon" in low or "scimitar" in low:
                kind = "mouse"
            elif "keyboard" in low or "k70" in low or "k95" in low or "k65" in low:
                kind = "keyboard"
            else:
                # A cooler, fan controller or wireless receiver is not a keyboard.
                # Don't guess — skip it rather than mislabelling it.
                continue
        key = f"{product}:{kind}"
        if key in seen:
            continue
        seen.add(key)
        # A real device, not a renamed demo: one whole-device zone and no made-up
        # firmware string, until per-model zone maps and the report protocol exist.
        dev = Device(
            id=f"hw-{product}-{len(seen)}",
            name=raw_name,
            kind=kind,
            connected=True,
            transport="USB",
            firmware="",
            zones=[Zone("Whole device")],
        )
        found.append(dev)
    return found


# ---------------------------------------------------------------------------
# saving and loading
# ---------------------------------------------------------------------------
def config_path() -> str:
    root = os.environ.get("XDG_CONFIG_HOME") or os.path.join(os.path.expanduser("~"), ".config")
    return os.path.join(root, "rgbcue", "library.json")


def _device_from(d: dict) -> Device:
    eff = d.get("effect") or {}
    dev = Device(
        id=d.get("id", "dev"),
        name=d.get("name", "Device"),
        kind=d.get("kind", "keyboard"),
        connected=bool(d.get("connected", False)),
        transport=d.get("transport", "USB"),
        firmware=d.get("firmware", "3.24.56"),
        zones=[Zone(z.get("name", "Zone"), bool(z.get("on", True)), tuple(z.get("colour", (235, 64, 64))))
               for z in d.get("zones", [])],
        effect=Effect(eff.get("kind", "static"), tuple(eff.get("colour", (235, 64, 64))),
                      tuple(eff.get("colour2", (80, 140, 245))), int(eff.get("speed", 50)),
                      int(eff.get("brightness", 80))),
        bindings={k: Binding(v.get("kind", "default"), v.get("detail", ""), v.get("label", ""))
                  for k, v in (d.get("bindings") or {}).items()},
        dpi_stages=list(d.get("dpi_stages") or []),
        active_stage=int(d.get("active_stage", 0)),
        polling_rate=int(d.get("polling_rate", 1000)),
        lift_off=float(d.get("lift_off", 1.2)),
        angle_snapping=bool(d.get("angle_snapping", False)),
        win_lock=bool(d.get("win_lock", False)),
    )
    return dev


def to_json(lib: Library) -> dict:
    return {
        "version": 1,
        "active_profile": lib.active_profile,
        "settings": asdict(lib.settings),
        "actions": {"macros": lib.actions.macros},
        "profiles": [
            {
                "name": p.name,
                "kind": p.kind,
                "linked_to": p.linked_to,
                "keyboard": p.keyboard,
                "devices": [
                    {
                        "id": d.id, "name": d.name, "kind": d.kind, "connected": d.connected,
                        "transport": d.transport, "firmware": d.firmware,
                        "zones": [{"name": z.name, "on": z.on, "colour": list(z.colour)} for z in d.zones],
                        "effect": {"kind": d.effect.kind, "colour": list(d.effect.colour),
                                   "colour2": list(d.effect.colour2), "speed": d.effect.speed,
                                   "brightness": d.effect.brightness},
                        "bindings": {k: {"kind": b.kind, "detail": b.detail, "label": b.label}
                                     for k, b in d.bindings.items()},
                        "dpi_stages": d.dpi_stages, "active_stage": d.active_stage,
                        "polling_rate": d.polling_rate, "lift_off": d.lift_off,
                        "angle_snapping": d.angle_snapping, "win_lock": d.win_lock,
                    }
                    for d in p.devices
                ],
            }
            for p in lib.profiles
        ],
    }


def from_json(data: dict) -> Library:
    lib = Library.fresh()
    s = data.get("settings") or {}
    lib.settings = AppSettings(**{k: v for k, v in s.items() if k in AppSettings.__annotations__})
    macros = (data.get("actions") or {}).get("macros")
    if macros:
        lib.actions = Actions(macros=macros)
    profiles = []
    for p in data.get("profiles") or []:
        prof = Profile(p.get("name", "Profile"), p.get("kind", "software"), p.get("linked_to", ""),
                       [_device_from(d) for d in p.get("devices", [])], p.get("keyboard") or {})
        if prof.devices:
            profiles.append(prof)
    if profiles:
        lib.profiles = profiles
        lib.active_profile = int(data.get("active_profile", 0))
    return lib


def load(path: str | None = None) -> Library:
    path = path or config_path()
    try:
        with open(path, "r", encoding="utf-8") as fh:
            return from_json(json.load(fh))
    except Exception:
        return Library.fresh()


def save(lib: Library, path: str | None = None) -> str:
    path = path or config_path()
    os.makedirs(os.path.dirname(path), exist_ok=True)
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as fh:
        json.dump(to_json(lib), fh, indent=2)
    os.replace(tmp, path)
    return path
