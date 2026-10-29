# Contributing

Use stable Rust 1.88 or newer on Windows. Cargo fetches the pinned public sk-iced revision automatically; a sibling checkout is not required. The MSVC toolchain requires Visual Studio Build Tools with Desktop development with C++ and a Windows SDK; GNU builds require MinGW-w64 and windres on PATH.

```powershell
cargo fmt --all -- --check
cargo test --locked -- --test-threads=1
cargo build --release --locked
```

Or run `./scripts/build.ps1 -Test`. The optional `-LocalToolchain` flag uses an existing ignored `.toolchain` directory; the repository does not download or redistribute compilers. `-Offline` requires dependencies to be cached already.

Keep UI pointer-motion state local where possible. File work belongs on workers, with cancellation and stale-result checks. Keep filesystem tests confined to owned fixtures. Two native tests (elevated MFT and Recycle Bin) are opt-in and ignored by the normal suite; run them only against disposable fixtures. Changes to drag overlays should retain incremental-versus-clean repaint tests at fractional DPI. Run UI tests serially: theme tests share the application typography context.

Use `FILE_BEHOLDER_CAPTURE=1` while running tests to write headless UI renders under target. The Windows workflow is currently disabled; when enabled, it checks formatting, tests, and a release build. Native default-app launching, audio, notifications, and UAC need an interactive smoke test when changed.

Retain third-party license headers and notices. Do not commit personal settings, sessions, transfer history, generated captures, toolchains, or compiled binaries. `./scripts/package-source.ps1` creates an explicit source-only ZIP without needing Git metadata.

When dependencies change, regenerate the embedded inventory with `python scripts/generate-licenses.py --target <windows-target>` and verify it with `--check`. Keep supplementary texts and source hashes under `docs/licenses/overrides`; see [license inventory documentation](docs/licenses/README.md).
