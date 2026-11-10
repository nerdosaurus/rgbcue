"""Look at a saved picture without an image viewer.

Prints the page as a coarse brightness map (so the layout can be judged in text),
then reports the number of distinct colours and whether any large area is a single
flat colour (a sign something did not draw).
"""

from __future__ import annotations

import sys

from PIL import Image

CHARS = " .:-=+*#%@"


def ascii_art(path: str, width: int = 126, height: int = 46) -> str:
    im = Image.open(path).convert("L")
    small = im.resize((width, height), Image.BOX)
    px = list(small.get_flattened_data())
    rows = []
    for y in range(height):
        rows.append("".join(CHARS[min(9, px[y * width + x] * 10 // 256)] for x in range(width)))
    return "\n".join(rows)


def main(argv: list[str]) -> int:
    if not argv:
        print("usage: python3 tools/view.py PICTURE [WIDTH] [HEIGHT]", file=sys.stderr)
        return 2
    path = argv[0]
    w = int(argv[1]) if len(argv) > 1 else 126
    h = int(argv[2]) if len(argv) > 2 else 46
    im = Image.open(path).convert("RGB")
    print(f"{path}  {im.size[0]}x{im.size[1]}")
    print(ascii_art(path, w, h))
    colours = im.getcolors(maxcolors=1 << 22) or []
    print(f"{len(colours)} distinct colours")
    # the single most common colour and its share tells us if a region failed to draw
    colours.sort(reverse=True)
    total = im.size[0] * im.size[1]
    for count, col in colours[:3]:
        print(f"  {col}  {100.0 * count / total:5.1f}%")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
