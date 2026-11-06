# Third-party notices

File Beholder's application-owned code, artwork and completion tones are covered by the project's MIT license. The components below retain their own licenses and attribution.

## Iced and the software renderer

The shared renderer companion in `../sk-iced/renderer` contains upstream
`iced_tiny_skia` 0.14.1 (MIT), File Beholder extensions (MIT), and PC Tonic additions
and clipping oracles (Apache-2.0). Its original LICENSE, LICENSE-APACHE, NOTICE,
input provenance, exact patch and file hashes are retained together. This does
not relicense upstream Iced or File Beholder code. Both apps consume the same
active source and regression support; independent renderer copies are retired.

`SK_ICED_RENDERER_PRESENT_TRACE` enables opt-in local profiling; the legacy
`FILE_BEHOLDER_PRESENT_TRACE` alias remains supported. Normal operation records
no trace or telemetry. Source packages include the exact companion and rewrite
Cargo paths to `vendor/shared-renderer` for standalone rebuilds.

## Dependency inventory

Dependency versions are locked in Cargo.lock. The Windows release inventory in `docs/licenses/dependencies.json` retains component names, source URLs, license expressions, copyright notices, and license texts. It is embedded in Settings → Show licenses. Supplementary upstream texts and their source revisions/checksums are retained under `docs/licenses/overrides`.

The inventory includes Iced, tiny-skia, image, windows-sys, resvg/usvg, svgtypes, serde/serde_json, BLAKE3, and their active transitive dependencies. Their individual license terms remain unchanged.

## Icons

The bundled Material Design SVGs are from [Google Material Design Icons](https://github.com/google/material-design-icons) under Apache-2.0. Sources and the complete license are retained in `assets/material`. These include content_copy, content_cut, content_paste, delete, undo, search, manage_search, restore, create_new_folder, drive_file_rename_outline, folder_open, open_in_new, open_with, link, terminal, source, info, and security.

Windows Shell association artwork is supplied by the installed operating system and applications at runtime. File Beholder's own folder/eye icon, settings artwork, and disclosure masks are covered by the project's MIT license.

## sk-iced 0.1.0

The shared UI library is pinned to revision `9b07703f15d44abf82622851aa68385749d2274e` in Cargo.toml and Cargo.lock. Its published Apache-2.0/MIT license texts and NOTICE are retained unchanged under `docs/licenses/sk-iced` and in the dependency inventory. Historical product names inside those upstream notices refer to that pinned library distribution.

Matching source is available at https://github.com/SkylerWallace/sk-iced/tree/9b07703f15d44abf82622851aa68385749d2274e.

## Fonts

Fira Sans is bundled through iced_graphics. The font metadata credits digitized data copyright 2012–2016 to The Mozilla Foundation and Telefonica S.A.; its SIL Open Font License 1.1 is retained under `docs/licenses/fonts`.

Iced's bundled icon font retains its embedded “Copyright (C) 2020 by original authors @ fontello.com” attribution and the Iced distribution's MIT license. Both fonts are included in the in-app viewer.
