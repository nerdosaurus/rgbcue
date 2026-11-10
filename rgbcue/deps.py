"""Setting up the two helper libraries this app needs, without asking the person to.

The app draws its whole interface itself into a pixel buffer, so it only needs:

  * Pillow      - to turn that buffer into a PNG and to draw text with a real font
  * python-xlib - to put the interface on screen as a normal desktop window (X11)

Both are downloaded once into ``vendor/`` beside this file and used from there, so
nothing has to be installed globally and no administrator rights are needed.  If a
system-wide copy of either is already available, that is used instead and nothing
is downloaded.
"""

from __future__ import annotations

import hashlib
import io
import json
import os
import platform
import sys
import urllib.request
import zipfile

VENDOR_DIR = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "vendor")

# Dependencies, with a pinned fallback for the interpreter running on this machine.
# The pinned wheel is only used when PyPI cannot be reached.
PINS = {
    "pillow": {
        "filename": "pillow-13.0.0-cp314-cp314-manylinux_2_27_x86_64.manylinux_2_28_x86_64.whl",
        "url": "https://files.pythonhosted.org/packages/1d/f2/1371707ce438ba9a43f9d24089828494ec6f9396e28cd7903aa704dd6306/"
               "pillow-13.0.0-cp314-cp314-manylinux_2_27_x86_64.manylinux_2_28_x86_64.whl",
        "sha256": "e91c2b1cbeae768c98752e67e717e283d68dae45717b41f2d91eaee648e37b7d",
    },
    "python-xlib": {
        "filename": "python_xlib-0.33-py2.py3-none-any.whl",
        "url": "https://files.pythonhosted.org/packages/fc/b8/ff33610932e0ee81ae7f1269c890f697d56ff74b9f5b2ee5d9b7fa2c5355/"
               "python_xlib-0.33-py2.py3-none-any.whl",
        "sha256": "c3534038d42e0df2f1392a1b30a15a4ff5fdc2b86cfa94f072bf11b10a164398",
    },
    "six": {
        "filename": "six-1.17.0-py2.py3-none-any.whl",
        "url": "https://files.pythonhosted.org/packages/b7/ce/149a00dd41f10bc29e5921b496af8b574d8413afcd5e30dfa0ed46c2cc5e/"
               "six-1.17.0-py2.py3-none-any.whl",
        "sha256": "4721f391ed90541fddacab5acf947aa0d3dc7d27b2e1e8eda2be8970586c3274",
    },
}

_TAG_CACHE: dict[str, str] = {}


def _vendor_on_path() -> None:
    """Make sure the folder holding the downloaded libraries is importable."""
    if not os.path.isdir(VENDOR_DIR):
        try:
            os.makedirs(VENDOR_DIR, exist_ok=True)
        except OSError:
            return
    if VENDOR_DIR not in sys.path:
        sys.path.insert(0, VENDOR_DIR)


def _have(module: str) -> bool:
    try:
        __import__(module)
        return True
    except Exception:
        return False


def _wheel_tags() -> list[str]:
    """Wheel names this interpreter can load, most specific first."""
    v = sys.version_info
    abi = f"cp{v.major}{v.minor}"
    tags = []
    if sys.implementation.name != "cpython":
        return ["py3-none-any"]
    if platform.system() == "Linux":
        archs = ["manylinux_2_28_x86_64", "manylinux_2_27_x86_64", "manylinux2014_x86_64",
                 "manylinux_2_17_x86_64", "linux_x86_64"]
        if platform.machine() in ("aarch64", "arm64"):
            archs = ["manylinux_2_28_aarch64", "manylinux2014_aarch64", "linux_aarch64"]
        for arch in archs:
            tags.append(f"{abi}-{abi}-{arch}")
        for arch in archs:
            tags.append(f"{abi}-none-{arch}")
    elif platform.system() == "Darwin":
        tags.append(f"{abi}-{abi}-macosx_11_0_arm64")
        tags.append(f"{abi}-{abi}-macosx_10_9_x86_64")
    else:
        tags.append(f"{abi}-{abi}-win_amd64")
    tags.append(f"{abi}-none-any")
    tags.append("py3-none-any")
    return tags


def _pick_from_pypi(package: str) -> dict | None:
    """Ask PyPI which file fits this Python, newest release first."""
    try:
        with urllib.request.urlopen(f"https://pypi.org/pypi/{package}/json", timeout=30) as fh:
            meta = json.load(fh)
    except Exception:
        return None
    tags = _wheel_tags()
    releases = meta.get("releases") or {}
    def key(ver: str):
        parts = []
        for bit in ver.replace("-", ".").split("."):
            parts.append(int(bit) if bit.isdigit() else -1)
        return parts
    for version in sorted(releases, key=key, reverse=True):
        files = releases[version]
        for tag in tags:
            for entry in files:
                name = entry.get("filename", "")
                if not name.endswith(".whl") or entry.get("yanked"):
                    continue
                if tag not in name:
                    continue
                if entry.get("requires_python") and not _py_ok(entry["requires_python"]):
                    continue
                return {"filename": name, "url": entry["url"], "sha256": entry["digests"]["sha256"]}
    return None


def _py_ok(spec: str) -> bool:
    """Very small check of a requires_python spec against the running version."""
    v = sys.version_info
    here = (v.major, v.minor, v.micro)
    for clause in spec.split(","):
        clause = clause.strip()
        for op in (">=", "<=", "==", "!=", ">", "<"):
            if clause.startswith(op):
                want = clause[len(op):].strip()
                bits = [int(p) for p in want.split(".") if p.isdigit()]
                while len(bits) < 3:
                    bits.append(0)
                want_t = tuple(bits[:3])
                ok = {">=": here >= want_t, "<=": here <= want_t, "==": here[:2] == want_t[:2],
                      "!=": here[:2] != want_t[:2], ">": here > want_t, "<": here < want_t}[op]
                if not ok:
                    return False
                break
    return True


def _install(package: str, note: list[str]) -> bool:
    meta = _pick_from_pypi(package) or PINS.get(package)
    if not meta:
        note.append(f"could not find a {package} file for this Python ({sys.version.split()[0]})")
        return False
    try:
        with urllib.request.urlopen(meta["url"], timeout=120) as fh:
            blob = fh.read()
    except Exception as exc:  # offline
        note.append(f"download of {meta['filename']} failed: {exc}")
        return False
    digest = hashlib.sha256(blob).hexdigest()
    if meta.get("sha256") and digest != meta["sha256"]:
        note.append(f"{meta['filename']} did not match its published checksum")
        return False
    try:
        os.makedirs(VENDOR_DIR, exist_ok=True)
        with zipfile.ZipFile(io.BytesIO(blob)) as zf:
            for entry in zf.infolist():
                target = os.path.normpath(os.path.join(VENDOR_DIR, entry.filename))
                if not target.startswith(os.path.abspath(VENDOR_DIR) + os.sep):
                    continue
                if entry.is_dir():
                    os.makedirs(target, exist_ok=True)
                    continue
                os.makedirs(os.path.dirname(target), exist_ok=True)
                with zf.open(entry) as src, open(target, "wb") as dst:
                    dst.write(src.read())
    except Exception as exc:
        note.append(f"unpacking {meta['filename']} failed: {exc}")
        return False
    note.append(f"added {meta['filename']}")
    return True


def ensure(quiet: bool = False) -> tuple[bool, list[str]]:
    """Make sure Pillow and python-xlib can be imported. Returns (ok, notes)."""
    notes: list[str] = []
    _vendor_on_path()

    wanted = [("PIL", "pillow"), ("Xlib", "python-xlib"), ("six", "six")]
    missing = [(mod, pkg) for mod, pkg in wanted if not _have(mod)]
    for _mod, pkg in missing:
        _install(pkg, notes)
        _vendor_on_path()
        importlib_invalidate()
        if _have({"pillow": "PIL", "python-xlib": "Xlib", "six": "six"}[pkg]):
            notes.append(f"{pkg} ready")
    _vendor_on_path()
    ok_pil = _have("PIL")
    if not ok_pil:
        notes.append("Pillow is needed to draw the interface and could not be prepared. "
                     "Install it with:  python3 -m pip install pillow")
    if not quiet:
        for line in notes:
            print(f"[setup] {line}", file=sys.stderr)
    return ok_pil, notes


def importlib_invalidate() -> None:
    import importlib
    importlib.invalidate_caches()
