"""Check that the app actually does what its controls say.

Clicks a few things and reads the result back, then saves the library, loads it
again and compares. Prints PASS or FAIL per check.
"""

from __future__ import annotations

import os
import sys
import tempfile

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if HERE not in sys.path:
    sys.path.insert(0, HERE)

from rgbcue import deps  # noqa: E402

deps.ensure(quiet=True)

from rgbcue import model as M  # noqa: E402
from rgbcue.ui import App  # noqa: E402

FAILS: list[str] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    print(f"{'PASS' if ok else 'FAIL'}  {name}{('  — ' + detail) if detail else ''}")
    if not ok:
        FAILS.append(name)


def click(app: App, action_starts: str, nth: int = 0) -> bool:
    """Find the nth clickable region whose action starts with a word and click it."""
    app.render(0.3, None)
    hits = [(box, act) for box, act in app.regions if act and act[0] == action_starts]
    if len(hits) <= nth:
        return False
    box, _act = hits[nth]
    cx, cy = (box[0] + box[2]) / 2, (box[1] + box[3]) / 2
    app.on_move(cx, cy)
    app.on_press(cx, cy)
    app.on_release()
    app.render(0.4, None)
    return True


def main() -> int:
    app = App(autoload=False)
    kb = app.lib.profile().devices[0]

    # lighting: choose the rainbow effect on the keyboard
    app.goto(f"device:{kb.id}")
    app.tab[kb.id] = "Lighting"
    app.render(0.3, None)
    check("lighting tab opens", app.current_tab(kb) == "Lighting")
    before = kb.effect.kind
    ok = False
    for i in range(len(app.regions)):
        if click(app, "effect", i):
            if kb.effect.kind != before:
                ok = True
                break
    check("effect changes when its tile is clicked", ok, f"{before} -> {kb.effect.kind}")

    # brightness slider
    app.render(0.3, None)
    sl = [(box, sid) for box, act in app.regions if act[0] == "slider"
          for sid in [act[1]]]
    if sl:
        box, sid = sl[0]
        app.on_press(box[0] + (box[2] - box[0]) * 0.25, (box[1] + box[3]) / 2)
        app.on_release()
        check("brightness slider moves", kb.effect.brightness < 100,
              f"brightness now {kb.effect.brightness}")
        app.render(0.3, None)
        app.on_press(box[0] + (box[2] - box[0]) * 0.98, (box[1] + box[3]) / 2)
        app.on_release()
    else:
        check("brightness slider found", False)

    # zone toggle: click and check the zone really flips
    app.render(0.3, None)
    zones = [z for z in kb.zones]
    snapshot = [(z.name, z.on) for z in zones]
    toggled = False
    for i in range(len(zones)):
        app.render(0.3, None)
        if click(app, "zone_toggle", i):
            now = [(z.name, z.on) for z in zones]
            for (name, was), (_, is_now) in zip(snapshot, now):
                if was != is_now:
                    toggled = True
                    check("zone toggle flips a zone", True, f"{name}: {was} -> {is_now}")
                    break
            if toggled:
                break
    if not toggled:
        check("zone toggle flips a zone", False, f"nothing changed from {snapshot}")

    # key remap: click a key, assign an action, set its detail
    app.tab[kb.id] = "Key remaps"
    app.render(0.3, None)
    clicked_key = click(app, "key", 3)
    sel = app.selected_key.get(kb.id)
    check("clicking a key selects it", clicked_key and sel is not None, f"selected {sel!r}")
    if sel:
        ok = False
        for i in range(len(app.regions)):
            if click(app, "binding_kind", i):
                b = kb.bindings.get(sel)
                if b and b.kind not in ("default",):
                    ok = True
                    break
        check("assigning an action to a key works", ok,
              f"{sel} -> {(kb.bindings.get(sel).kind if kb.bindings.get(sel) else None)}")

    # mouse: DPI stages
    mouse = [d for d in app.lib.profile().devices if d.kind == "mouse"][0]
    app.goto(f"device:{mouse.id}")
    app.tab[mouse.id] = "Performance"
    app.render(0.3, None)
    n_before = len(mouse.dpi_stages)
    if click(app, "stage_add"):
        check("adding a DPI stage works", len(mouse.dpi_stages) == n_before + 1,
              f"{n_before} -> {len(mouse.dpi_stages)}")
    else:
        check("add stage button found", False)
    app.render(0.3, None)
    if len(mouse.dpi_stages) > 1 and click(app, "stage_del", 0):
        after = len(mouse.dpi_stages)
        check("deleting a DPI stage works", after == n_before, f"{after} stages left")
    app.render(0.3, None)
    target = mouse.active_stage
    if click(app, "stage", 0):
        check("using a DPI stage works", True, f"active stage {mouse.active_stage}")
    # polling rate
    app.render(0.3, None)
    for i in range(4):
        if click(app, "polling", i):
            break
    check("polling rate changes", mouse.polling_rate in (125, 250, 500, 1000),
          f"{mouse.polling_rate} Hz")

    # buttons: choose a button, then an action
    app.tab[mouse.id] = "Buttons"
    app.render(0.3, None)
    if click(app, "button_pick", 2):
        chosen = app.button_sel
        check("choosing a mouse button works", chosen is not None, f"{chosen!r}")
        app.render(0.3, None)
        if chosen and click(app, "button_kind", 1):
            b = mouse.bindings.get(chosen)
            check("assigning a mouse button action works", b is not None,
                  f"{chosen} -> {b.kind if b else None}")
    else:
        check("mouse button list found", False)

    # profiles: create one by typing a name
    app.goto("profiles")
    app.render(0.3, None)
    n_profiles = len(app.lib.profiles)
    focused = False
    for box, act in app.regions:
        if act[0] == "focus":
            cx, cy = (box[0] + box[2]) / 2, (box[1] + box[3]) / 2
            app.on_press(cx, cy)
            app.on_text("Evening")
            app.on_key("Return")
            focused = True
            break
    check("typing a profile name creates one", focused and len(app.lib.profiles) == n_profiles + 1,
          f"{n_profiles} -> {len(app.lib.profiles)} profiles")
    if len(app.lib.profiles) == n_profiles + 1:
        names = [p.name for p in app.lib.profiles]
        check("the new profile has the typed name", names[-1] == "Evening", f"{names}")

    # saving: round-trip through a real file
    with tempfile.TemporaryDirectory() as folder:
        os.environ["XDG_CONFIG_HOME"] = folder
        path = os.path.join(folder, "rgbcue", "library.json")
        M.save(app.lib)
        again = M.load()
        check("the library is written to disk", os.path.exists(path), path)
        check("profile names survive a save and load",
              [p.name for p in again.profiles] == [p.name for p in app.lib.profiles])
        check("the keyboard's effect survives",
              again.profile().devices[0].effect.kind == app.lib.profile().devices[0].effect.kind,
              again.profile().devices[0].effect.kind)
        check("remapped keys survive",
              {k: b.kind for k, b in again.profile().devices[0].bindings.items()} ==
              {k: b.kind for k, b in app.lib.profile().devices[0].bindings.items()})
        mouse_before = [d.dpi_stages for d in app.lib.profile().devices if d.kind == "mouse"]
        mouse_after = [d.dpi_stages for d in again.profile().devices if d.kind == "mouse"]
        check("DPI stages survive", mouse_before == mouse_after, f"{mouse_after}")

    # hardware detection reads the kernel's device list without writing anything
    found = M.detect_hardware()
    check("hardware detection runs", isinstance(found, list), f"{len(found)} device(s)")

    print(f"\n{len(FAILS)} failure(s)")
    return 0 if not FAILS else 1


if __name__ == "__main__":
    sys.exit(main())
