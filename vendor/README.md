# Shared renderer sources

The independent renderer copy and baseline patch were retired. Build from the
single `../sk-iced/renderer` companion selected in Cargo.toml. See its README,
provenance and hash manifest. `scripts/package-source.ps1` includes the exact
companion as vendor/shared-renderer and rewrites Cargo paths for exported builds.
