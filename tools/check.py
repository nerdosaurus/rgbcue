"""Drive the interface without a mouse: click every control on every page.

This is how the app checks itself.  It draws a frame, finds every clickable
region, clicks it, and reports any that raise an error, so a broken button is
found before a person finds it.
"""

from __future__ import annotations

import os
import sys
import traceback

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if HERE not in sys.path:
    sys.path.insert(0, HERE)

from rgbcue import deps  # noqa: E402

deps.ensure(quiet=True)

from rgbcue.ui import App  # noqa: E402


def frames_for(app: App) -> list[tuple[str, list]]:
    """Every page, with its clickable regions."""
    out = []
    app.goto("dashboard")
    app.render(0.3, None)
    out.append(("dashboard", list(app.regions)))
    for page in ("actions", "profiles", "settings"):
        app.goto(page)
        app.render(0.3, None)
        out.append((page, list(app.regions)))
    for prof in app.lib.profiles:
        for dev in prof.devices:
            app.goto(f"device:{dev.id}")
            for tab in App.tabs_for(dev):
                app.tab[dev.id] = tab
                app.render(0.3, None)
                out.append((f"{dev.id}/{tab}", list(app.regions)))
    return out


def main() -> int:
    failures = 0
    # every run starts from the examples, so nothing carries over
    app = App(autoload=False)
    for name, regions in frames_for(app):
        # a fresh app per page keeps one click from changing another page's layout
        for box, action in regions:
            try:
                fresh = App(autoload=False)
                # reach the same page again
                if "/" in name:
                    dev_id, tab = name.split("/", 1)
                    for prof in fresh.lib.profiles:
                        for d in prof.devices:
                            if d.id == dev_id:
                                fresh.tab[dev_id] = tab
                    fresh.goto(f"device:{dev_id}")
                else:
                    fresh.goto(name)
                fresh.render(0.3, None)
                cx = (box[0] + box[2]) / 2
                cy = (box[1] + box[3]) / 2
                fresh.on_move(cx, cy)
                fresh.on_press(cx, cy)
                fresh.on_release()
                fresh.render(0.4, (cx, cy))
            except Exception:
                failures += 1
                print(f"FAIL {name} {action} at {box}")
                traceback.print_exc(limit=4)
                if failures > 12:
                    print("... stopping after 12 failures")
                    return 1
        print(f"{name}: {len(regions)} controls clicked")

    # typing: focus each text box, type, press Return
    typed = 0
    app = App(autoload=False)
    for name, regions in frames_for(app):
        for box, action in regions:
            if action[0] != "focus":
                continue
            try:
                typed += 1
                cx = (box[0] + box[2]) / 2
                cy = (box[1] + box[3]) / 2
                app.on_press(cx, cy)
                app.on_text("Test name")
                app.on_key("BackSpace")
                app.on_key("Return")
                app.render(0.4, None)
            except Exception:
                failures += 1
                print(f"FAIL typing in {name} {action}")
                traceback.print_exc(limit=4)
        print(f"{name}: typed into boxes")

    # keys and popups
    app = App(autoload=False)
    for name, regions in frames_for(app):
        for box, action in regions:
            if action[0] not in ("kind_popup", "macro_popup", "detail_popup", "button_pick",
                                 "colour_popup"):
                continue
            try:
                cx = (box[0] + box[2]) / 2
                cy = (box[1] + box[3]) / 2
                app.on_press(cx, cy)
                app.render(0.4, None)
                # choose an option from the popup that opened
                if app.popup:
                    opt = app.popup["options"][min(1, len(app.popup["options"]) - 1)]
                    opts = app.popup["options"]
                    idx = opts.index(opt)
                    visible = opts[:11]
                    row = 78 + visible.index(opt) * 30
                    px = (app.width / 2 - 130) + 40
                    py = app.height / 2 - 90 + row
                    app.on_press(px, py)
                    app.render(0.4, None)
                app.on_key("Escape")
            except Exception:
                failures += 1
                print(f"FAIL popup in {name} {action}")
                traceback.print_exc(limit=4)
    print(f"{typed} text boxes exercised")

    print(f"\n{failures} failure(s)")
    return 0 if failures == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
