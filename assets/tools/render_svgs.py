#!/usr/bin/env python3
"""Rasterise the cleaned iCUE line-art SVGs into the PNGs the app bundles.

The SVGs under `assets/*/svg/` are the source of truth. Each mouse has a
top view (`body_1`), a side view (`body_2`, or `body_3` for the wired
Scimitar) and optionally a layer SVG with `<g id="body|buttons|wheel|lights">`
groups. Layers are rendered on the full canvas so they line up when the app
draws them over each other.

Requires: rsvg-convert (librsvg2-bin) and ImageMagick `convert`.

Run from the repository root:

    python3 assets/tools/render_svgs.py
"""
import os
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))

# device -> (svg dir, top svg, side svg, side-layers svg, layer map, card radius)
DEVICES = {
    "darkstar_wireless": {
        "svg_dir": "assets/darkstar_wireless/svg",
        "top": "darkstar_wireless_body_1.svg",
        "side": "darkstar_wireless_body_2.svg",
        "layers": "darkstar_wireless_body_2_layers.svg",
        "layer_ids": {
            "side_body": "body",
            "side_buttons": "wheel",
            "side_lights": "lights",
        },
    },
    "scimitar_wireless": {
        "svg_dir": "assets/scimitar_wireless/svg",
        "top": "scimitar_elite_rgb_wireless_body_1.svg",
        "side": "scimitar_elite_rgb_wireless_body_2.svg",
        "layers": "scimitar_elite_rgb_wireless_body_2_layers.svg",
        "layer_ids": {
            "side_body": "body",
            "side_buttons": "buttons",
        },
    },
    "scimitar_wired": {
        "svg_dir": "assets/scimitar_wired/svg",
        "top": "scimitar_elite_rgb_body_1.svg",
        "side": "scimitar_elite_rgb_body_3.svg",
        "layers": "scimitar_elite_rgb_body_3_layers.svg",
        "layer_ids": {
            "side_body": "body",
            "side_buttons": "buttons",
        },
    },
}

K65_SVG = "assets/k65_rgb_mini/svg/k65_rgb_mini_na.svg"
K65_OUT = "assets/k65"


def run(cmd):
    subprocess.run(cmd, check=True)


def svg_size(path):
    root = ET.parse(os.path.join(ROOT, path)).getroot()
    width = float(root.attrib["width"])
    height = float(root.attrib["height"])
    return width, height


def render(svg, out, width):
    run(["rsvg-convert", "-w", str(round(width)), os.path.join(ROOT, svg), "-o", out])


def render_group(svg, group, out, width):
    """Render one `<g id=...>` on the full SVG canvas, so layers align."""
    tree = ET.parse(os.path.join(ROOT, svg))
    root = tree.getroot()
    for parent in root.iter():
        for child in list(parent):
            if child.tag.endswith("g") and child.attrib.get("id") and child.attrib["id"] != group:
                parent.remove(child)
    with tempfile.NamedTemporaryFile("w", suffix=".svg", delete=False) as tmp:
        tree.write(tmp.name)
        tmp_path = tmp.name
    try:
        run(["rsvg-convert", "-w", str(round(width)), tmp_path, "-o", out])
    finally:
        os.unlink(tmp_path)


def convert(args):
    run(["convert", *args])


def main():
    for name, cfg in DEVICES.items():
        svg_dir = cfg["svg_dir"]
        device_dir = os.path.join(ROOT, "assets", name)
        layers_dir = os.path.join(device_dir, "layers")
        os.makedirs(layers_dir, exist_ok=True)

        top_svg = os.path.join(svg_dir, cfg["top"])
        side_svg = os.path.join(svg_dir, cfg["side"])
        width, _ = svg_size(top_svg)
        side_width, _ = svg_size(side_svg)

        print(f"{name}: top {top_svg} ({width:.0f}w), side {side_svg} ({side_width:.0f}w)")

        top_png = os.path.join(device_dir, "top.png")
        side_png = os.path.join(device_dir, "side.png")
        render(top_svg, top_png, width)
        render(top_svg, os.path.join(device_dir, "top_2x.png"), width * 2)
        render(side_svg, side_png, side_width)
        render(side_svg, os.path.join(device_dir, "side_2x.png"), side_width * 2)

        # The dashboard card uses the side view with slightly thicker strokes.
        convert([side_png, "-channel", "A", "-morphology", "Dilate", "Disk:1",
                 "+channel", os.path.join(device_dir, "side_card.png")])

        layers_svg = os.path.join(svg_dir, cfg["layers"])
        for out_name, group in cfg["layer_ids"].items():
            out = os.path.join(layers_dir, f"{out_name}.png")
            render_group(layers_svg, group, out, side_width)
            render_group(layers_svg, group, os.path.join(layers_dir, f"{out_name}_2x.png"),
                         side_width * 2)
            print(f"  {out_name} <- #{group}")

        # Keep the full side layer beside the split ones for reference/tools.
        render(side_svg, os.path.join(layers_dir, "side.png"), side_width)
        render(side_svg, os.path.join(layers_dir, "side_2x.png"), side_width * 2)

    # K65 card drawing.
    width, _ = svg_size(K65_SVG)
    os.makedirs(os.path.join(ROOT, K65_OUT), exist_ok=True)
    card = os.path.join(ROOT, K65_OUT, "card.png")
    render(K65_SVG, card, width)
    convert([card, "-channel", "A", "-morphology", "Dilate", "Disk:4", "+channel",
             os.path.join(ROOT, K65_OUT, "card_thick.png")])
    print(f"k65: {K65_SVG} -> {K65_OUT}/card.png + card_thick.png ({width:.0f}w)")


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as exc:
        sys.exit(f"command failed: {' '.join(exc.cmd)}")
