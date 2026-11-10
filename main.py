"""RGBCUE — a native Linux companion for Corsair keyboards and mice.

Run it like an ordinary program:

    ./rgbcue                    open the window
    ./rgbcue --screenshot out.png   draw the interface and save a picture, then quit
    ./rgbcue --list-devices     show what Corsair hardware was found
    ./rgbcue --selftest         check that the interface can be drawn at all
"""

from __future__ import annotations

import argparse
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
if HERE not in sys.path:
    sys.path.insert(0, HERE)

from rgbcue import deps  # noqa: E402


def _parse() -> argparse.Namespace:
    p = argparse.ArgumentParser(
        prog="rgbcue",
        description="Configure Corsair keyboards and mice on Linux: lighting, key remaps, "
                    "DPI stages, macros and profiles.",
    )
    p.add_argument("--screenshot", metavar="FILE", help="save a picture of the interface to FILE and exit")
    p.add_argument("--frames", type=int, default=120, help="frames to draw before the picture (default 120)")
    p.add_argument("--width", type=int, default=None, help="window width in pixels")
    p.add_argument("--height", type=int, default=None, help="window height in pixels")
    p.add_argument("--page", default=None,
                   help="open on a page: dashboard, device, actions, profiles, settings")
    p.add_argument("--tab", default=None, help="open a device page on this tab: lighting, keys, "
                                               "performance, buttons, settings")
    p.add_argument("--device", default=None, help="device id or name fragment to open")
    p.add_argument("--set", action="append", default=[], metavar="KEY=VALUE",
                   help="change a setting before drawing, e.g. effect=rainbow or page=settings")
    p.add_argument("--clean", action="store_true", help="ignore the saved library; start from the examples")
    p.add_argument("--list-devices", action="store_true", help="print the Corsair devices found and exit")
    p.add_argument("--selftest", action="store_true", help="draw the interface once, report, and exit")
    p.add_argument("--print-size", action="store_true", help="print the default window size and exit")
    p.add_argument("--audit", action="store_true",
                   help="draw every page and report layout problems, then exit")
    return p.parse_args()


def _apply_named_changes(app, pairs: list[str]) -> None:
    """Small hook so the picture can be taken on any page without a mouse."""
    from rgbcue import model as M
    for pair in pairs:
        if "=" not in pair:
            continue
        key, value = pair.split("=", 1)
        key, value = key.strip().lower(), value.strip()
        if key == "page":
            app.goto(value)
        elif key == "profile":
            for i, p in enumerate(app.lib.profiles):
                if p.name.lower().startswith(value.lower()):
                    app.lib.active_profile = i
                    break
        elif key == "device":
            for prof in app.lib.profiles:
                for d in prof.devices:
                    if value.lower() in d.id.lower() or value.lower() in d.name.lower():
                        app.goto(f"device:{d.id}")
                        break
        elif key == "tab":
            dev = app.device()
            if dev:
                wanted = {"keys": "Key remaps", "keymap": "Key remaps", "lighting": "Lighting",
                          "performance": "Performance", "buttons": "Buttons",
                          "settings": "Device settings"}.get(value.lower(), value)
                app.tab[dev.id] = wanted
        elif key == "effect":
            for d in app.lib.profile().devices:
                d.effect.kind = value.lower()
        elif key == "theme":
            pass


def main(argv: list[str] | None = None) -> int:
    args = _parse()
    ok, _notes = deps.ensure(quiet=args.list_devices)
    if not ok:
        print("RGBCUE cannot start: it could not prepare its drawing library (Pillow).\n"
              "Install it with:  python3 -m pip install pillow", file=sys.stderr)
        return 2

    from rgbcue import model as M
    from rgbcue.ui import App, W, H
    App2 = App

    if args.list_devices:
        found = M.detect_hardware()
        if not found:
            print("No Corsair devices were found on this computer.")
            print("The app will show a demo keyboard and mouse you can configure instead.")
            return 0
        for d in found:
            print(f"{d.name}  ({d.kind}, connected over {d.transport})")
        return 0

    if args.print_size:
        print(f"{W}x{H}")
        return 0

    width = args.width or W
    height = args.height or H
    app = App(width=width, height=height, autoload=not args.clean)

    if args.device:
        for prof in app.lib.profiles:
            for d in prof.devices:
                if args.device.lower() in d.id.lower() or args.device.lower() in d.name.lower():
                    app.goto(f"device:{d.id}")
                    break
    if args.tab:
        dev = app.device()
        if dev:
            app.tab[dev.id] = {"keys": "Key remaps", "keymap": "Key remaps", "lighting": "Lighting",
                               "performance": "Performance", "buttons": "Buttons",
                               "settings": "Device settings"}.get(args.tab.lower(),
                                                                  args.tab.capitalize())
    if args.page:
        if args.page.lower() == "device" and not args.device:
            dev = app.device()
            if dev:
                app.goto(f"device:{dev.id}")
        else:
            app.goto(args.page.lower())
    if args.set:
        _apply_named_changes(app, args.set)

    if args.selftest:
        probe = app.draw(0.0, None)
        w, h = probe.size
        colours = probe.convert("RGB").getcolors(maxcolors=1 << 20) or []
        print(f"interface drawn at {w}x{h} with {len(colours)} distinct colours")
        return 0 if len(colours) > 50 else 1

    if args.audit:
        return _audit(app)

    if args.screenshot:
        return _screenshot(app, args.screenshot, args.frames)

    from rgbcue.window import run_window
    return run_window(app)


def _overlaps(app) -> list[str]:
    """Clickable areas that sit on top of each other: a sure sign of a layout clash."""
    out = []
    boxes = [(box, act) for box, act in app.regions if act]
    for i in range(len(boxes)):
        a_box, a_act = boxes[i]
        for j in range(i + 1, len(boxes)):
            b_box, b_act = boxes[j]
            ix = min(a_box[2], b_box[2]) - max(a_box[0], b_box[0])
            iy = min(a_box[3], b_box[3]) - max(a_box[1], b_box[1])
            if ix <= 1 or iy <= 1:
                continue
            inter = ix * iy
            small = min((a_box[2] - a_box[0]) * (a_box[3] - a_box[1]),
                        (b_box[2] - b_box[0]) * (b_box[3] - b_box[1]))
            if small > 0 and inter / small > 0.35:
                out.append(f"{a_act} and {b_act} overlap "
                           f"({100 * inter / small:.0f}% of the smaller)")
                if len(out) > 12:
                    return out
    return out


def _audit(app) -> int:
    """Draw every page and report layout problems: text cut off, text off-screen,
    controls outside the window.  Print-only, so it can run anywhere."""
    tabs_for = app.tabs_for
    pages: list[tuple[str, str | None]] = [("dashboard", None), ("actions", None),
                                           ("profiles", None), ("settings", None)]
    problems = 0
    for name, _ in pages:
        app.goto(name)
        app.render(0.4, None)
        warns, trunc = app.warnings, app.truncated
        clashes = _overlaps(app)
        status = "ok" if not (warns or trunc or clashes) else \
            f"{len(warns)} off-screen, {len(trunc)} cut short, {len(clashes)} overlapping"
        print(f"{name:<12} {status}")
        problems += len(warns) + len(clashes)
        for w in warns[:6]:
            print(f"   ! {w}")
        for t in trunc[:6]:
            print(f"   ~ shortened {t}")
        for o in clashes[:6]:
            print(f"   x {o}")
    for prof in app.lib.profiles:
        for dev in prof.devices:
            app.goto(f"device:{dev.id}")
            for tab in tabs_for(dev):
                app.tab[dev.id] = tab
                app.render(0.4, None)
                warns, trunc = app.warnings[:], app.truncated[:]
                clashes = _overlaps(app)
                status = "ok" if not (warns or trunc or clashes) else \
                    f"{len(warns)} off-screen, {len(trunc)} cut short, {len(clashes)} overlapping"
                print(f"{dev.kind}:{tab:<18} {status}")
                problems += len(warns) + len(clashes)
                for w in warns[:6]:
                    print(f"   ! {w}")
                for t in trunc[:6]:
                    print(f"   ~ shortened {t}")
                for o in clashes[:6]:
                    print(f"   x {o}")
    print(f"\n{problems} layout problem(s)")
    return 0 if problems == 0 else 1


def _screenshot(app, path: str, frames: int) -> int:
    """Draw the interface a number of frames, save the last one, and exit."""
    target = path
    if not os.path.isabs(target):
        target = os.path.join(os.getcwd(), target)
    folder = os.path.dirname(target)
    if folder:
        os.makedirs(folder, exist_ok=True)

    image = None
    # A few frames at rest, then a handful spread over time so animated lighting
    # has moved on, exactly as it would in an open window.  Only the last frame
    # is turned into the final picture, so drawing stays quick.
    import time as _time
    start = _time.monotonic()
    total = max(1, frames)
    for i in range(total - 1):
        app.render(i * 0.033, None)
    image = app.draw(max(0.0, (total - 1) * 0.033), None)
    image.save(target, "PNG")
    elapsed = _time.monotonic() - start
    print(f"saved {target} ({image.size[0]}x{image.size[1]}, {frames} frames, {elapsed:.2f}s)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
