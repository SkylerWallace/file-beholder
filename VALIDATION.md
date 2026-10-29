# Validation

## Build and test

Use the configured Windows Rust toolchain. The GitHub workflow also defines a
Windows/MSVC build; local checks do not establish that the hosted build passed.

```powershell
$env:RUST_MIN_STACK = "16777216"
cargo fmt --all -- --check
cargo test --locked -- --test-threads=1
cargo build --release --locked
```

The larger test-thread stack accommodates headless rendering of complete views.
Set `FILE_BEHOLDER_CAPTURE=1` to write preview images under `target`. Those
images and test logs are generated artifacts, not application inputs.

Windows builds embed the file description and product name as `File Beholder`.
File and product versions come from Cargo.toml. Inspect the built resource with:

```powershell
(Get-Item target/release/file-beholder.exe).VersionInfo |
    Format-List FileDescription, ProductName, ProductVersion, FileVersion
```

## Focused UI checks

```powershell
cargo test -p sk-iced --lib --locked
cargo test -p sk-iced --test visual --locked
cargo test -p sk-iced --test components --locked segmented
cargo test -p iced_tiny_skia --lib --locked
cargo test --locked settings_ui::tests
cargo test --locked tree_header_actions
cargo test --locked adaptive_sidebar::tests
cargo test --locked shadow_paint_checks
cargo check --features sk-iced/serde,sk-iced/raster-icons --locked
```

Available styles are Default, Neumorphic, Material and Metro. Theme checks cover
round trips, tinting, contrast, persistent button selection, selector groups and
raised dropdowns. Unknown saved style keys fall back to Default.

Button checks cover centered content, automatic 32×32 icon buttons, content-fitting
text, explicit sizing, disabled input and immediate actions. The file-tree header
check verifies two equal buttons separated by four DIP. Settings checks cover
layout-icon centering, menu selections, swatch actions and popup bounds.

Sidebar checks cover compact-only tray actions, stable saved width and scroll
position across responsive transitions. Tab/panel checks cover stable identities,
selection, history, in-flight scans and cancellation when closing. Animation
checks cover Windows PE and reduced-motion policies, reversal, clipping, capture
cancellation and stopping frame requests when idle.

Renderer references compare rounded fills, shadow compositing, transparent inset
borders, gradients and SVG tint alpha. Retained-frame checks compare partial
updates with fresh rendering, including repeated directory-size changes.
See [SCROLL_PERFORMANCE.md](SCROLL_PERFORMANCE.md) for the benchmark workload and
previous measurements; CPU timings exclude native presentation.

## Application coverage

The application suite includes directory traversal, NTFS metadata aggregation,
filtering/sorting, path validation, settings/session round trips, docking and
virtualized scrolling. Transfer tests use owned fixtures for copy, move, delete,
Undo, retries, conflicts, cancellation and mixed-result reporting. Recovery and
activity dialogs are exercised with large synthetic result sets.

## Native checks

The normal suite does not trigger desktop notifications, audio, UAC or default-app
launches. Check those interactively when their implementations change. Native
Open With, Properties, VSS discovery and snapshot browsing also require Windows
smoke checks. Elevated MFT and Recycle Bin round trips are ignored by default and
require explicit opt-in against disposable fixtures.

A Windows PE boot and live changes to Windows animation settings remain manual
checks. Exercise compact/wide resizing, sidebar opening, keyboard navigation,
rapid tab/panel changes and popup dismissal in native UI smoke tests.

## Settings cards and switches

Focused shared tests cover immediate/disabled switch actions, motion-policy and
Windows PE snapping, and input-focus retention when settings cards stack on resize.
Application settings tests cover bounded scrolling, fixed header/footer, all accent
choices, layout presets and sound controls without playing audio. Light/dark
previews exercise Default, Neumorphic, Material and Metro materials.

## README screenshots

Generate the ten public screenshots from synthetic fixtures without reading user folders or performing transfers:

```powershell
cargo test --locked readme_captures::generate_readme_gallery -- --ignored --test-threads=1
```

The PNGs are written to `target/readme-captures/`. Inspect them before copying the ten `file-beholder-*.png` files into `docs/images/`. The fixture renders dark and light variants of all four themes, plus three- and four-panel layouts, with an expanded active transfer and populated Share columns. Use serial execution for captures because appearance tests share the typography context.

## Embedded licenses

`cargo test --locked licenses:: -- --test-threads=1` checks embedded texts, component filtering, modal isolation, Escape dismissal, and layouts at 480×480 and 1400×900. With `FILE_BEHOLDER_CAPTURE=1`, the layout check writes previews under `target/licenses-*.png`.

Run `python scripts/generate-licenses.py --check` after dependency changes. The generator fails on missing dependency license texts or changed supplementary-file checksums. [Inventory documentation](docs/licenses/README.md) describes regeneration for the release target.
