"""Compare original-resolution captures; requires Pillow, never resizes sources."""
import argparse
import json
from pathlib import Path

from PIL import Image, ImageDraw

parser = argparse.ArgumentParser()
root = Path(__file__).resolve().parent / "results/windows/electron-ui"
parser.add_argument("--reference", type=Path, default=root / "reference")
parser.add_argument("--native", type=Path, default=root.parent.parent / "bundle-dist/visuals")
parser.add_argument("--before", type=Path, help="Optional historical capture for a before/after font comparison")
args = parser.parse_args()


def combine(images, labels, destination):
    width, height = images[0].size
    assert all(image.size == (width, height) for image in images)
    canvas = Image.new("RGB", (width * len(images), height + 24), "#dddddd")
    draw = ImageDraw.Draw(canvas)
    for index, (image, label) in enumerate(zip(images, labels)):
        canvas.paste(image, (width * index, 24))
        draw.text((width * index + 8, 5), label, fill="black")
    canvas.save(destination)


for state in ["issue-tracker", "completed", "complete", "search", "empty"]:
    images = [Image.open(folder / (state + ".png")).convert("RGB")
              for folder in [args.reference, args.native]]
    assert images[0].size == (1650, 1080), "Expected fixed 1100x720 client at 150% DPI"
    combine(images, ["Electron reference", "Incular native"],
            args.native / ("compare-" + state + ".png"))

images = {name: Image.open(folder / "issue-tracker.png").convert("RGB")
          for name, folder in [("electron", args.reference), ("after", args.native)]}
for name, bounds in {"details": (1140, 170, 1610, 1080), "rows": (280, 230, 1090, 415),
                     "toolbar": (280, 25, 1615, 130)}.items():
    combine([images[key].crop(bounds) for key in ["electron", "after"]],
            ["Electron reference", "Incular native"], args.native / ("compare-" + name + ".png"))

if args.before is not None:
    images["before"] = Image.open(args.before / "issue-tracker.png").convert("RGB")
    combine([images[key].crop((1140, 340, 1610, 585)) for key in ["electron", "before", "after"]],
            ["Electron", "Before: rounded size + filtered placement", "After: fractional size + rasterized phase"],
            root / "font-comparison.png")

# Fixed physical-pixel crops; no registration or scaling to hide positioning errors.
# These are screenshot errors, including baseline/hinting/color differences, not
# a universal text-quality score. Foreground union excludes the mostly white area.
regions = {"description": [1160, 485, 1608, 608], "row_titles": [282, 227, 1092, 270],
           "toolbar_heading": [280, 28, 900, 74]}
result = {}
for name, bounds in regions.items():
    reference = list(images["electron"].crop(bounds).get_flattened_data())
    result[name] = {"physicalPixelBounds": bounds}
    for version in (["before", "after"] if args.before is not None else ["after"]):
        data = list(images[version].crop(bounds).get_flattened_data())
        foreground = [(a, b) for a, b in zip(reference, data) if min(a) < 230 or min(b) < 230]
        result[name][version] = {
            "foregroundUnionPixels": len(foreground),
            "meanAbsoluteRgbError": sum(abs(a[c] - b[c]) for a, b in foreground for c in range(3)) / (3 * len(foreground)),
            "allPixelMeanAbsoluteRgbError": sum(abs(a[c] - b[c]) for a, b in zip(reference, data) for c in range(3)) / (3 * len(reference)),
        }
(args.native / "font-pixel-comparison.json").write_text(json.dumps(result, indent=2), encoding="utf-8")
print(json.dumps(result, indent=2))
