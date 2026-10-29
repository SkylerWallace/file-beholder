"""Generate Windows and Iced icons from the raster export of app-icon.svg."""
from argparse import ArgumentParser
from pathlib import Path
from PIL import Image


def main():
    assets = Path(__file__).resolve().parents[1] / "assets"
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("--png", type=Path, default=assets / "app-icon.png",
                        help="Transparent SVG export, at least 1024 pixels square")
    args = parser.parse_args()
    with Image.open(args.png) as source:
        if source.width != source.height or source.width < 1024:
            parser.error("The PNG must be square and at least 1024 pixels wide")
        master = source.convert("RGBA").resize((1024, 1024), Image.Resampling.LANCZOS)
    master.save(assets / "app-icon.png")
    master.save(assets / "app-icon.ico", sizes=[
        (size, size) for size in (16, 20, 24, 32, 40, 48, 64, 128, 256)
    ])
    pixels = master.resize((64, 64), Image.Resampling.LANCZOS)
    (assets / "app-icon.rgba").write_bytes(pixels.tobytes())
    print("Generated PNG, multi-size ICO and 64px RGBA icon.")


if __name__ == "__main__":
    main()
