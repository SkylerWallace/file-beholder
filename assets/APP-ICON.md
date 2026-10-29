# Application icon

Angular amber folder flaps form an eye around a notched teal pupil. The top-right
fold has a narrow shaded edge, and the left edges align vertically. The white eye
has vertical ends inset slightly from the folder edges. This is the
vector adaptation of the selected option 01 concept. Artwork is covered by the
application's MIT license.

- `app-icon.svg`: editable vector source.
- `app-icon.png`: 1024 px transparent image.
- `app-icon.ico`: Windows icon with 16–256 px sizes.
- `app-icon.rgba`: 64 px RGBA pixels for Iced and sidebar branding.
- `app-icon.rc`: executable icon and Windows version resource template. `build.rs`
  fills its version fields from Cargo.toml and compiles it for MSVC or GNU builds.

After editing the SVG, export it as a transparent 1024 px square PNG, then run
`python scripts/generate-icon.py --png path/to/export.png` (Pillow 12+).
The script creates all raster assets from that export and never overwrites the SVG.
Python is only needed to regenerate artwork, not to build or run File Beholder.
