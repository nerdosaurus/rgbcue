"""Puts the interface on screen as an ordinary desktop window.

The app draws its own pixels, so the only thing needed from the desktop is a
window to show them in.  This module talks to the display server through
python-xlib (X11, including an XWayland session on Wayland) and turns mouse and
keyboard events into calls on the App.

If no display can be reached it says so plainly instead of failing silently.
"""

from __future__ import annotations

import os
import select
import sys
import time

from PIL import Image

from .ui import App


class NoDisplay(Exception):
    pass


class Window:
    def __init__(self, app: App, title: str = "RGBCUE - Corsair device control for Linux"):
        self.app = app
        self.title = title
        self.width = app.width
        self.height = app.height
        self.running = True
        self.closed_cleanly = False
        self.frames = 0
        self._last_draw = 0.0
        self.fps = 30.0
        self._show()

    # -- set-up ----------------------------------------------------------
    def _show(self) -> None:
        try:
            from Xlib import X, XK, display as xdisplay, error
            from Xlib.protocol import event as xevent
        except Exception as exc:  # pragma: no cover - reported to the person
            raise NoDisplay(f"python-xlib is not available ({exc})") from exc

        name = os.environ.get("DISPLAY")
        if not name:
            raise NoDisplay(
                "This computer gives the app no X11 display (DISPLAY is empty). "
                "On a Wayland desktop, an XWayland session normally provides one; "
                "outside a desktop session there is nowhere to draw."
            )
        try:
            self.d = xdisplay.Display(name)
            self.screen = self.d.screen()
        except Exception as exc:
            raise NoDisplay(f"could not open display {name!r}: {exc}") from exc

        self.X = X
        self.xevent = xevent
        self.XK = XK
        self.win = self.screen.root.create_window(
            60, 60, self.width, self.height, 0,
            self.screen.root_depth,
            X.InputOutput,
            X.CopyFromParent,
            background_pixel=self.screen.black_pixel,
            event_mask=(X.ExposureMask | X.StructureNotifyMask | X.KeyPressMask |
                        X.ButtonPressMask | X.ButtonReleaseMask | X.PointerMotionMask |
                        X.EnterWindowMask | X.LeaveWindowMask),
            colormap=X.CopyFromParent,
        )

        # make it look like a real application window
        self.win.set_wm_name(self.title)
        self.win.set_wm_icon_name("RGBCUE")
        try:
            self.win.set_wm_class("rgbcue", "RGBCUE")
        except Exception:
            pass
        try:
            atoms = {k: self.d.intern_atom(k) for k in
                     ("WM_PROTOCOLS", "WM_DELETE_WINDOW", "UTF8_STRING", "_NET_WM_NAME")}
            self.win.change_property(atoms["_NET_WM_NAME"], atoms["UTF8_STRING"], 8, self.title.encode())
            self.win.set_wm_protocols([atoms["WM_DELETE_WINDOW"]])
            self.wm_delete = atoms["WM_DELETE_WINDOW"]
        except Exception:
            self.wm_delete = None

        # a fixed size keeps the layout predictable
        try:
            from Xlib import Xutil
            hints = self.win.get_wm_normal_hints()
            hints.set_flags(Xutil.PMinSize | Xutil.PMaxSize)
            hints.min_width = hints.max_width = self.width
            hints.min_height = hints.max_height = self.height
            self.win.set_wm_normal_hints(hints)
        except Exception:
            pass

        self.gc = self.win.create_gc(foreground=self.screen.black_pixel,
                                     background=self.screen.black_pixel)
        self.win.map()
        self.d.flush()
        self._text_supported = True

    # -- drawing ---------------------------------------------------------
    def _blit(self, image: Image.Image) -> None:
        if image.size != (self.width, self.height):
            image = image.resize((self.width, self.height), Image.LANCZOS)
        # A depth-24 ZPixmap stores 32 bits per pixel on the wire, and a single
        # PutImage request is capped by a 16-bit length field, so a full frame
        # must be sent as strips.  The vendored put_pil_image already handles
        # both the BGRX pixel packing and the strip splitting correctly.
        self.win.put_pil_image(self.gc, 0, 0, image.convert("RGB"))
        self.d.flush()
        self.frames += 1

    def redraw(self, force: bool = True) -> None:
        now = time.monotonic()
        if not force and (now - self._last_draw) < (1.0 / self.fps):
            return
        self._last_draw = now
        # drawing at twice the size and halving it gives smooth edges; the
        # window path uses the same picture the screenshot does
        self._blit(self.app.draw(now, self.app.pointer))

    # -- events ----------------------------------------------------------
    def _key_name(self, event) -> str | None:
        try:
            keysym = self.d.keycode_to_keysym(event.detail, 0)
            name = self.XK.keysym_to_string(keysym)
            if name:
                return "space" if name == " " else name
            # names for the keys a text field or the keyboard needs
            special = {
                self.XK.string_to_keysym("Return"): "Return",
                self.XK.string_to_keysym("BackSpace"): "BackSpace",
                self.XK.string_to_keysym("Escape"): "Escape",
                self.XK.string_to_keysym("Tab"): "Tab",
                self.XK.string_to_keysym("space"): "space",
                self.XK.string_to_keysym("Delete"): "Delete",
            }
            return special.get(keysym)
        except Exception:
            return None

    def handle(self, event) -> None:
        X = self.X
        t = event.type
        if t == X.Expose:
            self.redraw(True)
        elif t == X.MotionNotify:
            self.app.on_move(float(event.root_x - self.win_origin[0]),
                             float(event.root_y - self.win_origin[1]))
            self.redraw(False)
        elif t == X.EnterNotify:
            pass
        elif t == X.LeaveNotify:
            self.app.pointer = None
            self.redraw(True)
        elif t == X.ButtonPress:
            if event.detail == 4 or event.detail == 5:
                self.app.on_scroll(float(event.event_x), float(event.event_y),
                                   -1 if event.detail == 4 else 1)
            else:
                self.app.on_press(float(event.event_x), float(event.event_y))
            self.redraw(True)
        elif t == X.ButtonRelease:
            self.app.on_release()
            self.redraw(True)
        elif t == X.KeyPress:
            name = self._key_name(event)
            if name:
                # one key press means one event: the text goes in on the same call
                self.app.on_key(name)
                self.redraw(True)
        elif t == X.ClientMessage:
            if self.wm_delete and event.data[1][0] == self.wm_delete:
                self.running = False
        elif t in (X.ConfigureNotify,):
            self.redraw(True)

    # -- main loop -------------------------------------------------------
    def run(self) -> int:
        self.win_origin = (0, 0)
        try:
            geo = self.win.get_geometry()
            self.win_origin = (geo.x, geo.y)
        except Exception:
            pass
        self.redraw(True)
        last_anim = time.monotonic()
        try:
            while self.running:
                # wait a frame for the desktop, then draw the animations
                pending = select.select([self.d.fileno()], [], [], 0.033)[0]
                while self.d.pending_events():
                    self.handle(self.d.next_event())
                now = time.monotonic()
                if now - last_anim >= 0.033:
                    last_anim = now
                    self.redraw(True)
            self.closed_cleanly = True
        except (KeyboardInterrupt, EOFError):
            self.closed_cleanly = True
        except Exception as exc:
            if getattr(exc, "__class__", type(exc)).__name__ in ("DisplayError", "ConnectionClosedError"):
                self.closed_cleanly = True
            else:
                raise
        finally:
            try:
                self.d.close()
            except Exception:
                pass
        return 0


def run_window(app: App, title: str | None = None) -> int:
    try:
        win = Window(app, title or "RGBCUE - Corsair device control for Linux")
    except NoDisplay as exc:
        print(f"RGBCUE could not open a window.\n{exc}\n"
              f"Nothing was changed. The app still works without a window:\n"
              f"  ./rgbcue --screenshot shot.png   writes a picture of the interface",
              file=sys.stderr)
        return 1
    code = win.run()
    # settings are kept even when the window is closed from the desktop
    try:
        from . import model as M
        if app.dirty:
            M.save(app.lib)
    except Exception as exc:
        print(f"could not save settings: {exc}", file=sys.stderr)
    return code
