"""The window: sidebar, dashboard, device pages, actions, profiles and settings.

Everything here is one class, ``App``.  It is given a size and a time, draws the
whole interface into a pixel buffer and returns it, and it answers clicks and key
presses by changing its own state.  Keeping the drawing and the behaviour in one
place means the same code serves the real window and the ``--screenshot`` run.
"""

from __future__ import annotations

import copy
import math
import os
import time

from PIL import Image

from . import model as M
from .theme import (ACCENT, ACCENT_TEXT, BG, BLUE, BORDER, BORDER_HI, Canvas, GREEN, PANEL, PANEL_HI,
                    PANEL_LO, PURPLE, RED, SIDEBAR, TEXT, TEXT_DIM, TEXT_FAINT, ZONE_COLORS, font, hsv,
                    mix, scale_color)

W, H = 1260, 800
SIDEBAR_W = 236
TOPBAR_H = 56
PAD = 24

NAV = [
    ("dashboard", "Dashboard", "grid"),
    ("actions", "Actions", "bolt"),
    ("profiles", "Profiles", "save"),
    ("settings", "Settings", "gear"),
]


class App:
    def __init__(self, width: int = W, height: int = H, library: M.Library | None = None,
                 autoload: bool = True):
        self.width = width
        self.height = height
        self.lib = library or (M.load() if autoload else M.Library.fresh())
        self.hardware = M.detect_hardware()
        self.page = "dashboard"
        self.device_id = self._first_device_id()
        self.tab = {}
        self.selected_key: dict[str, str] = {}
        self.button_sel: str | None = None
        self.macro_draft: dict[str, str] = {"name": "", "keys": ""}
        self.popup: dict | None = None
        self.toast: tuple[str, float] = ("", 0.0)
        self.pointer: tuple[float, float] | None = None
        self.regions: list = []
        self.sliders: dict = {}
        self.dragging: str | None = None
        self.editing: dict | None = None
        self.now = 0.0
        self.dirty = False
        self.saved_at = 0.0
        self.scale = 2
        self._status = ""
        self._spin_phase = 0.0
        if self.hardware:
            self._merge_hardware()

    # -- helpers ---------------------------------------------------------
    def _first_device_id(self) -> str:
        prof = self.lib.profile()
        return prof.devices[0].id if prof.devices else ""

    def _merge_hardware(self) -> None:
        """Mark demo devices as connected when matching hardware is present."""
        if not self.hardware:
            return
        for prof in self.lib.profiles:
            for dev in prof.devices:
                for hw in self.hardware:
                    if hw.kind == dev.kind:
                        dev.connected = True
                        dev.transport = hw.transport
                        break

    def say(self, text: str) -> None:
        self.toast = (text, self.now + 3.2)

    def device(self) -> M.Device | None:
        return self.lib.device(self.device_id)

    def goto(self, page: str) -> None:
        if page.startswith("device:"):
            self.device_id = page.split(":", 1)[1]
            self.page = "device"
        else:
            self.page = page
        self.popup = None
        self.editing = None

    def current_tab(self, dev: M.Device) -> str:
        tabs = self.tabs_for(dev)
        want = self.tab.get(dev.id, tabs[0])
        return want if want in tabs else tabs[0]

    @staticmethod
    def tabs_for(dev: M.Device) -> list[str]:
        if dev.kind == "mouse":
            return ["Lighting", "Performance", "Buttons", "Device settings"]
        return ["Lighting", "Key remaps", "Device settings"]

    # -- drawing entry point ---------------------------------------------
    def render(self, now: float | None = None, pointer: tuple[float, float] | None = None):
        """Draws the interface into a buffer at ``self.scale`` times the window size."""
        self.now = time.monotonic() if now is None else now
        self.pointer = pointer
        buf = Image.new("RGBA", (self.width * self.scale, self.height * self.scale), BG + (255,))
        c = Canvas(buf, self.scale, pointer)
        self.regions = []
        self.sliders = {}
        c.regions = self.regions
        c.sliders = self.sliders

        self._draw_topbar(c)
        self._draw_sidebar(c)
        self._draw_content(c)
        self._draw_toast(c)
        if self.popup:
            self._draw_popup(c)
        # keep what the layout check wants to complain about
        self.warnings = list(c.warnings)
        self.truncated = list(c.truncated)
        return buf

    def draw(self, now: float | None = None, pointer: tuple[float, float] | None = None) -> Image.Image:
        """The finished picture at window size, ready to show or save."""
        buf = self.render(now, pointer)
        if self.scale == 1:
            return buf.convert("RGB")
        # exactly halving is much faster than a general resample and is what
        # two-times drawing is for
        small = buf.reduce(self.scale)
        return small.convert("RGB")

    # =====================================================================
    # top bar
    # =====================================================================
    def _draw_topbar(self, c: Canvas) -> None:
        c.rect((0, 0, self.width, TOPBAR_H), fill=(17, 19, 23))
        c.line([(0, TOPBAR_H), (self.width, TOPBAR_H)], fill=BORDER, width=1)

        # mark + wordmark
        c.rect((PAD, 16, PAD + 24, 40), fill=ACCENT, radius=7)
        c.icon("bolt", (PAD + 12, 28), 13, ACCENT_TEXT)
        c.text((PAD + 34, 27), "RGBCUE", 16, True, TEXT, anchor="lm")
        c.text((PAD + 34 + c.measure("RGBCUE", 16, True)[0] + 8, 28), "for Linux", 11, False,
               TEXT_FAINT, anchor="lm")

        # active profile chip
        prof = self.lib.profile()
        chip_x = self.width - 300
        box = (chip_x, 13, chip_x + 200, 43)
        hover = c.contains(box)
        c.rect(box, fill=PANEL_HI if hover else PANEL, radius=8, outline=BORDER_HI if hover else BORDER)
        c.circle((box[0] + 16, 28), 4, fill=ACCENT)
        c.text_in((box[0] + 26, 28), prof.name, 12, True, TEXT, anchor="lm", pad=0)
        c.icon("chevron-down", (box[2] - 15, 28), 10, TEXT_DIM)
        c.register(box, ("profile_picker",))

        # save state
        state = "Unsaved changes" if self.dirty else "All changes saved"
        col = ACCENT if self.dirty else GREEN
        c.circle((self.width - 340, 28), 3.5, fill=col)
        c.text((self.width - 330, 28), state, 11, False, TEXT_DIM, anchor="lm")

    # =====================================================================
    # sidebar
    # =====================================================================
    def _draw_sidebar(self, c: Canvas) -> None:
        c.rect((0, TOPBAR_H, SIDEBAR_W, self.height), fill=SIDEBAR)
        c.line([(SIDEBAR_W, TOPBAR_H), (SIDEBAR_W, self.height)], fill=BORDER, width=1)
        y = TOPBAR_H + 18
        c.text((PAD - 4, y), "WORKSPACE", 10, True, TEXT_FAINT)
        y += 20
        for pid, label, icon in NAV:
            box = (12, y, SIDEBAR_W - 12, y + 38)
            active = self.page == pid
            hover = c.contains(box)
            if active:
                c.rect(box, fill=PANEL_HI, radius=8)
            elif hover:
                c.rect(box, fill=(26, 29, 34), radius=8)
            col = TEXT if active or hover else TEXT_DIM
            c.icon(icon, (box[0] + 22, y + 19), 13, ACCENT if active else col)
            c.text((box[0] + 40, y + 19), label, 12.5, active, col, anchor="lm")
            c.register(box, ("goto", pid))
            y += 42

        y += 6
        c.line([(PAD - 4, y), (SIDEBAR_W - 20, y)], fill=BORDER, width=1)
        y += 18
        c.text((PAD - 4, y), "DEVICES", 10, True, TEXT_FAINT)
        y += 20

        prof = self.lib.profile()
        for dev in prof.devices:
            box = (12, y, SIDEBAR_W - 12, y + 56)
            active = self.page == "device" and self.device_id == dev.id
            hover = c.contains(box)
            if active:
                c.rect(box, fill=PANEL_HI, radius=9)
                c.rect((box[0], y + 10, box[0] + 3, y + 46), fill=ACCENT, radius=1.5)
            elif hover:
                c.rect(box, fill=(27, 30, 36), radius=9)
            self._device_thumb(c, dev, (box[0] + 14, y + 9, box[0] + 46, y + 47))
            c.text_in((box[0] + 56, y + 17), dev.name, 12, True, TEXT, pad=0)
            dot = GREEN if dev.connected else (120, 127, 140)
            c.circle((box[0] + 60, y + 40), 3, fill=dot)
            c.text_in((box[0] + 68, y + 40), "Connected" if dev.connected else "Demo device",
                      10.5, False, TEXT_DIM if dev.connected else TEXT_FAINT, anchor="lm", pad=0)
            c.register(box, ("goto", f"device:{dev.id}"))
            y += 62

        # footer
        fy = self.height - 74
        c.line([(PAD - 4, fy), (SIDEBAR_W - 20, fy)], fill=BORDER, width=1)
        hw = len(self.hardware)
        if hw:
            c.circle((PAD - 4 + 4, fy + 18), 3.5, fill=GREEN)
            c.text((PAD + 8, fy + 18), f"{hw} Corsair device{'s' if hw != 1 else ''} found", 11,
                   False, TEXT_DIM, anchor="lm")
        else:
            c.circle((PAD - 4 + 4, fy + 18), 3.5, fill=(150, 120, 50))
            c.text((PAD + 8, fy + 18), "No hardware found", 11, False, TEXT_DIM, anchor="lm")
        c.text((PAD - 4, fy + 36), "Demo mode · settings are saved", 10.5, False, TEXT_FAINT)
        c.text((PAD - 4, fy + 52), "Local only · v1.0", 10.5, False, TEXT_FAINT)

    def _device_thumb(self, c: Canvas, dev: M.Device, box) -> None:
        x0, y0, x1, y1 = box
        c.rect(box, fill=(16, 18, 22), radius=6)
        col = dev.lit_colour(self.now)
        if dev.kind == "keyboard":
            bw, bh = (x1 - x0) - 10, 18
            bx, by = x0 + 5, y0 + (y1 - y0) / 2 - bh / 2
            c.rect((bx, by, bx + bw, by + bh), fill=(38, 42, 50), radius=3)
            for r in range(3):
                for i in range(9):
                    kx = bx + 3 + i * (bw - 6) / 9
                    c.rect((kx, by + 3 + r * 4.6, kx + (bw - 6) / 9 - 1.6, by + 3 + r * 4.6 + 3.4),
                           fill=scale_color(col, 0.85), radius=1)
        else:
            cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
            c.rect((cx - 7, cy - 15, cx + 7, cy + 15), fill=(40, 44, 52), radius=7)
            c.rect((cx - 6, cy - 14, cx + 6, cy - 1), fill=(52, 57, 66), radius=5)
            c.line([(cx, cy - 14), (cx, cy - 4)], fill=(30, 33, 39), width=1.2)
            c.rect((cx - 1.4, cy - 9, cx + 1.4, cy - 2), fill=scale_color(col, 1.0), radius=1.2)
            c.circle((cx, cy + 8), 4.2, fill=scale_color(dev.zones[0].colour if dev.zones else col, 0.9))
            c.circle((cx, cy + 8), 4.2, outline=(0, 0, 0, 90))

    # =====================================================================
    # content shell
    # =====================================================================
    def _draw_content(self, c: Canvas) -> None:
        x0 = SIDEBAR_W + PAD
        top = TOPBAR_H + 18
        right = self.width - PAD
        bottom = self.height - 16
        if self.page == "dashboard":
            self._page_dashboard(c, (x0, top, right, bottom))
        elif self.page == "device":
            self._page_device(c, (x0, top, right, bottom))
        elif self.page == "actions":
            self._page_actions(c, (x0, top, right, bottom))
        elif self.page == "profiles":
            self._page_profiles(c, (x0, top, right, bottom))
        elif self.page == "settings":
            self._page_settings(c, (x0, top, right, bottom))

    def _page_title(self, c: Canvas, x: float, y: float, title: str, sub: str) -> float:
        c.text((x, y), title, 21, True, TEXT)
        c.text((x, y + 27), sub, 12, False, TEXT_DIM)
        return y + 62

    def _card(self, c: Canvas, box, title: str | None = None, sub: str | None = None,
              pad: float = 18) -> float:
        c.rect(box, fill=PANEL, radius=12, outline=BORDER)
        y = box[1] + pad
        if title:
            c.text((box[0] + pad, y), title, 13, True, TEXT)
            y += 20
        if sub:
            c.text((box[0] + pad, y), sub, 11.5, False, TEXT_DIM)
            y += 18
        return y

    def _label(self, c: Canvas, x: float, y: float, text: str, value: str | None = None) -> None:
        c.text((x, y), text, 11.5, True, TEXT_DIM)
        if value is not None:
            w = c.measure(value, 11.5, True)[0]
            c.text((x, y), value, 11.5, True, TEXT)
            # right-align the value where a width is known
            c.text((x, y), value, 11.5, True, TEXT)

    # =====================================================================
    # dashboard
    # =====================================================================
    def _page_dashboard(self, c: Canvas, box) -> None:
        x0, y0, x1, _y1 = box
        prof = self.lib.profile()
        y = self._page_title(c, x0, y0, "Dashboard",
                             "Everything your devices are set to right now.")
        # header actions
        self._header_buttons(c, x1, y0)

        # stat row
        stats = [
            ("Devices", str(len(prof.devices)), f"{sum(1 for d in prof.devices if d.connected)} connected"),
            ("Active profile", prof.name, prof.kind.capitalize()),
            ("Lighting", _effect_label(prof.devices[0].effect.kind) if prof.devices else "—",
             f"{len(prof.devices[0].zones)} zones" if prof.devices else ""),
            ("Mouse DPI", _dpi_text(prof), "on stage " + str((_mouse(prof).active_stage + 1)) if _mouse(prof) else ""),
        ]
        gap = 14
        cw = (x1 - x0 - gap * 3) / 4
        for i, (label, value, sub) in enumerate(stats):
            bx = x0 + i * (cw + gap)
            box_s = (bx, y, bx + cw, y + 88)
            c.rect(box_s, fill=PANEL, radius=12, outline=BORDER)
            c.text((bx + 16, y + 16), label.upper(), 9.5, True, TEXT_FAINT)
            c.text_in((bx + 16, y + 32), value, 20, True, TEXT, pad=0)
            c.text_in((bx + 16, y + 60), sub, 11, False, TEXT_DIM, pad=0)
        y += 106

        # device cards
        devs = prof.devices
        if not devs:
            c.text((x0, y), "No devices yet. Add one from Profiles.", 12, False, TEXT_DIM)
            return
        cw2 = (x1 - x0 - gap) / 2 if len(devs) > 1 else (x1 - x0)
        ch = 300
        for i, dev in enumerate(devs[:2]):
            bx = x0 + i * (cw2 + gap)
            self._dashboard_device_card(c, (bx, y, bx + cw2, y + ch), dev)

        y += ch + gap
        # hardware banner
        bh = 74
        banner = (x0, y, x1, y + bh)
        c.rect(banner, fill=(24, 27, 32), radius=12, outline=BORDER)
        c.icon("info", (x0 + 26, y + bh / 2), 15, ACCENT)
        if self.hardware:
            head = f"{len(self.hardware)} Corsair device(s) found on this computer"
            body = "Detected through the kernel's USB list. Settings are saved here, ready to apply."
        else:
            head = "No Corsair hardware detected — demo devices are shown"
            body = ("This machine exposes no Corsair USB devices to the app, so the keyboard and mouse "
                    "above are demos you can configure freely. Your settings are still saved.")
        c.text((x0 + 44, y + 22), head, 12.5, True, TEXT)
        c.text_in((x0 + 44, y + 42), body, 11.5, False, TEXT_DIM, pad=0)
        y += bh + gap

        # quick presets + storage, filling the rest of the page
        rest = _y1 - y
        if rest > 110:
            half = (x1 - x0 - gap) / 2
            left = (x0, y, x0 + half, y + min(rest, 132))
            right = (x0 + half + gap, y, x1, y + min(rest, 132))
            self._preset_card(c, left, prof)
            self._storage_card(c, right, prof)

    def _preset_card(self, c: Canvas, box, prof: M.Profile) -> None:
        y = self._card(c, box, "Quick lighting presets",
                       "Applies to every device in this profile.")
        presets = [("static", "Steady red", (235, 64, 64)), ("rainbow", "Rainbow", (168, 130, 255)),
                   ("breath", "Breathe blue", (80, 140, 245)), ("wave", "Aqua wave", (70, 200, 215))]
        bw = (box[2] - box[0] - 36 - 3 * 8) / 4
        for i, (kind, label, col) in enumerate(presets):
            bx = box[0] + 18 + i * (bw + 8)
            pb = (bx, y, bx + bw, y + 46)
            hover = c.contains(pb)
            active = prof.devices and prof.devices[0].effect.kind == kind
            c.rect(pb, fill=(38, 42, 50) if active else (PANEL_HI if hover else PANEL_LO), radius=9,
                   outline=ACCENT if active else BORDER)
            c.circle((bx + 16, y + 23), 6, fill=col)
            c.text_in((bx + 28, y + 23), label, 10.5, active, TEXT if active else TEXT_DIM,
                      anchor="lm", pad=0)
            c.register(pb, ("preset", kind))
        c.text((box[0] + 18, y + 58), "Presets change the effect; per-device colours stay as they are.",
               10, False, TEXT_FAINT)

    def _storage_card(self, c: Canvas, box, prof: M.Profile) -> None:
        y = self._card(c, box, "Profile and storage")
        c.text((box[0] + 18, y), "ACTIVE PROFILE", 10, True, TEXT_FAINT)
        c.text_in((box[0] + 18, y + 14), f"{prof.name} — {prof.kind}", 11.5, True, TEXT, pad=0)
        c.text((box[0] + 18, y + 34), "SAVED TO", 10, True, TEXT_FAINT)
        c.text_in((box[0] + 18, y + 48), _short_path(M.config_path()), 10.5, False, TEXT_DIM, pad=0)
        c.pill("Saved automatically" if not self.dirty else "Unsaved changes",
               (box[0] + 18, y + 66), GREEN if not self.dirty else ACCENT, None, 9.5)
        c.button((box[2] - 112, box[3] - 42, box[2] - 18, box[3] - 12), "Save now", ("save",),
                 "secondary" if not self.dirty else "primary", 11, icon="save")

    def _header_buttons(self, c: Canvas, x1: float, y: float) -> None:
        b_w, b_h = 104, 34
        self._save_button(c, (x1 - b_w, y - 2, x1, y - 2 + b_h))
        if self.dirty:
            c.circle((x1 - b_w - 10, y + 15), 3, fill=ACCENT)

    def _save_button(self, c: Canvas, box) -> None:
        c.button(box, "Save", ("save",), "primary" if self.dirty else "secondary", 12.5, icon="check")

    def _dashboard_device_card(self, c: Canvas, box, dev: M.Device) -> None:
        x0, y0, x1, y1 = box
        hover = c.contains(box)
        c.rect(box, fill=PANEL if not hover else (28, 31, 37), radius=12, outline=BORDER)
        c.text_in((x0 + 18, y0 + 16), dev.name, 13.5, True, TEXT, pad=0)
        # status pill
        label = "Connected" if dev.connected else "Demo"
        col = GREEN if dev.connected else TEXT_DIM
        pw, _ = c.measure(label, 10, True)
        c.rect((x1 - 18 - pw - 20, y0 + 14, x1 - 18, y0 + 32), fill=(col[0] // 6, col[1] // 6, col[2] // 6),
               radius=9)
        c.circle((x1 - 18 - pw - 12, y0 + 23), 3, fill=col)
        c.text((x1 - 18 - pw - 5, y0 + 23), label, 10, True, col, anchor="lm")

        preview = (x0 + 16, y0 + 44, x1 - 16, y0 + 44 + 140)
        self._device_preview(c, dev, preview, interactive=False)

        # effect row
        ey = preview[3] + 14
        c.text((x0 + 18, ey), "Effect", 10.5, True, TEXT_FAINT)
        c.text((x0 + 78, ey), _effect_label(dev.effect.kind), 11.5, True, TEXT)
        # zone dots
        dx = x1 - 18
        for z in reversed(dev.zones[-4:]):
            dx -= 16
            c.circle((dx, ey + 5), 5, fill=z.colour if z.on else (60, 64, 72))
            c.circle((dx, ey + 5), 5, outline=(0, 0, 0, 80))

        # brightness
        by = ey + 22
        c.text((x0 + 18, by + 6), "Brightness", 11, False, TEXT_DIM)
        c.slider(f"dash-bright-{dev.id}", (x0 + 96, by, x1 - 78, by + 18), dev.effect.brightness, 0, 100,
                 lambda v, d=dev: self._set_brightness(d, v))
        c.text((x1 - 18, by + 6), f"{dev.effect.brightness}%", 11, True, TEXT, anchor="ra")

        # open button
        c.button((x0 + 16, y1 - 44, x0 + 122, y1 - 14), "Open device",
                 ("goto", f"device:{dev.id}"), "secondary", 11.5)

    def _set_brightness(self, dev: M.Device, v: float) -> None:
        dev.effect.brightness = int(round(v))
        self.dirty = True

    # =====================================================================
    # device page
    # =====================================================================
    def _page_device(self, c: Canvas, box) -> None:
        x0, y0, x1, y1 = box
        dev = self.device()
        if dev is None:
            self.goto("dashboard")
            return
        c.text_in((x0, y0), dev.name, 21, True, TEXT, pad=0)
        # status under the title, so a long device name never runs into it
        label = "Connected over " + dev.transport if dev.connected else "Demo device"
        col = GREEN if dev.connected else (150, 158, 170)
        pw, _ = c.measure(label, 10.5, True)
        px = x0
        c.rect((px, y0 + 30, px + pw + 20, y0 + 48), fill=(col[0] // 7, col[1] // 7, col[2] // 7),
               radius=9)
        c.circle((px + 10, y0 + 39), 3, fill=col)
        c.text((px + 17, y0 + 39), label, 10.5, True, col, anchor="lm")
        c.text((px + pw + 32, y0 + 39), f"{dev.kind.capitalize()} · firmware {dev.firmware}",
               10.5, False, TEXT_FAINT, anchor="lm")

        # tabs
        tabs = self.tabs_for(dev)
        current = self.current_tab(dev)
        tx = x0
        ty = y0 + 36
        for t in tabs:
            w = c.measure(t, 12.5, True)[0] + 26
            tab_box = (tx, ty, tx + w, ty + 32)
            active = t == current
            if active:
                c.rect(tab_box, fill=PANEL, radius=8, outline=BORDER)
                c.text((tx + w / 2, ty + 16), t, 12.5, True, ACCENT, anchor="mm")
            else:
                hover = c.contains(tab_box)
                c.text((tx + w / 2, ty + 16), t, 12.5, False, TEXT if hover else TEXT_DIM, anchor="mm")
            c.register(tab_box, ("tab", t))
            tx += w + 6
        c.line([(x0, ty + 40), (x1, ty + 40)], fill=BORDER, width=1)

        top = ty + 58
        if current == "Lighting":
            self._tab_lighting(c, (x0, top, x1, y1), dev)
        elif current == "Key remaps":
            self._tab_keymap(c, (x0, top, x1, y1), dev)
        elif current == "Performance":
            self._tab_performance(c, (x0, top, x1, y1), dev)
        elif current == "Buttons":
            self._tab_buttons(c, (x0, top, x1, y1), dev)
        else:
            self._tab_device_settings(c, (x0, top, x1, y1), dev)

    # -- lighting ---------------------------------------------------------
    def _tab_lighting(self, c: Canvas, box, dev: M.Device) -> None:
        x0, y0, x1, y1 = box
        left_w = int((x1 - x0) * 0.52)
        preview = (x0, y0, x0 + left_w, y0 + 330)
        c.rect(preview, fill=(11, 13, 16), radius=14, outline=BORDER)
        self._device_preview(c, dev, (preview[0] + 26, preview[1] + 30, preview[2] - 26, preview[3] - 56),
                             interactive=True)

        # device brightness row under the preview
        by = preview[3] - 34
        c.text((preview[0] + 26, by), "Brightness", 11, False, TEXT_DIM)
        c.slider(f"light-bright-{dev.id}", (preview[0] + 104, by - 4, preview[2] - 74, by + 14),
                 dev.effect.brightness, 0, 100, lambda v, d=dev: self._set_brightness(d, v))
        c.text((preview[2] - 26, by), f"{dev.effect.brightness}%", 11, True, TEXT, anchor="ra")

        # zones
        zy = preview[3] + 18
        c.text((x0, zy), "ZONES", 10, True, TEXT_FAINT)
        zy += 22
        cw = (left_w - 12) / 2
        for i, z in enumerate(dev.zones):
            col_i, row_i = i % 2, i // 2
            zb = (x0 + col_i * (cw + 12), zy + row_i * 46, x0 + col_i * (cw + 12) + cw,
                  zy + row_i * 46 + 40)
            c.rect(zb, fill=PANEL, radius=9, outline=BORDER)
            c.rect((zb[0] + 10, zb[1] + 12, zb[0] + 26, zb[1] + 28),
                   fill=z.colour if z.on else (48, 52, 60), radius=4)
            c.text_in((zb[0] + 34, zb[1] + 20), z.name, 11.5, False, TEXT if z.on else TEXT_FAINT, anchor="lm")
            c.toggle((zb[2] - 40, zb[1] + 12, zb[2] - 14, zb[1] + 28), z.on, ("zone_toggle", i))
            c.register((zb[0], zb[1], zb[2] - 46, zb[3]), ("zone_select", i))
        zy += ((len(dev.zones) + 1) // 2) * 46 + 8

        # how the lighting looks right now, and how far it has been applied
        if y1 - zy >= 96:
            tall = y1 - zy >= 150
            c.rect((x0, zy, x0 + left_w, y1), fill=PANEL, radius=14, outline=BORDER)
            c.text((x0 + 20, zy + 16), "CURRENT LOOK", 10, True, TEXT_FAINT)
            self._effect_legend(c, (x0 + 18, zy + 32, x0 + left_w - 18, zy + 32 + 40), dev)
            if tall:
                c.line([(x0 + 20, zy + 90), (x0 + left_w - 20, zy + 90)], fill=BORDER, width=1)
                c.text((x0 + 20, zy + 102), "APPLIED TO", 10, True, TEXT_FAINT)
                c.text_in((x0 + 20, zy + 118),
                          f"{dev.name} · {len([z for z in dev.zones if z.on])} of "
                          f"{len(dev.zones)} zones · {dev.effect.brightness}% brightness",
                          11, False, TEXT_DIM, pad=0)
                c.text_in((x0 + 20, zy + 140),
                          "Kept in this profile. It is sent to the device once the USB "
                          "report protocol is added.", 10, False, TEXT_FAINT, pad=0)

        # right: effects  (fit to available height)
        rx = x0 + left_w + 18
        rbox = (rx, y0, x1, y1)
        c.rect(rbox, fill=PANEL, radius=14, outline=BORDER)
        c.text((rx + 18, y0 + 16), "Lighting effect", 13, True, TEXT)
        c.text((rx + 18, y0 + 36), "Choose how the illuminated zones behave.", 11, False, TEXT_DIM)

        ey = y0 + 60
        avail_h = y1 - ey - 24
        row_h = 40
        max_rows = int((avail_h - 120) / row_h)
        effects = M.EFFECTS[: max(3, min(len(M.EFFECTS), max_rows))]
        for kind, label, desc in effects:
            eb = (rx + 14, ey, x1 - 14, ey + row_h - 4)
            active = dev.effect.kind == kind
            hover = c.contains(eb)
            if active:
                c.rect(eb, fill=(38, 42, 50), radius=8)
                c.rect((eb[0], eb[1] + 6, eb[0] + 3, eb[3] - 6), fill=ACCENT, radius=1.5)
            elif hover:
                c.rect(eb, fill=(31, 34, 41), radius=8)
            c.circle((eb[0] + 18, eb[1] + (row_h - 4) / 2), 6,
                     fill=self._effect_swatch(dev, kind), outline=(0, 0, 0, 90))
            c.text((eb[0] + 34, eb[1] + 12), label, 12, active, TEXT if active else TEXT_DIM)
            c.text_in((eb[0] + 34, eb[1] + 25), desc, 10, False, TEXT_FAINT, pad=0)
            c.register(eb, ("effect", kind))
            if active:
                c.icon("check", (eb[2] - 18, eb[1] + (row_h - 4) / 2), 11, ACCENT)
            ey += row_h

        # colours + speed
        ey += 6
        c.line([(rx + 18, ey), (x1 - 18, ey)], fill=BORDER, width=1)
        ey += 16
        two = dev.effect.kind in ("breath", "wave", "gradient", "spiral", "ripple")
        if two:
            c.text((rx + 18, ey), "Colours", 11, True, TEXT_DIM)
            for row in range(2):
                sy = ey + 20 + row * 26
                for i, (name, col) in enumerate(ZONE_COLORS):
                    sx = rx + 18 + i * 24
                    cur = (dev.effect.colour if row == 0 else dev.effect.colour2) == col
                    c.swatch((sx, sy, sx + 18, sy + 18), col, ("colour", row, i), selected=cur)
            ey += 20 + 2 * 26 + 6
        else:
            c.text((rx + 18, ey), "Colour", 11, True, TEXT_DIM)
            for i, (name, col) in enumerate(ZONE_COLORS):
                sx = rx + 18 + i * 24
                c.swatch((sx, sy := ey + 20, sx + 18, sy + 18), col, ("colour", 0, i),
                         selected=dev.effect.colour == col)
            ey += 48

        if dev.effect.kind in ("breath", "rainbow", "wave", "spiral", "ripple"):
            c.text((rx + 18, ey), "Speed", 11, True, TEXT_DIM)
            c.slider(f"speed-{dev.id}", (rx + 76, ey - 4, x1 - 60, ey + 14), dev.effect.speed, 0, 100,
                     lambda v, d=dev: self._set_speed(d, v))
            c.text((x1 - 18, ey), f"{dev.effect.speed}%", 11, True, TEXT, anchor="ra")
            ey += 26

        # live sample
        self._effect_legend(c, (rx + 14, y1 - 66, x1 - 14, y1 - 16), dev)

    def _set_speed(self, dev: M.Device, v: float) -> None:
        dev.effect.speed = int(round(v))
        self.dirty = True

    def _effect_swatch(self, dev: M.Device, kind: str):
        e = dev.effect
        if kind in ("static", "solid"):
            return e.colour
        if kind == "rainbow":
            return hsv((self.now * 80) % 360, 0.8, 0.95)
        if kind == "gradient":
            return mix(e.colour, e.colour2, 0.5)
        if kind == "breath":
            return mix(e.colour, e.colour2, (math.sin(self.now * 2) + 1) / 2)
        return mix(e.colour2, e.colour, (math.sin(self.now * 1.6) + 1) / 2)

    def _effect_legend(self, c: Canvas, box, dev: M.Device) -> None:
        x0, y0, x1, y1 = box
        c.rect(box, fill=(20, 23, 27), radius=9)
        c.text((x0 + 12, y0 + 9), "Live sample", 9.5, True, TEXT_FAINT)
        y = y0 + 24
        h = (y1 - y) / 2
        for i in range((x1 - x0 - 24) // 6):
            t = self.now * (0.4 + dev.effect.speed / 100.0)
            if dev.effect.kind == "gradient":
                col = mix(dev.effect.colour, dev.effect.colour2, i / max(1, (x1 - x0 - 24) // 6))
            elif dev.effect.kind == "rainbow":
                col = hsv((i * 12 + t * 90) % 360, 0.8, 0.95)
            elif dev.effect.kind == "wave":
                col = mix(dev.effect.colour2, dev.effect.colour, (math.sin(i * 0.5 - t * 3) + 1) / 2)
            elif dev.effect.kind == "breath":
                col = mix(dev.effect.colour, dev.effect.colour2, (math.sin(t * 2) + 1) / 2)
            elif dev.effect.kind == "spiral":
                col = mix(dev.effect.colour, dev.effect.colour2, (math.sin(i * 0.8 + t * 2) + 1) / 2)
            else:
                col = dev.effect.colour
            col = scale_color(col, dev.effect.brightness / 100.0 * 0.95 + 0.05)
            c.rect((x0 + 12 + i * 6, y, x0 + 17 + i * 6, y + h), fill=col, radius=1)

    # -- key remaps -------------------------------------------------------
    def _tab_keymap(self, c: Canvas, box, dev: M.Device) -> None:
        x0, y0, x1, y1 = box
        right_w = 288
        left_right = x1 - right_w - 18      # where the assignment panel begins
        left_w = left_right - x0
        board = (x0, y0, left_right, y0 + 306)
        c.rect(board, fill=(11, 13, 16), radius=14, outline=BORDER)
        c.text((board[0] + 20, board[1] + 14), "Click a key to change what it does", 11, False, TEXT_DIM)
        self._keyboard_grid(c, dev, (board[0] + 16, board[1] + 40, board[2] - 16, board[3] - 20),
                            selectable=True)
        # quick presets for the whole keyboard
        quick_y = board[3] + 14
        c.rect((x0, quick_y, x0 + left_w, quick_y + 92), fill=PANEL, radius=12, outline=BORDER)
        c.text((x0 + 18, quick_y + 12), "KEYBOARD LIGHTING", 10, True, TEXT_FAINT)
        c.text_in((x0 + 18, quick_y + 28), f"{_effect_label(dev.effect.kind)} · "
                                           f"{dev.effect.brightness}% brightness · "
                                           f"{sum(1 for z in dev.zones if z.on)} zones on",
                  11, False, TEXT_DIM, pad=0)
        c.button((x0 + 18, quick_y + 50, x0 + 148, quick_y + 78), "Lighting", ("open_lighting",),
                 "secondary", 11, icon="bolt")
        for i, (kind, label) in enumerate([("static", "Steady"), ("rainbow", "Rainbow"),
                                           ("wave", "Wave"), ("ripple", "Ripple")]):
            bx = x0 + 160 + i * 78
            if bx + 70 > x0 + left_w - 14:
                break
            b = (bx, quick_y + 50, bx + 70, quick_y + 78)
            c.button(b, label, ("effect", kind), "ghost" if dev.effect.kind != kind else "primary", 10.5)

        # zones at a glance
        zy = quick_y + 104
        if zy + 120 < y1:
            c.rect((x0, zy, x0 + left_w, y1), fill=PANEL, radius=12, outline=BORDER)
            c.text((x0 + 18, zy + 14), "ZONES", 10, True, TEXT_FAINT)
            c.text_in((x0 + 18, zy + 30), "A zone lights its own keys in a colour of its own.",
                      11, False, TEXT_DIM, pad=0)
            zx = x0 + 18
            for i, z in enumerate(dev.zones):
                row_y = zy + 58 + i * 34
                if row_y + 28 > y1 - 12:
                    break
                c.circle((zx + 7, row_y + 13), 7, fill=z.colour if z.on else (52, 56, 64))
                c.text((zx + 22, row_y + 13), z.name, 11.5, True, TEXT if z.on else TEXT_FAINT,
                       anchor="lm")
                c.text_in((zx + 150, row_y + 13), "on" if z.on else "off", 10.5, False,
                          GREEN if z.on else TEXT_FAINT, anchor="lm", pad=0)
                c.toggle((x0 + left_w - 66, row_y + 2, x0 + left_w - 14, row_y + 24), z.on,
                         ("zone", dev.id, z.name))

        side = (x1 - right_w, y0, x1, y1)
        c.rect(side, fill=PANEL, radius=14, outline=BORDER)
        sel = self.selected_key.get(dev.id) or "Q"
        c.text((side[0] + 18, side[1] + 16), "Assignment", 13, True, TEXT)
        c.text((side[0] + 18, side[1] + 36), f"Key: {sel}", 11.5, False, TEXT_DIM)
        key_box = (side[0] + 18, side[1] + 56, side[2] - 18, side[1] + 100)
        c.rect(key_box, fill=(20, 23, 27), radius=9, outline=BORDER)
        c.text((key_box[0] + 12, (key_box[1] + key_box[3]) / 2), sel, 18, True, ACCENT, anchor="lm")
        binding = dev.bindings.get(sel) or M.Binding()
        c.text_in((key_box[0] + 60 + c.measure(sel, 18, True)[0], (key_box[1] + key_box[3]) / 2),
                  binding.describe(sel), 11, False, TEXT_DIM, anchor="lm", pad=0)

        y = side[1] + 114
        c.text((side[0] + 18, y), "ACTION", 10, True, TEXT_FAINT)
        y += 18
        col_w = (side[2] - side[0] - 36 - 8) / 2
        for i, (value, label) in enumerate(M.ACTION_KINDS):
            col, row_i = divmod(i, (len(M.ACTION_KINDS) + 1) // 2)
            rx = side[0] + 14 + col * (col_w + 8)
            ry = y + row_i * 32
            row = (rx, ry, rx + col_w, ry + 30)
            active = binding.kind == value
            if active:
                c.rect(row, fill=(38, 42, 50), radius=7)
            elif c.contains(row):
                c.rect(row, fill=(31, 34, 41), radius=7)
            c.circle((row[0] + 13, ry + 15), 5, outline=ACCENT if active else (90, 97, 108), width=1.6)
            if active:
                c.circle((row[0] + 13, ry + 15), 2.4, fill=ACCENT)
            c.text_in((row[0] + 25, ry + 15), label, 10.5, active, TEXT if active else TEXT_DIM,
                      anchor="lm", pad=0)
            c.register(row, ("binding_kind", value))
        y += (((len(M.ACTION_KINDS) + 1) // 2) * 32)

        # detail controls for the chosen action
        detail_label = {"key": "KEY", "media": "MEDIA KEY", "macro": "MACRO", "app": "APPLICATION PATH",
                        "dpi": "STAGE", "profile": "PROFILE"}.get(binding.kind)
        y = self._control(c, (side[0], y + 10, side[2], y + 90), binding, detail_label,
                          lambda v, d=dev, k=sel: self._set_binding_detail(d, k, v))

        # assigned list
        ay = y + 16
        c.line([(side[0] + 18, ay), (side[2] - 18, ay)], fill=BORDER, width=1)
        c.text((side[0] + 18, ay + 12), "REMAPPED KEYS", 10, True, TEXT_FAINT)
        ly = ay + 32
        assigned = [(k, b) for k, b in dev.bindings.items() if b.kind != "default"]
        if not assigned:
            c.text_in((side[0] + 18, ly), "None yet — pick an action above.", 11, False,
                      TEXT_FAINT, pad=0)
        room = int(max(0, (side[3] - 60 - ly) // 28))
        for k, b in assigned[:room]:
            row = (side[0] + 14, ly, side[2] - 14, ly + 26)
            if c.contains(row):
                c.rect(row, fill=(31, 34, 41), radius=6)
            c.text((row[0] + 10, ly + 13), k, 11, True, ACCENT, anchor="lm")
            c.text_in((row[0] + 70, ly + 13), b.describe(k), 10.5, False, TEXT_DIM,
                      anchor="lm", pad=0)
            c.icon("close", (row[2] - 12, ly + 13), 9, TEXT_FAINT)
            c.register((row[2] - 26, ly, row[2], ly + 26), ("binding_reset", k))
            ly += 28
        if len(assigned) > room:
            c.text((side[0] + 18, ly), f"+{len(assigned) - room} more", 10, False, TEXT_FAINT)

        c.button((side[0] + 18, side[3] - 40, side[2] - 18, side[3] - 12), "Reset all keys",
                 ("reset_keys",), "ghost", 11)

    def _detail_options(self, dev: M.Device, kind: str) -> list[tuple[str, str]]:
        if kind == "key":
            return [(k, k) for k in M.REMAP_KEYS]
        if kind == "media":
            return [(k, k) for k in M.MEDIA_KEYS]
        if kind == "macro":
            return [(m["name"], m["name"]) for m in self.lib.actions.macros]
        if kind == "dpi":
            return [(str(i + 1), f"Stage {i + 1} — {v} DPI") for i, v in enumerate(dev.dpi_stages)]
        if kind == "profile":
            return [(p.name, p.name) for p in self.lib.profiles]
        return []

    def _set_binding_detail(self, dev: M.Device, key: str, value: str) -> None:
        b = dev.bindings.setdefault(key, M.Binding())
        b.detail = value
        dev.bindings[key] = b
        self.dirty = True

    def _keyboard_grid(self, c: Canvas, dev: M.Device, area, selectable: bool = True,
                       on_click=None) -> None:
        """Draw a full-size keyboard. Keys show their lighting and whether they are remapped."""
        x0, y0, x1, y1 = area
        rows = M.KEYBOARD_ROWS
        main_units = 15.0            # the main block, including the backslash key
        # the wide gaps already account for the space between the three blocks, so
        # the row that runs furthest decides how wide the board is
        total_units = max(sum(w for _label, w in row) for row in rows)
        unit = min((x1 - x0) / total_units, (y1 - y0) / (len(rows) * 1.16))
        key_h = unit * 0.92
        row_gap = key_h * 0.18
        board_w = total_units * unit
        bx = x0 + ((x1 - x0) - board_w) / 2
        by = y0 + 4
        selected = self.selected_key.get(dev.id)

        for r, row in enumerate(rows):
            y = by + r * (key_h + row_gap)
            x = bx
            for label, w in row:
                if label == "gap":
                    x += w * unit
                    continue
                kw = w * unit - (unit * 0.06)
                box = (x, y, x + kw, y + key_h)
                self._draw_key(c, dev, box, label, selected, selectable, on_click)
                x += w * unit

    def _draw_key(self, c: Canvas, dev: M.Device, box, label: str, selected: str | None,
                  selectable: bool, on_click) -> None:
        col = self._key_colour(dev, label, box)
        hover = selectable and c.contains(box)
        lit = scale_color(col, 0.34)
        base = (34, 37, 44)
        if hover:
            base = (46, 51, 60)
        c.rect(box, fill=base, radius=4)
        # light layer
        c.rect((box[0] + 1.5, box[1] + 1.5, box[2] - 1.5, box[3] - 1.5), fill=lit, radius=3)
        c.rect((box[0] + 1.5, box[1] + 1.5, box[2] - 1.5, box[3] - 1.5),
               outline=scale_color(col, 0.55), width=1)
        binding = dev.bindings.get(label)
        if binding and binding.kind != "default":
            c.rect(box, fill=None, radius=4, outline=ACCENT, width=1.6)
            c.circle((box[2] - 5, box[1] + 5), 2.2, fill=ACCENT)
        if selected == label:
            c.rect((box[0] - 1, box[1] - 1, box[2] + 1, box[3] + 1), fill=None, radius=5,
                   outline=(255, 255, 255), width=1.8)
        if box[2] - box[0] > 13 and box[3] - box[1] > 11:
            fs = 8.5 if len(label) <= 2 else 7.2
            c.text(((box[0] + box[2]) / 2, box[3] - 6), label, fs, False, (226, 231, 238), anchor="mm")
        if selectable:
            c.register(box, ("key", label) if on_click is None else ("custom", on_click, label))

    def _key_colour(self, dev: M.Device, label: str, box):
        e = dev.effect
        b = e.brightness / 100.0
        zone_col = e.colour
        for z in dev.zones:
            if not z.on:
                continue
            if z.name == "WASD" and label in ("W", "A", "S", "D"):
                zone_col = z.colour
            elif z.name == "Indicators" and (label.startswith("F") or label in ("Esc", "PrtSc", "ScrLk", "Pause")):
                zone_col = z.colour
            elif z.name == "Underglow" and label in ("Space", "LCtrl", "RCtrl", "LAlt", "RAlt"):
                zone_col = z.colour
        t = self.now
        speed = 0.4 + e.speed / 100.0 * 2.2
        norm = (box[0] - 0) / max(1.0, self.width)
        if e.kind == "static":
            col = e.colour
        elif e.kind == "solid":
            col = zone_col
        elif e.kind == "breath":
            col = mix(e.colour, e.colour2, (math.sin(t * speed * 1.7) + 1) / 2)
        elif e.kind == "rainbow":
            seed = sum(ord(ch) for ch in label)
            col = hsv((seed * 7 + t * speed * 60) % 360, 0.82, 1.0)
        elif e.kind == "wave":
            col = mix(e.colour2, e.colour, (math.sin(norm * 9 - t * speed * 3) + 1) / 2)
        elif e.kind == "gradient":
            col = mix(e.colour, e.colour2, min(1.0, max(0.0, norm * 8 - 1.4)))
        elif e.kind == "ripple":
            cx, cy = 0.5, 0.52
            dist = math.hypot(norm * 2.4 - cx * 2.4, 0.5 - cy)
            col = mix(e.colour, e.colour2, (math.sin(dist * 12 - t * speed * 4) + 1) / 2)
        elif e.kind == "spiral":
            col = mix(e.colour, e.colour2, (math.sin(norm * 20 + 0.5 + t * speed * 2) + 1) / 2)
        else:
            col = e.colour
        return scale_color(col, 0.25 + 0.75 * b)

    # -- performance (mouse) ---------------------------------------------
    def _tab_performance(self, c: Canvas, box, dev: M.Device) -> None:
        x0, y0, x1, y1 = box
        left_w = int((x1 - x0) * 0.5)
        card = (x0, y0, x0 + left_w, y1)
        y = self._card(c, card, "DPI stages",
                       "Each stage is a speed you can switch between with a button.")
        stages = dev.dpi_stages or [800]
        for i, dpi in enumerate(stages):
            row = (card[0] + 14, y, card[2] - 14, y + 54)
            active = i == dev.active_stage
            c.rect(row, fill=(38, 42, 50) if active else PANEL_LO, radius=9,
                   outline=ACCENT if active else BORDER)
            c.text((row[0] + 14, row[1] + 16), f"Stage {i + 1}", 11, True, TEXT_DIM if not active else ACCENT)
            c.text((row[0] + 14, row[1] + 32), f"{dpi:,} DPI", 16, True, TEXT)
            if active:
                c.pill("ACTIVE", (row[0] + 100, row[1] + 30), ACCENT_TEXT, ACCENT, 9, 6)
            else:
                c.button((row[2] - 74, row[1] + 14, row[2] - 38, row[1] + 40), "Use",
                         ("stage", i), "ghost", 10.5)
            if len(stages) > 1:
                c.button((row[2] - 34, row[1] + 14, row[2] - 12, row[1] + 40), "",
                         ("stage_del", i), "ghost", 10.5, icon="trash")
            y += 60
        add_y = y + 2
        c.button((card[0] + 14, add_y, card[0] + 130, add_y + 30), "Add stage",
                 ("stage_add",), "secondary", 11, icon="plus")

        # DPI curve
        graph = (card[0] + 14, add_y + 44, card[2] - 14, y1 - 10)
        if graph[3] - graph[1] > 60:
            c.rect(graph, fill=(20, 23, 27), radius=9)
            c.text((graph[0] + 12, graph[1] + 8), "Pointer speed", 9.5, True, TEXT_FAINT)
            gx0, gy0, gx1, gy1 = graph[0] + 12, graph[1] + 26, graph[2] - 12, graph[3] - 14
            for i in range(4):
                gy = gy0 + (gy1 - gy0) * i / 3
                c.line([(gx0, gy), (gx1, gy)], fill=(34, 38, 45), width=1)
            max_dpi = max(stages) if stages else 800
            pts = []
            for i, dpi in enumerate(stages):
                px = gx0 + (gx1 - gx0) * (i / max(1, len(stages) - 1))
                py = gy1 - (gy1 - gy0) * (dpi / max(1, max_dpi))
                pts.append((px, py))
            if len(pts) > 1:
                c.line(pts, ACCENT, 2)
            for i, (px, py) in enumerate(pts):
                c.circle((px, py), 5 if i != dev.active_stage else 6.5,
                         fill=(20, 23, 27), outline=ACCENT, width=2)
                if i == dev.active_stage:
                    c.circle((px, py), 3, fill=ACCENT)

        # right column
        rx = x0 + left_w + 18
        rcard = (rx, y0, x1, y1)
        y = self._card(c, rcard, "Mouse performance")
        # active DPI
        c.text((rx + 18, y), "POINTER SPEED", 10, True, TEXT_FAINT)
        y += 18
        dpi_row = (rx + 18, y, rcard[2] - 18, y + 46)
        c.rect(dpi_row, fill=PANEL_LO, radius=9, outline=BORDER)
        c.text((dpi_row[0] + 14, (dpi_row[1] + dpi_row[3]) / 2),
               f"{stages[dev.active_stage] if stages else 800:,} DPI", 17, True, TEXT, anchor="lm")
        c.text((dpi_row[2] - 14, (dpi_row[1] + dpi_row[3]) / 2),
               f"stage {dev.active_stage + 1} of {len(stages)}", 10.5, False, TEXT_DIM, anchor="rm")
        y += 60

        # polling rate chips
        c.text((rx + 18, y), "POLLING RATE", 10, True, TEXT_FAINT)
        y += 18
        chip_w = (rcard[2] - rcard[0] - 36 - 3 * 8) / 4
        for i, hz in enumerate((125, 250, 500, 1000)):
            cb = (rx + 18 + i * (chip_w + 8), y, rx + 18 + i * (chip_w + 8) + chip_w, y + 36)
            active = dev.polling_rate == hz
            c.rect(cb, fill=(38, 42, 50) if active else PANEL_LO, radius=8,
                   outline=ACCENT if active else BORDER)
            c.text(((cb[0] + cb[2]) / 2, (cb[1] + cb[3]) / 2), f"{hz} Hz", 11, active,
                   ACCENT if active else TEXT_DIM, anchor="mm")
            c.register(cb, ("polling", hz))
        y += 56

        # lift-off
        c.text((rx + 18, y), "LIFT-OFF HEIGHT", 10, True, TEXT_FAINT)
        c.text((rcard[2] - 18, y), f"{dev.lift_off:.1f} mm", 10.5, True, TEXT, anchor="ra")
        y += 16
        c.slider(f"liftoff-{dev.id}", (rx + 18, y, rcard[2] - 18, y + 18), dev.lift_off, 0.5, 3.0,
                 lambda v, d=dev: self._set_liftoff(d, v))
        y += 34

        # toggles
        for label, value, action, sub in (
            ("Angle snapping", dev.angle_snapping, ("angle",),
             "Straightens mostly-horizontal mouse movement"),
        ):
            row = (rx + 18, y, rcard[2] - 18, y + 50)
            c.rect(row, fill=PANEL_LO, radius=9, outline=BORDER)
            c.text((row[0] + 14, row[1] + 14), label, 11.5, True, TEXT)
            c.text_in((row[0] + 14, row[1] + 30), sub, 10, False, TEXT_FAINT, pad=0)
            c.toggle((row[2] - 54, row[1] + 15, row[2] - 16, row[1] + 35), value, action)
            y += 58

        c.text((rx + 18, y + 4), "CMOS SENSOR", 10, True, TEXT_FAINT)
        c.text((rx + 18, y + 20), "18,000 DPI · 400 IPS · 50 G", 11.5, False, TEXT_DIM)

    def _set_liftoff(self, dev: M.Device, v: float) -> None:
        dev.lift_off = round(float(v), 1)
        self.dirty = True

    # -- mouse buttons ----------------------------------------------------
    def _tab_buttons(self, c: Canvas, box, dev: M.Device) -> None:
        x0, y0, x1, _y1 = box
        left_w = int((x1 - x0) * 0.5)
        card = (x0, y0, x0 + left_w, y0 + 96 + len(M.MOUSE_BUTTONS) * 34)
        y = self._card(c, card, "Button remapping", "What each button does.")
        for name in M.MOUSE_BUTTONS:
            b = dev.bindings.get(name) or M.Binding()
            row = (card[0] + 14, y, card[2] - 14, y + 30)
            if c.contains(row):
                c.rect(row, fill=(31, 34, 41), radius=7)
            c.text((row[0] + 10, y + 15), name, 11, True, TEXT, anchor="lm")
            desc = b.describe(name)
            c.text_in((row[0] + 140, y + 15), desc, 10.5, False,
                      ACCENT if b.kind != "default" else TEXT_DIM, anchor="lm", pad=0)
            c.icon("chevron-right", (row[2] - 12, y + 15), 9, TEXT_FAINT)
            c.register(row, ("button_pick", name))
            y += 34

        # the chosen button's action, edited in place below the list
        if self.button_sel:
            sel = self.button_sel
            b = dev.bindings.get(sel) or M.Binding()
            gy = min(y + 8, self.height - 160)
            gbox = (card[0], gy, card[2], min(self.height - 20, gy + 130))
            c.rect(gbox, fill=PANEL, radius=12, outline=BORDER_HI)
            c.text((gbox[0] + 16, gbox[1] + 14), f"{sel}", 12, True, TEXT)
            c.text((gbox[0] + 16 + c.measure(sel, 12, True)[0] + 8, gbox[1] + 14),
                   "— pick an action", 10.5, False, TEXT_DIM)
            for i, (kind, label) in enumerate(M.ACTION_KINDS[:3]):
                cb = (gbox[0] + 16 + i * 100, gbox[1] + 34, gbox[0] + 108 + i * 100, gbox[1] + 62)
                active = b.kind == kind
                c.rect(cb, fill=(42, 46, 55) if active else PANEL_LO, radius=7,
                       outline=ACCENT if active else BORDER)
                c.text(((cb[0] + cb[2]) / 2, (cb[1] + cb[3]) / 2), label.split(" (")[0], 10.5,
                       active, ACCENT if active else TEXT_DIM, anchor="mm")
                c.register(cb, ("button_kind", sel, kind))
            self._modal_control(c, (gbox[0], gbox[1] + 66, gbox[2], gbox[3]), "Action detail",
                                b.kind, b.detail, lambda v, n=sel: self._set_button_detail(n, v))

        rx = x0 + left_w + 18
        side = (rx, y0, x1, y0 + 300)
        c.rect(side, fill=PANEL, radius=14, outline=BORDER)
        c.text((rx + 18, y0 + 16), "Mouse layout", 13, True, TEXT)
        c.text((rx + 18, y0 + 36), "Buttons with a change are marked.", 11, False, TEXT_DIM)
        self._mouse_diagram(c, dev, (rx + 18, y0 + 58, x1 - 18, y0 + 290))

    def _set_button_detail(self, name: str, value: str) -> None:
        dev = self.device()
        if not dev:
            return
        b = dev.bindings.get(name) or M.Binding()
        b.detail = value
        dev.bindings[name] = b
        self.dirty = True


    def _mouse_diagram(self, c: Canvas, dev: M.Device, area) -> None:
        x0, y0, x1, y1 = area
        cx = (x0 + x1) / 2
        h = min(y1 - y0, 232)
        w = h * 0.52
        top = y0 + (y1 - y0 - h) / 2
        body = (cx - w / 2, top, cx + w / 2, top + h)
        col0 = dev.zones[0].colour if dev.zones else ACCENT
        col1 = dev.zones[1].colour if len(dev.zones) > 1 else BLUE
        col2 = dev.zones[2].colour if len(dev.zones) > 2 else PURPLE
        b = dev.effect.brightness / 100.0
        c.rect(body, fill=(38, 42, 50), radius=w / 2)
        c.rect(body, fill=None, radius=w / 2, outline=BORDER_HI, width=1.4)
        # left/right buttons
        c.rect((body[0] + 5, body[1] + 6, cx - 2, body[1] + h * 0.30), fill=(48, 53, 62),
               radius=w / 3)
        c.rect((cx + 2, body[1] + 6, body[2] - 5, body[1] + h * 0.30), fill=(48, 53, 62),
               radius=w / 3)
        c.line([(cx, body[1] + 8), (cx, body[1] + h * 0.30)], fill=(30, 33, 39), width=1.4)
        # scroll wheel lit
        c.rect((cx - 4, body[1] + 30, cx + 4, body[1] + 62), fill=scale_color(col1, 0.35 + b * 0.65),
               radius=4)
        # side strip
        c.rect((body[0] + 2, body[1] + h * 0.44, body[0] + 7, body[1] + h * 0.72),
               fill=scale_color(col2, 0.3 + b * 0.7), radius=2.5)
        # logo
        c.circle((cx, body[1] + h * 0.62), w * 0.16, fill=scale_color(col0, 0.3 + b * 0.7))
        c.circle((cx, body[1] + h * 0.62), w * 0.16, outline=(0, 0, 0, 90))
        # dpi indicator
        if len(dev.zones) > 3:
            c.circle((cx, body[1] + h * 0.80), 3.5, fill=scale_color(dev.zones[3].colour, 0.4 + b * 0.6))
        # labels for remapped buttons
        marks = [("Left click", body[0] + 8, body[1] + h * 0.18, "lm"),
                 ("Right click", body[2] - 8, body[1] + h * 0.18, "rm")]
        for name, mx, my, anchor in marks:
            bd = dev.bindings.get(name)
            if bd and bd.kind != "default":
                c.circle((mx, my), 3.5, fill=ACCENT)

    # -- device settings --------------------------------------------------
    def _tab_device_settings(self, c: Canvas, box, dev: M.Device) -> None:
        x0, y0, _x1, _y1 = box
        card = (x0, y0, x0 + 420, y0 + 320)
        y = self._card(c, card, "Device information")
        for label, value in (("Model", dev.name), ("Type", dev.kind.capitalize()),
                             ("Connection", dev.transport if dev.connected else "Not connected"),
                             ("Firmware", dev.firmware), ("Lighting zones", str(len(dev.zones)))):
            c.text((card[0] + 18, y), label, 11.5, False, TEXT_DIM)
            c.text_in((card[0] + 130, y), value, 11.5, True, TEXT, pad=0)
            y += 26
        y += 8
        c.line([(card[0] + 18, y - 6), (card[2] - 18, y - 6)], fill=BORDER, width=1)
        c.text((card[0] + 18, y), "Firmware updates are not applied by this app.", 10.5, False, TEXT_FAINT)

        card2 = (x0 + 440, y0, x0 + 860, y0 + 320)
        y = self._card(c, card2, "Device options")
        if dev.kind == "keyboard":
            row = (card2[0] + 14, y, card2[2] - 14, y + 52)
            c.rect(row, fill=PANEL_LO, radius=9, outline=BORDER)
            c.text((row[0] + 14, row[1] + 14), "Windows key lock", 11.5, True, TEXT)
            c.text_in((row[0] + 14, row[1] + 30), "Stops the Windows key from opening the menu", 10,
                      False, TEXT_FAINT, pad=0)
            c.toggle((row[2] - 54, row[1] + 16, row[2] - 16, row[1] + 36), dev.win_lock, ("win_lock",))
            y += 62
        row = (card2[0] + 14, y, card2[2] - 14, y + 52)
        c.rect(row, fill=PANEL_LO, radius=9, outline=BORDER)
        c.text((row[0] + 14, row[1] + 14), "Polling rate", 11.5, True, TEXT)
        c.text_in((row[0] + 14, row[1] + 30), "How often the device reports to the computer", 10,
                  False, TEXT_FAINT, pad=0)
        pb = (row[2] - 96, row[1] + 14, row[2] - 14, row[1] + 40)
        c.rect(pb, fill=PANEL_HI, radius=7, outline=BORDER_HI)
        c.text_in((pb[0] + 10, (pb[1] + pb[3]) / 2), f"{dev.polling_rate} Hz", 10.5, True, TEXT,
                  anchor="lm", pad=0)
        c.register(pb, ("pick", "polling", "Polling rate",
                        [(str(v), f"{v} Hz") for v in (125, 250, 500, 1000)], str(dev.polling_rate),
                        lambda v, d=dev: self._set_polling(d, int(v))))
        y += 62
        c.button((card2[0] + 14, y, card2[0] + 150, y + 32), "Reset device",
                 ("reset_device",), "danger", 11, icon="trash")

    def _set_polling(self, dev: M.Device, hz: int) -> None:
        dev.polling_rate = hz
        self.dirty = True

    # =====================================================================
    # device preview used on the dashboard and lighting tab
    # =====================================================================
    def _device_preview(self, c: Canvas, dev: M.Device, box, interactive: bool = False) -> None:
        x0, y0, x1, y1 = box
        if dev.kind == "keyboard":
            # subtle glow behind the keyboard, as iCUE does
            glow = dev.lit_colour(self.now)
            c.rect((x0 + 10, y1 - 14, x1 - 10, y1 - 4), fill=(glow[0] // 4, glow[1] // 4, glow[2] // 4), radius=6)
            self._keyboard_grid(c, dev, (x0, y0, x1, y1 - 16), selectable=interactive)
        else:
            self._mouse_diagram(c, dev, (x0 + (x1 - x0) * 0.18, y0, x1 - (x1 - x0) * 0.18, y1))

    # =====================================================================
    # actions page
    # =====================================================================
    def _page_actions(self, c: Canvas, box) -> None:
        x0, y0, x1, y1 = box
        y = self._page_title(c, x0, y0, "Actions",
                             "Macros and shortcuts you can assign to any key or button.")
        self._save_button(c, (x1 - 104, y0 - 2, x1, y0 + 32))

        left_w = int((x1 - x0) * 0.56)
        card = (x0, y, x0 + left_w, y1)
        cy = self._card(c, card, "Macros")
        rows = self.lib.actions.macros
        for i, m in enumerate(rows):
            row = (card[0] + 14, cy, card[2] - 14, cy + 52)
            hover = c.contains(row)
            c.rect(row, fill=(31, 34, 41) if hover else PANEL_LO, radius=9, outline=BORDER)
            c.rect((row[0] + 12, row[1] + 14, row[0] + 36, row[1] + 38), fill=(44, 49, 58), radius=6)
            c.icon("bolt", (row[0] + 24, row[1] + 26), 11, ACCENT)
            c.text_in((row[0] + 46, row[1] + 16), m.get("name", "Macro"), 12, True, TEXT, pad=0)
            c.text_in((row[0] + 46, row[1] + 33), m.get("keys", ""), 10.5, False, TEXT_DIM, pad=0)
            if m.get("delay"):
                c.pill(f"{m['delay']} ms", (row[2] - 92, row[1] + 16), TEXT_DIM, None, 9)
            c.button((row[2] - 36, row[1] + 13, row[2] - 12, row[1] + 39), "", ("macro_del", i),
                     "ghost", 10.5, icon="trash")
            cy += 58
        if not rows:
            c.text((card[0] + 18, cy), "No macros yet.", 11.5, False, TEXT_DIM)
        cy = max(cy, card[3] - 220)
        c.line([(card[0] + 14, cy), (card[2] - 14, cy)], fill=BORDER, width=1)
        cy += 16
        c.text((card[0] + 14, cy), "NEW MACRO", 10, True, TEXT_FAINT)
        cy += 18
        self._text_input(c, ("macro_name",), (card[0] + 14, cy, card[0] + 212, cy + 32),
                         self.macro_draft.get("name", ""), "Name", commit=self._macro_field("name"))
        self._text_input(c, ("macro_keys",), (card[0] + 222, cy, card[2] - 80, cy + 32),
                         self.macro_draft.get("keys", ""), "Keys, e.g. Ctrl + Shift + M",
                         commit=self._macro_field("keys"))
        c.button((card[2] - 72, cy, card[2] - 14, cy + 32), "Add", ("macro_add",), "primary", 11, icon="plus")

        rx = x0 + left_w + 18
        side = (rx, y, x1, y1)
        cy = self._card(c, side, "Where actions are used")
        used = []
        for dev in self.lib.profile().devices:
            for k, b in dev.bindings.items():
                if b.kind in ("macro", "media", "key", "app"):
                    used.append((f"{dev.name} · {k}", b.describe(k)))
        if not used:
            self._empty_note(c, (side[0] + 18, cy, side[2] - 18, cy + 80), "Nothing is remapped yet.",
                             "Open a device and pick an action for a key.")
        for i, (where, what) in enumerate(used[:8]):
            row_y = cy + i * 42
            c.text_in((side[0] + 18, row_y), where, 11, True, TEXT, pad=0)
            c.text_in((side[0] + 18, row_y + 16), what, 10.5, False, TEXT_DIM, pad=0)

    def _empty_note(self, c: Canvas, box, head: str, body: str) -> None:
        x0, y0, x1, y1 = box
        c.rect(box, fill=(20, 23, 27), radius=9)
        c.text_in((x0 + 14, y0 + 16), head, 11.5, True, TEXT_DIM, pad=0)
        c.wrap((x0 + 14, y0 + 34, x1 - 14, y1), body, 10.5, TEXT_FAINT)

    # =====================================================================
    # profiles page
    # =====================================================================
    def _page_profiles(self, c: Canvas, box) -> None:
        x0, y0, x1, y1 = box
        y = self._page_title(c, x0, y0, "Profiles",
                             "A profile keeps every device's settings together.")
        self._save_button(c, (x1 - 104, y0 - 2, x1, y0 + 32))
        left_w = 420
        card = (x0, y, x0 + left_w, y1)
        cy = self._card(c, card, "Profiles")
        for i, p in enumerate(self.lib.profiles):
            row = (card[0] + 14, cy, card[2] - 14, cy + 62)
            active = i == self.lib.active_profile
            c.rect(row, fill=(38, 42, 50) if active else PANEL_LO, radius=9,
                   outline=ACCENT if active else BORDER)
            c.text_in((row[0] + 14, row[1] + 13), p.name, 12.5, True, TEXT, pad=0)
            sub = p.kind.capitalize() + (f" · linked to {os.path.basename(p.linked_to)}" if p.linked_to else "")
            c.text_in((row[0] + 14, row[1] + 32), sub, 10.5, False, TEXT_DIM, pad=0)
            c.text_in((row[0] + 14, row[1] + 46), f"{len(p.devices)} device(s)", 10, False, TEXT_FAINT, pad=0)
            if active:
                c.pill("ACTIVE", (row[2] - 74, row[1] + 12), ACCENT_TEXT, ACCENT, 9, 6)
            else:
                c.button((row[2] - 70, row[1] + 12, row[2] - 14, row[1] + 36), "Switch",
                         ("profile_use", i), "ghost", 10.5)
            c.register((row[0], row[1], row[0] + 200, row[1] + 30), ("profile_select", i))
            cy += 70
        cy = max(cy, y1 - 150)
        c.line([(card[0] + 14, cy), (card[2] - 14, cy)], fill=BORDER, width=1)
        cy += 16
        self._text_input(c, ("profile_name",), (card[0] + 14, cy, card[2] - 120, cy + 32), "",
                         "New profile name")
        c.button((card[2] - 112, cy, card[2] - 14, cy + 32), "Create", ("profile_add",), "primary", 11,
                 icon="plus")
        cy += 44
        c.button((card[0] + 14, cy, card[0] + 120, cy + 32), "Duplicate", ("profile_dup",), "secondary", 11)
        c.button((card[0] + 128, cy, card[0] + 236, cy + 32), "Delete", ("profile_del",), "danger", 11,
                 icon="trash")

        rx = x0 + left_w + 18
        side = (rx, y, x1, y1)
        prof = self.lib.profile()
        cy = self._card(c, side, "Profile details")
        c.text((side[0] + 18, cy), "PROFILE NAME", 10, True, TEXT_FAINT)
        cy += 18
        self._text_input(c, ("profile_rename",), (side[0] + 18, cy, side[2] - 18, cy + 34), prof.name,
                         "Profile name", commit=self._rename_profile)
        cy += 46
        c.text((side[0] + 18, cy), "PROFILE TYPE", 10, True, TEXT_FAINT)
        cy += 18
        for value, label, desc in (("software", "Software profile", "Saved on this computer"),
                                   ("hardware", "Hardware profile", "Saved on the device itself")):
            row = (side[0] + 14, cy, side[2] - 14, cy + 44)
            active = prof.kind == value
            if active:
                c.rect(row, fill=(38, 42, 50), radius=8)
            c.circle((row[0] + 16, row[1] + 22), 5, outline=ACCENT if active else (90, 97, 108), width=1.6)
            if active:
                c.circle((row[0] + 16, row[1] + 22), 2.4, fill=ACCENT)
            c.text((row[0] + 30, row[1] + 12), label, 11.5, active, TEXT if active else TEXT_DIM)
            c.text_in((row[0] + 30, row[1] + 28), desc, 10, False, TEXT_FAINT, pad=0)
            c.register(row, ("profile_kind", value))
            cy += 50
        cy += 6
        c.text((side[0] + 18, cy), "LINK TO APPLICATION", 10, True, TEXT_FAINT)
        c.text_in((side[0] + 18, cy + 14), "Switch to this profile when a program starts.", 10, False,
                  TEXT_DIM, pad=0)
        cy += 32
        self._text_input(c, ("profile_link",), (side[0] + 18, cy, side[2] - 18, cy + 32), prof.linked_to,
                         "e.g. /usr/bin/game", commit=self._link_profile)
        cy += 44
        c.text((side[0] + 18, cy), "DEVICES IN THIS PROFILE", 10, True, TEXT_FAINT)
        cy += 18
        for dev in prof.devices:
            row = (side[0] + 18, cy, side[2] - 18, cy + 30)
            c.rect((row[0], row[1] + 7, row[0] + 8, row[1] + 23),
                   fill=dev.lit_colour(self.now), radius=2)
            c.text_in((row[0] + 16, row[1] + 15), dev.name, 11, False, TEXT_DIM, anchor="lm", pad=0)
            c.text_in((row[2] - 90, row[1] + 15), f"{dev.effect.brightness}%", 10.5, True, TEXT,
                      anchor="rm", pad=0)
            cy += 32

    def _rename_profile(self, value: str) -> None:
        if value.strip():
            self.lib.profile().name = value.strip()
            self.dirty = True
            self.say("Profile renamed")

    def _link_profile(self, value: str) -> None:
        self.lib.profile().linked_to = value.strip()
        self.dirty = True

    # =====================================================================
    # settings page
    # =====================================================================
    def _page_settings(self, c: Canvas, box) -> None:
        x0, y0, x1, y1 = box
        y = self._page_title(c, x0, y0, "Settings",
                             "How the app starts up, saves and behaves.")
        self._save_button(c, (x1 - 104, y0 - 2, x1, y0 + 32))
        col_w = (x1 - x0 - 18) / 2
        s = self.lib.settings

        card = (x0, y, x0 + col_w, y1)
        cy = self._card(c, card, "General")
        for label, key, sub in (
            ("Start when you sign in", "start_at_login", "Launch the app with your desktop"),
            ("Start minimised", "start_minimised", "Open in the background tray"),
            ("Minimise to tray", "minimise_to_tray", "Keep running when the window is closed"),
            ("Enable game integrations", "enable_sdk", "Let games set the lighting"),
            ("Show demo devices", "show_demo_devices", "Show the sample keyboard and mouse"),
            ("Check for updates", "check_for_updates", "Look for a newer version at launch"),
        ):
            row = (card[0] + 14, cy, card[2] - 14, cy + 50)
            c.rect(row, fill=PANEL_LO, radius=9, outline=BORDER)
            c.text((row[0] + 14, row[1] + 14), label, 11.5, True, TEXT)
            c.text_in((row[0] + 14, row[1] + 30), sub, 10, False, TEXT_FAINT, pad=0)
            c.toggle((row[2] - 54, row[1] + 15, row[2] - 16, row[1] + 35), bool(getattr(s, key)),
                     ("setting_toggle", key))
            cy += 58
        # keyboard use, which a gaming keyboard app should say out loud
        ky = cy + 6
        if ky + 120 < card[3]:
            c.line([(card[0] + 18, ky), (card[2] - 18, ky)], fill=BORDER, width=1)
            c.text((card[0] + 18, ky + 14), "OTHER PROFILES", 10, True, TEXT_FAINT)
            c.text_in((card[0] + 18, ky + 30),
                      "A new profile starts with a copy of the devices in the active profile.",
                      10.5, False, TEXT_DIM, pad=0)
            view = c.measure("View profiles", 11, False)[0] + 34
            c.button((card[0] + 18, ky + 52, card[0] + 18 + view, ky + 82), "View profiles",
                     ("goto", "profiles"), "secondary", 11, icon="chevron-right")
            hw = M.detect_hardware()
            c.text_in((card[0] + 18, ky + 96),
                      f"{len(hw)} Corsair device(s) found in the system device list."
                      if hw else "No Corsair devices in the system device list right now.",
                      10, False, TEXT_FAINT, pad=0)

        side = (x0 + col_w + 18, y, x1, y1)
        cy = self._card(c, side, "Lighting and devices")
        c.text((side[0] + 18, cy), "GLOBAL BRIGHTNESS", 10, True, TEXT_FAINT)
        c.text((side[2] - 18, cy), f"{s.brightness}%", 10.5, True, TEXT, anchor="ra")
        cy += 16
        c.slider("global-bright", (side[0] + 18, cy, side[2] - 18, cy + 18), s.brightness, 0, 100,
                 self._set_global_brightness)
        cy += 34
        c.text((side[0] + 18, cy), "DEFAULT POLLING RATE", 10, True, TEXT_FAINT)
        cy += 18
        chip_w = (side[2] - side[0] - 36 - 24) / 4
        for i, hz in enumerate((125, 250, 500, 1000)):
            cb = (side[0] + 18 + i * (chip_w + 8), cy, side[0] + 18 + i * (chip_w + 8) + chip_w, cy + 34)
            active = s.polling_rate_hz == hz
            c.rect(cb, fill=(38, 42, 50) if active else PANEL_LO, radius=8,
                   outline=ACCENT if active else BORDER)
            c.text(((cb[0] + cb[2]) / 2, (cb[1] + cb[3]) / 2), f"{hz}", 11, active,
                   ACCENT if active else TEXT_DIM, anchor="mm")
            c.register(cb, ("setting_polling", hz))
        cy += 48
        c.line([(side[0] + 18, cy), (side[2] - 18, cy)], fill=BORDER, width=1)
        cy += 16
        c.text((side[0] + 18, cy), "STORAGE", 10, True, TEXT_FAINT)
        cy += 18
        path = M.config_path()
        c.text_in((side[0] + 18, cy), path, 10, False, TEXT_DIM, pad=0)
        cy += 20
        c.text_in((side[0] + 18, cy), "Profiles and preferences are saved here as JSON.", 10, False,
                  TEXT_FAINT, pad=0)
        cy += 26
        c.button((side[0] + 18, cy, side[0] + 150, cy + 32), "Save now", ("save",), "secondary", 11,
                 icon="save")
        cy += 46
        c.line([(side[0] + 18, cy), (side[2] - 18, cy)], fill=BORDER, width=1)
        cy += 16
        c.text((side[0] + 18, cy), "ABOUT", 10, True, TEXT_FAINT)
        cy += 18
        c.text_in((side[0] + 18, cy), "RGBCUE 1.0 — a native Linux companion for Corsair keyboards and mice.",
                  10.5, False, TEXT_DIM, pad=0, ellipsis=False)
        cy += 18
        c.wrap((side[0] + 18, cy, side[2] - 18, side[3]), 
               "This app draws its own interface and stores your profiles locally. It does not send anything "
               "anywhere, and it needs no account.", 10, TEXT_FAINT)

    def _set_global_brightness(self, v: float) -> None:
        self.lib.settings.brightness = int(round(v))
        for dev in self.lib.profile().devices:
            dev.effect.brightness = int(round(v))
        self.dirty = True

    # -- one control that covers every kind of detail a binding can need ---
    def _control(self, c: Canvas, box, binding: M.Binding, label: str | None,
                 commit) -> float:
        """Draws the detail control for a binding (a value, a list or a path)."""
        x0, y0, x1, _y1 = box
        if label:
            c.text((x0 + 18, y0), label, 10, True, TEXT_FAINT)
            y0 += 16
        if binding.kind == "app":
            self._text_input(c, ("binding_path",), (x0 + 18, y0, x1 - 18, y0 + 30),
                             binding.detail, "Type a path, then press Enter", commit=commit)
            return y0 + 38
        if binding.kind in ("key", "media", "macro", "dpi", "profile"):
            dev = self.device()
            options = self._detail_options(dev, binding.kind) if dev else []
            cur = binding.detail or (options[0][1] if options else "")
            pb = (x0 + 18, y0, x1 - 18, y0 + 32)
            outline = BORDER_HI if c.contains(pb) else BORDER
            c.rect(pb, fill=(20, 23, 27), radius=8, outline=outline)
            c.text_in((pb[0] + 12, y0 + 16), cur or "Choose…", 11.5, False, TEXT, anchor="lm", pad=0)
            c.icon("chevron-down", (pb[2] - 14, y0 + 16), 10, TEXT_DIM)
            c.register(pb, ("pick", label or "Choose", options, cur, commit))
            return y0 + 40
        return y0 - 16

    def _modal_control(self, c: Canvas, box, title: str, kind: str, current: str,
                       commit) -> None:
        """The same control, drawn inside a card with its own title (mouse buttons)."""
        x0, y0, x1, y1 = box
        if kind == "app":
            self._text_input(c, ("button_path", title), (x0 + 16, y0 + 24, x1 - 16, y0 + 54),
                             current, "Type a path, then press Enter", commit=commit)
            return
        dev = self.device()
        options = self._detail_options(dev, kind) if dev else []
        cur = current or (options[0][1] if options else "")
        pb = (x0 + 16, y0 + 24, x1 - 16, y0 + 56)
        c.rect(pb, fill=(20, 23, 27), radius=8, outline=BORDER_HI if c.contains(pb) else BORDER)
        c.text_in((pb[0] + 12, y0 + 40), cur or "Choose…", 11.5, False, TEXT, anchor="lm", pad=0)
        c.icon("chevron-down", (pb[2] - 14, y0 + 40), 10, TEXT_DIM)
        c.register(pb, ("pick", title, options, cur, commit))

    # =====================================================================
    # text input, popup, toast
    # =====================================================================
    def _text_input(self, c: Canvas, key: tuple, box, value: str, placeholder: str,
                    commit=None) -> None:
        focused = self.editing is not None and self.editing.get("key") == key
        text = self.editing["text"] if focused else value
        hover = c.contains(box)
        c.rect(box, fill=(18, 21, 25), radius=8,
               outline=ACCENT if focused else (BORDER_HI if hover else BORDER))
        shown = text if text else ""
        c.text_in((box[0] + 10, (box[1] + box[3]) / 2), shown or placeholder, 11.5, False,
                  TEXT if shown else TEXT_FAINT, anchor="lm", pad=0)
        if focused:
            tw = c.measure(shown, 11.5, False)[0]
            caret_x = box[0] + 10 + min(tw, box[2] - box[0] - 20)
            if (int(self.now * 2) % 2) == 0:
                c.line([(caret_x + 1, box[1] + 8), (caret_x + 1, box[3] - 8)], fill=ACCENT, width=1.4)
        c.register(box, ("focus", key, value, placeholder,
                         commit if commit is not None else self._default_commit(key)))

    def _default_commit(self, key: tuple):
        if key and key[0] == "macro_add_field":
            return None
        return None

    def _commit_text(self, key: tuple, text: str) -> None:
        lib = self.lib
        if key[0] == "profile_name" and text.strip():
            lib.profiles.append(M.Profile(text.strip(), "software", "",
                                          copy.deepcopy(lib.profile().devices)))
            lib.active_profile = len(lib.profiles) - 1
            self.say(f"Created profile “{text.strip()}”")
        elif key[0] == "profile_rename" and text.strip():
            lib.profile().name = text.strip()
            self.say("Profile renamed")
        elif key[0] == "profile_link":
            lib.profile().linked_to = text.strip()
        elif key[0] == "macro_name" or key[0] == "macro_keys":
            return  # handled by the Add button
        elif key[0] == "binding_detail":
            dev = self.device()
            if dev and len(key) >= 3:
                self._set_binding_detail(dev, key[2], text)
        self.dirty = True

    def _draw_popup(self, c: Canvas) -> None:
        p = self.popup
        opts = p["options"]
        row_h = 30
        width = 320
        max_rows = 11
        visible = opts[: max_rows * 1]
        height = min(len(visible), max_rows) * row_h + 44
        x = min(max(60, self.width / 2 - width / 2), self.width - width - 60)
        y = max(60, self.height / 2 - height / 2)
        c.rect((0, 0, self.width, self.height), fill=(0, 0, 0, 120))
        box = (x, y, x + width, y + height)
        c.rect(box, fill=(28, 31, 38), radius=12, outline=BORDER_HI)
        c.text_in((x + 16, y + 12), p.get("title", "Choose"), 11.5, True, TEXT_DIM, pad=0)
        c.icon("close", (box[2] - 18, y + 18), 10, TEXT_DIM)
        c.register((box[2] - 32, y + 4, box[2] - 4, y + 32), ("popup_close",))
        scroll = p.get("scroll", 0)
        shown = visible[scroll: scroll + max_rows]
        ry = y + 36
        if len(opts) > max_rows:
            c.text((box[2] - 16, y + 18), f"{len(opts)} ▾", 9, False, TEXT_FAINT, anchor="rm")
        for value, label in shown:
            row = (x + 8, ry, box[2] - 8, ry + row_h - 2)
            hover = c.contains(row)
            current = str(value) == str(p.get("current"))
            if current:
                c.rect(row, fill=(42, 46, 55), radius=7)
            elif hover:
                c.rect(row, fill=(35, 38, 45), radius=7)
            c.text_in((row[0] + 10, (row[1] + row[3]) / 2), label, 11.5, current,
                      ACCENT if current else TEXT, anchor="lm", pad=0)
            if current:
                c.icon("check", (row[2] - 14, (row[1] + row[3]) / 2), 10, ACCENT)
            c.register(row, ("popup_pick", value))
            ry += row_h

    def _draw_toast(self, c: Canvas) -> None:
        text, until = self.toast
        if not text or self.now > until:
            return
        alpha = min(1.0, max(0.0, (until - self.now) / 0.6))
        w = c.measure(text, 12, True)[0] + 52
        box = (self.width / 2 - w / 2, self.height - 74, self.width / 2 + w / 2, self.height - 32)
        c.rect(box, fill=(30, 34, 40, int(240 * alpha)), radius=10,
               outline=(70, 78, 92, int(220 * alpha)))
        c.circle((box[0] + 20, (box[1] + box[3]) / 2), 6, fill=(*GREEN, int(255 * alpha)))
        c.icon("check", (box[0] + 20, (box[1] + box[3]) / 2), 9, (16, 20, 24))
        c.text(((box[0] + box[2]) / 2 + 10, (box[1] + box[3]) / 2), text, 12, True,
               (*TEXT, int(255 * alpha)), anchor="mm")

    # =====================================================================
    # input
    # =====================================================================
    def on_move(self, x: float, y: float) -> None:
        self.pointer = (x, y)
        if self.dragging and self.dragging in self.sliders:
            box, lo, hi, cb, _fmt = self.sliders[self.dragging]
            frac = (x - box[0]) / max(1.0, box[2] - box[0])
            cb(lo + max(0.0, min(1.0, frac)) * (hi - lo))

    def on_press(self, x: float, y: float) -> None:
        self.pointer = (x, y)
        # popup takes every click while it is open
        if self.popup:
            for box, action in reversed(self.regions):
                if _inside(box, x, y):
                    if action[0] == "popup_pick":
                        cb = self.popup.get("cb")
                        self.popup = None
                        if cb:
                            cb(action[1])
                        return
                    if action[0] == "popup_close":
                        self.popup = None
                        return
                    return
            self.popup = None
            return
        for box, action in reversed(self.regions):
            if not _inside(box, x, y):
                continue
            if action[0] == "slider":
                self.dragging = action[1]
                if action[1] in self.sliders:
                    sbox, lo, hi, cb, _fmt = self.sliders[action[1]]
                    frac = (x - sbox[0]) / max(1.0, sbox[2] - sbox[0])
                    cb(lo + max(0.0, min(1.0, frac)) * (hi - lo))
                return
            # a click elsewhere ends any text editing
            if action[0] != "focus":
                if self.editing:
                    self._finish_edit()
            self._handle(action)
            return
        if self.editing:
            self._finish_edit()

    def on_release(self) -> None:
        self.dragging = None

    def on_scroll(self, x: float, y: float, dy: float) -> None:
        if self.popup and len(self.popup["options"]) > 11:
            self.popup["scroll"] = max(0, min(len(self.popup["options"]) - 11,
                                              self.popup.get("scroll", 0) + (1 if dy < 0 else -1)))

    def on_key(self, name: str) -> None:
        if self.editing:
            if name == "Return":
                self._finish_edit()
            elif name == "Escape":
                self.editing = None
            elif name == "BackSpace":
                self.editing["text"] = self.editing["text"][:-1]
            elif name == "space":
                self.editing["text"] += " "
            else:
                if len(name) == 1:
                    self.editing["text"] += name
            return
        if name == "Escape":
            if self.popup:
                self.popup = None
            elif self.page != "dashboard":
                self.page = "dashboard"

    def on_text(self, s: str) -> None:
        if self.editing is not None:
            for ch in s:
                if ch.isprintable():
                    self.editing["text"] += ch

    def _finish_edit(self) -> None:
        if not self.editing:
            return
        key = self.editing["key"]
        text = self.editing["text"]
        cb = self.editing.get("commit")
        self.editing = None
        if cb:
            cb(text)
        else:
            self._commit_text(key, text)

    def _open_popup(self, title: str, options, current, cb) -> None:
        self.popup = {"title": title, "options": list(options), "current": current, "cb": cb, "scroll": 0}

    # -- action dispatch ---------------------------------------------------
    def _handle(self, action) -> None:
        a = action[0]
        dev = self.device()
        if a == "goto":
            self.goto(action[1])
        elif a == "tab":
            if dev:
                self.tab[dev.id] = action[1]
        elif a == "save":
            path = M.save(self.lib)
            self.dirty = False
            self.say(f"Saved to {os.path.basename(os.path.dirname(path))}/library.json")
        elif a == "profile_picker":
            opts = [(str(i), p.name) for i, p in enumerate(self.lib.profiles)]
            self._open_popup("Switch profile", opts, str(self.lib.active_profile), self._use_profile)
        elif a == "profile_use":
            self._use_profile(action[1])
        elif a == "profile_select":
            self.lib.active_profile = action[1]
            self.device_id = self._first_device_id()
            self.dirty = True
        elif a == "profile_add":
            self.editing = {"key": ("profile_name",), "text": "", "commit": ("commit",)}
        elif a == "profile_dup":
            prof = copy.deepcopy(self.lib.profile())
            prof.name = prof.name + " copy"
            self.lib.profiles.append(prof)
            self.lib.active_profile = len(self.lib.profiles) - 1
            self.dirty = True
            self.say("Profile duplicated")
        elif a == "profile_del":
            if len(self.lib.profiles) > 1:
                name = self.lib.profile().name
                del self.lib.profiles[self.lib.active_profile]
                self.lib.active_profile = 0
                self.device_id = self._first_device_id()
                self.dirty = True
                self.say(f"Deleted “{name}”")
            else:
                self.say("The last profile cannot be deleted")
        elif a == "profile_kind":
            self.lib.profile().kind = action[1]
            self.dirty = True
        elif a == "effect" and dev:
            dev.effect.kind = action[1]
            self.dirty = True
        elif a == "colour" and dev:
            _row, idx = action[1], action[2]
            col = ZONE_COLORS[idx][1]
            if action[1] == 0:
                dev.effect.colour = col
            else:
                dev.effect.colour2 = col
            self.dirty = True
        elif a == "zone" and dev:
            for z in dev.zones:
                if z.name == action[2]:
                    z.on = not z.on
                    self.dirty = True
                    break
        elif a == "open_lighting":
            if dev:
                self.tab[dev.id] = "Lighting"
        elif a == "zone_toggle" and dev:
            z = dev.zones[action[1]]
            z.on = not z.on
            self.dirty = True
        elif a == "zone_select" and dev:
            z = dev.zones[action[1]]
            dev.effect.colour = z.colour
            dev.effect.kind = "static"
            self.dirty = True
        elif a == "key" and dev:
            self.selected_key[dev.id] = action[1]
        elif a == "binding_kind" and dev:
            k = self.selected_key.get(dev.id) or "Q"
            b = dev.bindings.setdefault(k, M.Binding())
            b.kind = action[1]
            if not b.detail:
                opts = self._detail_options(dev, b.kind)
                b.detail = opts[0][0] if opts else ""
            if action[1] == "default":
                dev.bindings.pop(k, None)
            self.dirty = True
        elif a == "binding_reset" and dev:
            dev.bindings.pop(action[1], None)
            self.dirty = True
        elif a == "reset_keys" and dev:
            dev.bindings.clear()
            self.dirty = True
            self.say("All keys back to their defaults")
        elif a == "reset_device" and dev:
            fresh = M.demo_keyboard() if dev.kind == "keyboard" else M.demo_mouse()
            fresh.id, fresh.name, fresh.connected = dev.id, dev.name, dev.connected
            prof = self.lib.profile()
            idx = prof.devices.index(dev)
            prof.devices[idx] = fresh
            self.dirty = True
            self.say("Device settings reset")
        elif a == "stage" and dev:
            dev.active_stage = action[1]
            self.dirty = True
        elif a == "stage_add" and dev:
            last = dev.dpi_stages[-1] if dev.dpi_stages else 800
            dev.dpi_stages.append(min(18000, last * 2))
            self.dirty = True
        elif a == "stage_del" and dev:
            if len(dev.dpi_stages) > 1:
                del dev.dpi_stages[action[1]]
                dev.active_stage = min(dev.active_stage, len(dev.dpi_stages) - 1)
                self.dirty = True
        elif a == "polling" and dev:
            dev.polling_rate = action[1]
            self.dirty = True
        elif a == "angle" and dev:
            dev.angle_snapping = not dev.angle_snapping
            self.dirty = True
        elif a == "win_lock" and dev:
            dev.win_lock = not dev.win_lock
            self.dirty = True
        elif a == "button_pick" and dev:
            self.button_sel = action[1]
        elif a == "button_kind" and dev:
            name, kind = action[1], action[2]
            if kind == "default":
                dev.bindings.pop(name, None)
            else:
                b = dev.bindings.get(name) or M.Binding()
                b.kind = kind
                if not b.detail:
                    opts = self._detail_options(dev, kind)
                    b.detail = opts[0][0] if opts else ""
                dev.bindings[name] = b
            self.dirty = True
        elif a == "pick":
            self._open_popup(action[2], action[3], action[4], action[5])
        elif a == "focus":
            self.editing = {"key": action[1], "text": str(action[2] or ""), "commit": action[4]}
        elif a == "macro_add":
            self._add_macro()
        elif a == "macro_del":
            if 0 <= action[1] < len(self.lib.actions.macros):
                name = self.lib.actions.macros[action[1]].get("name", "macro")
                del self.lib.actions.macros[action[1]]
                self.dirty = True
                self.say(f"Deleted macro “{name}”")
        elif a == "preset":
            prof = self.lib.profile()
            for d in prof.devices:
                d.effect.kind = action[1]
                d.effect.brightness = self.lib.settings.brightness
            self.dirty = True
            self.say("Preset applied to every device")
        elif a == "setting_toggle":
            key = action[1]
            setattr(self.lib.settings, key, not getattr(self.lib.settings, key))
            self.dirty = True
        elif a == "setting_polling":
            self.lib.settings.polling_rate_hz = action[1]
            self.dirty = True

    def _set_button_kind(self, name: str, kind: str) -> None:
        dev = self.device()
        if not dev:
            return
        if kind == "default":
            dev.bindings.pop(name, None)
        else:
            dev.bindings[name] = M.Binding(kind, "", "")
        self.dirty = True

    def _use_profile(self, value) -> None:
        try:
            idx = int(value)
        except (TypeError, ValueError):
            return
        if 0 <= idx < len(self.lib.profiles):
            self.lib.active_profile = idx
            self.dirty = True
            self.device_id = self._first_device_id()
            self.say(f"Switched to “{self.lib.profiles[idx].name}”")

    def _add_macro(self) -> None:
        name = self.macro_draft.get("name", "").strip()
        keys = self.macro_draft.get("keys", "").strip()
        if not name:
            self.say("Type a macro name first")
            return
        self.lib.actions.macros.append({"name": name, "keys": keys or "No keys set", "delay": 0})
        self.macro_draft = {"name": "", "keys": ""}
        self.dirty = True
        self.say(f"Added macro “{name}”")

    def _macro_field(self, which: str):
        def setter(value: str) -> None:
            self.macro_draft[which] = value
        return setter

    # =====================================================================
    # a run without a window: draw, save, repeat
    # =====================================================================
    def still(self, t: float) -> Image.Image:
        return self.draw(t)


def _inside(box, x, y) -> bool:
    return box[0] <= x <= box[2] and box[1] <= y <= box[3]


def _wants_key(fn) -> bool:
    try:
        import inspect
        sig = inspect.signature(fn)
        return len(sig.parameters) >= 2
    except Exception:
        return False


def _effect_label(kind: str) -> str:
    for k, label, _d in M.EFFECTS:
        if k == kind:
            return label
    return kind.capitalize()


def _mouse(prof: M.Profile) -> M.Device | None:
    for d in prof.devices:
        if d.kind == "mouse":
            return d
    return None


def _short_path(path: str) -> str:
    home = os.path.expanduser("~")
    if path.startswith(home):
        return "~" + path[len(home):]
    return path


def _dpi_text(prof: M.Profile) -> str:
    m = _mouse(prof)
    if not m or not m.dpi_stages:
        return "—"
    return f"{m.dpi_stages[m.active_stage]:,}"
