"""Rasterize tray-icon SVG assets into multi-size Windows .ico files.

Sources supported:
    - A flat directory of <state>.svg files (e.g. the Non-macOS branding folder).
    - The macOS asset catalog: apple/client/Assets.xcassets/MenuBar*.imageset/*.svg.

Target: a fresh temp directory by default, or pass --out to choose one. The temp
default avoids accidentally clobbering checked-in icons; copy the result into
windows/Obscura Client/Assets/Tray/ once you've inspected them.

Each .ico embeds multiple sizes (16/32/48/64/256) re-rasterized from the SVG so the
shell picks the right one per DPI without resampling artifacts.

Requirements:
    pip install cairosvg pillow

Run from the repo root:
    python bin/svg_to_ico.py [SOURCE_DIR]

Note: macOS uses its imageset SVGs as "template" images — the OS auto-tints them.
Windows tray icons don't get auto-tinting; whatever colors are in the SVG ship to the
notification area. Prefer the Non-macOS branding folder, which already has the right
colors for Windows.
"""

from __future__ import annotations

import argparse
import sys
import tempfile
from io import BytesIO
from pathlib import Path

try:
    import cairosvg  # type: ignore
except ImportError:
    sys.exit("missing dependency: pip install cairosvg")

try:
    from PIL import Image
except ImportError:
    sys.exit("missing dependency: pip install pillow")


REPO_ROOT = Path(__file__).resolve().parents[1]
TRAY_SVG_DIR = REPO_ROOT / "rustlib" / "tray-icons"
WINDOWS_TRAY_ASSETS_DIR = REPO_ROOT / "windows" / "obscura-client" / "Assets" / "Tray"

# Covers 100%/125%/150%/200%/250%/300%/400% DPI scaling for a 16-logical-pixel tray slot.
SIZES = [(s, s) for s in (16, 32, 48, 64, 256)]


def find_svgs(source: Path) -> list[tuple[Path, str]]:
    """Return (svg_path, state_name) pairs found under `source`.

    Handles both a flat directory of <state>.svg files and an Apple-style asset
    catalog containing MenuBar*.imageset/ subdirectories.
    """
    pairs: list[tuple[Path, str]] = []

    for imageset in sorted(source.glob("MenuBar*.imageset")):
        name = imageset.name.removesuffix(".imageset").removeprefix("MenuBar")
        svg = next(iter(imageset.glob("*.svg")), None)
        if svg is None:
            print(f"warning: no svg found in {imageset.name}", file=sys.stderr)
            continue
        pairs.append((svg, name))

    for svg in sorted(source.glob("*.svg")):
        pairs.append((svg, svg.stem))

    return pairs


def convert(svg_path: Path, ico_path: Path) -> None:
    png_bytes = cairosvg.svg2png(
        url=str(svg_path), output_width=256, output_height=256
    )
    img = Image.open(BytesIO(png_bytes)).convert("RGBA")
    ico_path.parent.mkdir(parents=True, exist_ok=True)
    img.save(ico_path, sizes=SIZES, quality=100)


def display_path(p: Path) -> str:
    try:
        return str(p.relative_to(REPO_ROOT))
    except ValueError:
        return str(p)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "source",
        nargs="?",
        type=Path,
        default=TRAY_SVG_DIR,
        help="directory holding <state>.svg files or MenuBar*.imageset/ subdirs",
    )
    parser.add_argument(
        "-o",
        "--out",
        type=Path,
        default=WINDOWS_TRAY_ASSETS_DIR,
        help="output directory for .ico files",
    )
    parser.add_argument("-t", "--tempdir", action="store_true", help="export to temp dir")
    args = parser.parse_args()

    source: Path = args.source.expanduser().resolve()
    out_dir: Path = (
        args.out.expanduser().resolve()
        if not args.tempdir
        else Path(tempfile.mkdtemp(prefix="svg_to_ico_"))
    )

    pairs = find_svgs(source)
    if not pairs:
        print(f"no svg sources found under {source}", file=sys.stderr)
        return 1

    for svg, name in pairs:
        ico = out_dir / f"{name}.ico"
        convert(svg, ico)
        print(f"{display_path(svg)} -> {display_path(ico)}")
    print(ico.parent)
    return 0


if __name__ == "__main__":
    sys.exit(main())
