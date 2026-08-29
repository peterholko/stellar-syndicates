#!/usr/bin/env python3
"""Build a labeled contact sheet for visual QA of generated icon sources."""

from __future__ import annotations

import argparse
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    parser.add_argument("inputs", nargs="+", type=Path)
    parser.add_argument("--columns", type=int, default=5)
    parser.add_argument("--cell-size", type=int, default=140)
    parser.add_argument("--icon-size", type=int, default=64)
    args = parser.parse_args()

    thumb = max(32, args.cell_size)
    icon_size = max(8, min(args.icon_size, thumb - 16))
    label_h = 42
    cols = max(1, args.columns)
    rows = (len(args.inputs) + cols - 1) // cols
    sheet = Image.new("RGB", (cols * thumb, rows * (thumb + label_h)), "#05070d")
    draw = ImageDraw.Draw(sheet)
    font = ImageFont.load_default(size=15)

    for index, path in enumerate(args.inputs):
        image = Image.open(path).convert("RGBA")
        image.thumbnail((icon_size, icon_size), Image.Resampling.LANCZOS)
        col = index % cols
        row = index // cols
        x = col * thumb + (thumb - image.width) // 2
        y = row * (thumb + label_h) + (thumb - image.height) // 2
        sheet.paste(image, (x, y), image)
        label = path.stem.replace("exec-", "")
        label_box = draw.textbbox((0, 0), label, font=font)
        label_w = label_box[2] - label_box[0]
        draw.text(
            (col * thumb + (thumb - label_w) // 2, row * (thumb + label_h) + thumb + 10),
            label,
            fill="#d9f4ff",
            font=font,
        )

    args.output.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(args.output)


if __name__ == "__main__":
    main()
