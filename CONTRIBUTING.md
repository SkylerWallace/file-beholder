# Contributing

Use stable Rust 1.88 or newer on Windows. Cargo fetches the pinned public sk-iced revision automatically; the common renderer companion requires the matching `sk-iced` checkout beside this repository. The MSVC toolchain requires Visual Studio Build Tools with Desktop development with C++ and a Windows SDK; GNU builds require MinGW-w64 and windres on PATH.

```powershell
cargo fmt --all -- --check
cargo test --locked -- --test-threads=1
cargo build --release --locked
```

The development profile uses optimization level 1 for File Beholder and sk-iced to eliminate large widget-builder temporaries while retaining debug symbols, assertions, and overflow checks. Tests use the default thread stack; no `RUST_MIN_STACK` override is required. Cargo inherits these overrides in the test profile.

Or run `./scripts/build.ps1 -Test`. The optional `-LocalToolchain` flag uses an existing ignored `.toolchain` directory; the repository does not download or redistribute compilers. `-Offline` requires dependencies to be cached already.

Cargo outputs live in `build/target/` (also for direct Cargo commands); the build script copies the portable executable to `dist/file-beholder-v<VERSION>/`. This matches PC Tonic's build/dist layout. An explicit `CARGO_TARGET_DIR` overrides Cargo's default for direct commands.

Keep UI pointer-motion state local where possible. File work belongs on workers, with cancellation and stale-result checks. Keep filesystem tests confined to owned fixtures. Two native tests (elevated MFT and Recycle Bin) are opt-in and ignored by the normal suite; run them only against disposable fixtures. Changes to drag overlays should retain incremental-versus-clean repaint tests at fractional DPI. Run UI tests serially: theme tests share the application typography context.

Use `FILE_BEHOLDER_CAPTURE=1` while running tests to write headless UI renders under target. The Windows workflow is currently disabled; when enabled, it checks formatting, tests, and a release build. Native default-app launching, audio, notifications, and UAC need an interactive smoke test when changed.

Retain third-party license headers and notices. Do not commit personal settings, sessions, transfer history, generated captures, toolchains, or compiled binaries. `./scripts/package-source.ps1` creates an explicit source-only ZIP without needing Git metadata.

When dependencies change, regenerate the embedded inventory with `python scripts/generate-licenses.py --target <windows-target>` and verify it with `--check`. Keep supplementary texts and source hashes under `docs/licenses/overrides`; see [license inventory documentation](docs/licenses/README.md).

## Common software renderer

PC Tonic and File Beholder use the same companion under `../sk-iced/renderer`.
The UI library remains pinned to its public Git revision; the renderer and shared
test support are explicitly selected by local paths. Run
`python scripts/verify-renderer.py` before direct Cargo commands, and run
`cargo test -p iced_tiny_skia --lib --locked` plus the
`open_themed_dropdowns_remain_pixel_identical_after_partial_repaints` test when
updating it. The build/package scripts verify the shared snapshot. Source ZIPs
contain the full matching companion and rewrite these paths internally.

Shared renderer build/package verification uses Python 3.11+ for development only. The verifier also rejects a stale renderer/dependency license inventory; regenerate it after updating the companion or lockfile.
