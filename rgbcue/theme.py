"""Colours, fonts and the small drawing toolkit the interface is built from.

Everything on screen is drawn by hand into a pixel buffer: rectangles, rounded
panels, gradients, text and simple icons.  The toolkit below keeps that readable,
and it also records where the clickable things ended up so the window's mouse
handling can find them.
"""

from __future__ import annotations

import os
from PIL import Image, ImageDraw, ImageFont

# ---- palette ---------------------------------------------------------------
BG = (14, 16, 19)
SIDEBAR = (20, 22, 26)
PANEL = (26, 29, 35)
PANEL_HI = (34, 38, 46)
PANEL_LO = (22, 25, 30)
BORDER = (43, 48, 58)
BORDER_HI = (58, 65, 78)
TEXT = (242, 244, 247)
TEXT_DIM = (154, 163, 175)
TEXT_FAINT = (108, 117, 130)
ACCENT = (245, 197, 66)
ACCENT_DIM = (120, 96, 28)
ACCENT_TEXT = (24, 20, 6)
GREEN = (70, 209, 122)
BLUE = (90, 160, 255)
RED = (238, 96, 96)
AMBER = (240, 166, 90)
PURPLE = (168, 130, 255)

ZONE_COLORS = [
    ("Red", (235, 64, 64)),
    ("Orange", (245, 145, 55)),
    ("Yellow", (245, 208, 70)),
    ("Green", (80, 210, 120)),
    ("Cyan", (70, 200, 215)),
    ("Blue", (80, 140, 245)),
    ("Purple", (160, 120, 240)),
    ("Pink", (235, 110, 180)),
    ("White", (238, 240, 245)),
]

_FONT_DIRS = [
    "/usr/share/fonts/truetype/dejavu",
    "/usr/share/fonts/dejavu",
    "/usr/share/fonts/truetype/liberation",
    "/usr/share/fonts/liberation",
    "/usr/share/fonts/truetype/noto",
    "/System/Library/Fonts/Supplemental",
    "C:/Windows/Fonts",
]
_REGULAR = ["DejaVuSans.ttf", "LiberationSans-Regular.ttf", "NotoSans-Regular.ttf", "Arial.ttf", "FreeSans.ttf"]
_BOLD = ["DejaVuSans-Bold.ttf", "LiberationSans-Bold.ttf", "NotoSans-Bold.ttf", "Arial Bold.ttf", "FreeSansBold.ttf"]

_cache: dict[tuple[str, int, bool], ImageFont.FreeTypeFont] = {}


def _find(candidates: list[str]) -> str | None:
    for folder in _FONT_DIRS:
        for name in candidates:
            path = os.path.join(folder, name)
            if os.path.exists(path):
                return path
    # last resort: any truetype font the system has
    for folder in ("/usr/share/fonts", "/usr/local/share/fonts"):
        for root, _dirs, files in os.walk(folder):
            for name in files:
                if name.lower().endswith(".ttf"):
                    return os.path.join(root, name)
    return None


_FACE_PATHS: dict[bool, str | None] = {}


def font(size: int, bold: bool = False) -> ImageFont.FreeTypeFont:
    key = ("f", size, bold)
    if key in _cache:
        return _cache[key]
    if bold not in _FACE_PATHS:
        _FACE_PATHS[bold] = _find(_BOLD if bold else _REGULAR)
    path = _FACE_PATHS[bold]
    if path and os.path.exists(path):
        f = ImageFont.truetype(path, size)
    else:
        f = ImageFont.load_default(size)
    _cache[key] = f
    return f


def mix(a: tuple[int, int, int], b: tuple[int, int, int], t: float) -> tuple[int, int, int]:
    t = max(0.0, min(1.0, t))
    return (
        int(a[0] + (b[0] - a[0]) * t),
        int(a[1] + (b[1] - a[1]) * t),
        int(a[2] + (b[2] - a[2]) * t),
    )


def scale_color(c: tuple[int, int, int], f: float) -> tuple[int, int, int]:
    return (max(0, min(255, int(c[0] * f))), max(0, min(255, int(c[1] * f))), max(0, min(255, int(c[2] * f))))


def hsv(h: float, s: float, v: float) -> tuple[int, int, int]:
    h = h % 360.0
    i = int(h // 60) % 6
    f = h / 60.0 - int(h // 60)
    p, q, t = v * (1 - s), v * (1 - s * f), v * (1 - s * (1 - f))
    r, g, b = [(v, t, p), (q, v, p), (p, v, t), (p, q, v), (t, p, v), (v, p, q)][i]
    return (int(r * 255), int(g * 255), int(b * 255))


class Canvas:
    """Draws in logical pixels over a buffer rendered at ``scale`` times the size."""

    def __init__(self, image: Image.Image, scale: int = 2, pointer: tuple[float, float] | None = None):
        self.img = image
        self.d = ImageDraw.Draw(image, "RGBA")
        self.scale = scale
        self.pointer = pointer
        self.regions: list[tuple[tuple[float, float, float, float], tuple]] = []
        self.sliders: dict[str, tuple[tuple[float, float, float, float], float, float, object, str]] = {}
        # things a layout check looks at: text that had to be shortened, and
        # anything drawn outside the window
        self.warnings: list[str] = []
        self.truncated: list[str] = []
        self.bounds = (0.0, 0.0, image.size[0] / max(1, scale), image.size[1] / max(1, scale))

    # -- geometry helpers ---------------------------------------------------
    def _b(self, box):
        s = self.scale
        x0, y0, x1, y1 = box
        return [round(x0 * s), round(y0 * s), round(x1 * s), round(y1 * s)]

    def contains(self, box) -> bool:
        if not self.pointer:
            return False
        x0, y0, x1, y1 = box
        return x0 <= self.pointer[0] <= x1 and y0 <= self.pointer[1] <= y1

    def register(self, box, action) -> None:
        if action is not None:
            self.regions.append((box, action))

    # -- shapes -------------------------------------------------------------
    def rect(self, box, fill=None, radius: float = 0, outline=None, width: float = 1) -> None:
        b = self._b(box)
        r = radius * self.scale
        if len(b) != 4 or b[2] <= b[0] or b[3] <= b[1]:
            return
        if r > 0:
            self.d.rounded_rectangle(b, radius=r, fill=fill, outline=outline,
                                     width=max(1, int(width * self.scale)) if outline else 0)
        else:
            self.d.rectangle(b, fill=fill, outline=outline,
                             width=max(1, int(width * self.scale)) if outline else 0)

    def circle(self, center, radius: float, fill=None, outline=None, width: float = 1) -> None:
        s = self.scale
        cx, cy = center[0] * s, center[1] * s
        r = radius * s
        self.d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=fill, outline=outline,
                       width=max(1, int(width * s)) if outline else 0)

    def line(self, points, fill, width: float = 1) -> None:
        s = self.scale
        pts = [(p[0] * s, p[1] * s) for p in points]
        self.d.line(pts, fill=fill, width=max(1, int(width * s)), joint="curve")

    def polygon(self, points, fill) -> None:
        s = self.scale
        self.d.polygon([(p[0] * s, p[1] * s) for p in points], fill=fill)

    def gradient(self, box, top, bottom, vertical: bool = True, radius: float = 0) -> None:
        """A soft two-colour gradient. Rounded by masking with a rounded square."""
        b = self._b(box)
        w, h = b[2] - b[0], b[3] - b[1]
        if w <= 0 or h <= 0:
            return
        steps = max(2, h if vertical else w)
        strip = Image.new("RGBA", (max(1, w), max(1, h)))
        sd = ImageDraw.Draw(strip)
        for i in range(steps):
            c = mix(top, bottom, i / max(1, steps - 1))
            if vertical:
                sd.line([(0, i), (w, i)], fill=c)
            else:
                sd.line([(i, 0), (i, h)], fill=c)
        if radius > 0:
            mask = Image.new("L", (w, h), 0)
            ImageDraw.Draw(mask).rounded_rectangle([0, 0, w - 1, h - 1], radius=radius * self.scale, fill=255)
            strip.putalpha(mask)
        self.img.alpha_composite(strip, dest=(b[0], b[1])) if self.img.mode == "RGBA" else self.img.paste(strip, (b[0], b[1]))

    # -- text ---------------------------------------------------------------
    def text(self, xy, s: str, size: int = 13, bold: bool = False, color=TEXT, anchor: str = "la",
             alpha: int = 255) -> tuple[float, float]:
        s = str(s)
        f = font(int(size * self.scale), bold)
        pos = (xy[0] * self.scale, xy[1] * self.scale)
        fill = (color[0], color[1], color[2], alpha) if len(color) == 3 else color
        self.d.text(pos, s, font=f, fill=fill, anchor=anchor)
        # note anything drawn outside the window, so layout mistakes are visible
        bx0, by0, bx1, by1 = self.bounds
        w = f.getlength(s) / self.scale
        h = size * 1.25
        ax = anchor[0] if anchor else "l"
        ay = anchor[1] if len(anchor) > 1 else "a"
        left = xy[0] if ax == "l" else (xy[0] - w / 2 if ax == "m" else xy[0] - w)
        right = left + w
        top = xy[1] if ay in ("a", "t") else (xy[1] - h / 2 if ay in ("m", "s") else xy[1] - h)
        bottom = top + h
        if s.strip() and (left < bx0 - 2 or right > bx1 + 2 or top < by0 - 6 or bottom > by1 + 6):
            if len(self.warnings) < 60:
                self.warnings.append(f"text outside window: {s[:40]!r} spans "
                                     f"{left:.0f}..{right:.0f} x, {top:.0f}..{bottom:.0f} y")
        return self.measure(s, size, bold)

    def measure(self, s: str, size: int = 13, bold: bool = False) -> tuple[float, float]:
        f = font(int(size * self.scale), bold)
        return f.getlength(str(s)) / self.scale, size * 1.25

    def text_in(self, box, s: str, size: int = 13, bold: bool = False, color=TEXT,
                anchor: str = "la", pad: float = 0, ellipsis: bool = True) -> None:
        """Text fitted inside a box: cut with an ellipsis when too long.

        A two-value box (just a position) means "no width limit".
        """
        if len(box) == 2:
            x0, y0 = box
            x1 = x0 + 10000
        else:
            x0, y0, x1, _y1 = box
        inner = (x0 + pad, x1 - pad)
        if not ellipsis:
            self.text((inner[0], y0), s, size, bold, color, anchor)
            return
        available = inner[1] - inner[0]
        if available <= 4:
            return
        if self.measure(s, size, bold)[0] <= available:
            self.text((inner[0], y0), s, size, bold, color, anchor)
            return
        cut = str(s)
        while cut and self.measure(cut + "…", size, bold)[0] > available:
            cut = cut[:-1]
        if cut != str(s) and len(self.truncated) < 80:
            self.truncated.append(f"{str(s)[:50]!r} -> {cut[:40]!r} (room {available:.0f}px)")
        self.text((inner[0], y0), cut + "…", size, bold, color, anchor)

    def wrap(self, box, s: str, size: int = 12, color=TEXT_DIM, bold: bool = False,
             line_height: float | None = None) -> float:
        """Word-wrapped text; returns the height used."""
        x0, y0, x1, _y1 = box
        width = x1 - x0
        lh = line_height or size * 1.5
        words = str(s).split()
        lines, cur = [], ""
        for word in words:
            trial = (cur + " " + word).strip()
            if self.measure(trial, size, bold)[0] <= width or not cur:
                cur = trial
            else:
                lines.append(cur)
                cur = word
        if cur:
            lines.append(cur)
        y = y0
        for line in lines:
            self.text((x0, y), line, size, bold, color)
            y += lh
        return y - y0

    # -- composite widgets --------------------------------------------------
    def button(self, box, label: str, action, kind: str = "ghost", size: int = 13,
               icon: str | None = None, enabled: bool = True) -> None:
        hover = enabled and self.contains(box)
        if kind == "primary":
            fill = mix(ACCENT, (255, 232, 150), 0.35) if hover else ACCENT
            fg = ACCENT_TEXT
            outline = None
        elif kind == "secondary":
            fill = PANEL_HI if not hover else (PANEL_HI[0] + 10, PANEL_HI[1] + 10, PANEL_HI[2] + 12)
            fg = TEXT
            outline = BORDER_HI
        elif kind == "danger":
            fill = (58, 30, 30) if not hover else (76, 38, 38)
            fg = RED
            outline = (92, 46, 46)
        else:
            fill = PANEL_HI if hover else None
            fg = TEXT_DIM if not enabled else TEXT
            outline = BORDER if hover else None
        if not enabled:
            fg = TEXT_FAINT
        self.rect(box, fill=fill, radius=8, outline=outline)
        cx = (box[0] + box[2]) / 2
        cy = (box[1] + box[3]) / 2
        tw = self.measure(label, size, True)[0] if label else 0
        icon_w = 18 if icon else 0
        start = cx - (tw + icon_w) / 2
        if icon:
            self.icon(icon, (start + 7, cy), size - 2, fg)
        if label:
            self.text((start + icon_w, cy), label, size, True, fg, anchor="lm")
        self.register(box, action if enabled else None)

    def icon(self, name: str, center, size: float, color=TEXT, width: float = 1.6) -> None:
        """A tiny set of line icons, drawn by hand."""
        cx, cy = center
        h = size / 2
        c = (255,) if False else color
        if name == "gear":
            self.circle((cx, cy), h * 0.62, outline=c, width=width)
            self.circle((cx, cy), h * 0.2, outline=c, width=width)
            for k in range(8):
                import math
                a = k * math.pi / 4
                self.line([(cx + math.cos(a) * h * 0.62, cy + math.sin(a) * h * 0.62),
                           (cx + math.cos(a) * h * 0.98, cy + math.sin(a) * h * 0.98)], c, width)
        elif name == "chevron-down":
            self.line([(cx - h * 0.55, cy - h * 0.22), (cx, cy + h * 0.3), (cx + h * 0.55, cy - h * 0.22)], c, width)
        elif name == "chevron-right":
            self.line([(cx - h * 0.2, cy - h * 0.5), (cx + h * 0.3, cy), (cx - h * 0.2, cy + h * 0.5)], c, width)
        elif name == "plus":
            self.line([(cx - h * 0.6, cy), (cx + h * 0.6, cy)], c, width)
            self.line([(cx, cy - h * 0.6), (cx, cy + h * 0.6)], c, width)
        elif name == "trash":
            self.rect((cx - h * 0.5, cy - h * 0.35, cx + h * 0.5, cy + h * 0.7), radius=1.5, outline=c, width=width)
            self.line([(cx - h * 0.7, cy - h * 0.55), (cx + h * 0.7, cy - h * 0.55)], c, width)
            self.line([(cx - h * 0.2, cy - h * 0.8), (cx + h * 0.2, cy - h * 0.8)], c, width)
        elif name == "check":
            self.line([(cx - h * 0.5, cy + h * 0.02), (cx - h * 0.1, cy + h * 0.42), (cx + h * 0.55, cy - h * 0.42)], c, width)
        elif name == "close":
            self.line([(cx - h * 0.45, cy - h * 0.45), (cx + h * 0.45, cy + h * 0.45)], c, width)
            self.line([(cx + h * 0.45, cy - h * 0.45), (cx - h * 0.45, cy + h * 0.45)], c, width)
        elif name == "keyboard":
            self.rect((cx - h, cy - h * 0.55, cx + h, cy + h * 0.55), radius=2, outline=c, width=width)
            for i in range(4):
                self.circle((cx - h * 0.55 + i * h * 0.36, cy - h * 0.12), 0.9, fill=c)
            self.line([(cx - h * 0.5, cy + h * 0.22), (cx + h * 0.2, cy + h * 0.22)], c, width)
        elif name == "mouse":
            self.rect((cx - h * 0.5, cy - h * 0.85, cx + h * 0.5, cy + h * 0.85), radius=h * 0.5, outline=c, width=width)
            self.line([(cx, cy - h * 0.85), (cx, cy - h * 0.35)], c, width)
        elif name == "bolt":
            self.polygon([(cx - h * 0.15, cy - h), (cx + h * 0.55, cy - h * 0.15),
                          (cx + h * 0.05, cy - h * 0.05), (cx + h * 0.3, cy + h),
                          (cx - h * 0.55, cy + h * 0.05), (cx - h * 0.02, cy - h * 0.1)], c)
        elif name == "palette":
            self.circle((cx, cy), h * 0.9, outline=c, width=width)
            for dx, dy in ((-0.25, -0.35), (0.25, -0.3), (0.35, 0.2), (-0.05, 0.42)):
                self.circle((cx + dx * h * 1.4, cy + dy * h * 1.4), 1.6, fill=c)
        elif name == "grid":
            for gx in (0, 1):
                for gy in (0, 1):
                    self.rect((cx - h * 0.8 + gx * h * 0.95, cy - h * 0.8 + gy * h * 0.95,
                               cx - h * 0.15 + gx * h * 0.95, cy - h * 0.15 + gy * h * 0.95),
                              radius=1.5, outline=c, width=width)
        elif name == "sliders":
            for i, off in enumerate((-0.55, 0.0, 0.55)):
                self.line([(cx - h, cy + off * h), (cx + h, cy + off * h)], c, width * 0.85)
                self.circle((cx + (0.3 if i % 2 == 0 else -0.3) * h, cy + off * h), 2.2, fill=c)
        elif name == "save":
            self.rect((cx - h * 0.8, cy - h * 0.8, cx + h * 0.8, cy + h * 0.8), radius=2, outline=c, width=width)
            self.rect((cx - h * 0.4, cy - h * 0.8, cx + h * 0.4, cy - h * 0.2), radius=1, outline=c, width=width * 0.8)
        elif name == "info":
            self.circle((cx, cy), h * 0.9, outline=c, width=width)
            self.circle((cx, cy - h * 0.42), 1.4, fill=c)
            self.line([(cx, cy - h * 0.1), (cx, cy + h * 0.45)], c, width)

    def toggle(self, box, on: bool, action, enabled: bool = True) -> None:
        hover = enabled and self.contains(box)
        track = ACCENT if on else (60, 66, 78)
        if hover:
            track = mix(track, (255, 255, 255), 0.12)
        h = box[3] - box[1]
        self.rect(box, fill=track, radius=h / 2)
        knob_r = h / 2 - 3
        kx = box[2] - h / 2 if on else box[0] + h / 2
        self.circle((kx, (box[1] + box[3]) / 2), knob_r, fill=(250, 250, 252) if on else (190, 197, 208))
        self.register(box, action if enabled else None)

    def slider(self, sid: str, box, value: float, lo: float, hi: float, cb, fmt=None,
               accent=ACCENT, enabled: bool = True) -> None:
        """A horizontal slider. ``cb(new_value)`` is called while dragging."""
        x0, y0, x1, y1 = box
        cy = (y0 + y1) / 2
        track_h = 4
        frac = 0.0 if hi == lo else max(0.0, min(1.0, (value - lo) / (hi - lo)))
        self.rect((x0, cy - track_h / 2, x1, cy + track_h / 2), fill=(52, 58, 68), radius=track_h / 2)
        self.rect((x0, cy - track_h / 2, x0 + max(track_h, (x1 - x0) * frac), cy + track_h / 2),
                  fill=accent if enabled else (90, 96, 108), radius=track_h / 2)
        knob_x = x0 + (x1 - x0) * frac
        hover = self.contains((x0 - 8, y0 - 6, x1 + 8, y1 + 6))
        self.circle((knob_x, cy), 7.5 if hover else 7, fill=(252, 252, 254),
                    outline=accent if enabled else (120, 126, 138), width=1.5)
        if enabled:
            self.sliders[sid] = (box, lo, hi, cb, fmt or "")
            self.register((x0 - 8, y0 - 8, x1 + 8, y1 + 8), ("slider", sid))

    def swatch(self, box, color, action=None, selected: bool = False) -> None:
        hover = self.contains(box)
        r = (box[2] - box[0]) / 2
        if selected:
            self.circle(((box[0] + box[2]) / 2, (box[1] + box[3]) / 2), r + 2.5, outline=ACCENT, width=2)
        self.circle(((box[0] + box[2]) / 2, (box[1] + box[3]) / 2), r - (0.5 if hover else 1.0),
                    fill=color, outline=(255, 255, 255, 60))
        self.register(box, action)

    def pill(self, text: str, xy, fg=TEXT_DIM, bg=None, size: int = 11, pad: float = 8) -> tuple[float, float]:
        w = self.measure(text, size, True)[0]
        h = size + 9
        box = (xy[0], xy[1], xy[0] + w + pad * 2, xy[1] + h)
        if bg:
            self.rect(box, fill=bg, radius=h / 2)
        self.text((xy[0] + pad, (box[1] + box[3]) / 2), text, size, True, fg, anchor="lm")
        return box[2] - xy[0], h

    def card(self, box, fill=PANEL, radius: float = 12, outline=BORDER) -> None:
        self.rect(box, fill=fill, radius=radius, outline=outline)
