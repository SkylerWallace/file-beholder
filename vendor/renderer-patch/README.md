# Renderer provenance

This directory preserves File Beholder's baseline optimization patch for upstream `iced_tiny_skia 0.14.1`. The manifest records the original registry checksum, patch hash, and baseline changed-file hashes. Upstream Iced source and the application-owned extensions are MIT-licensed; retain `../iced_tiny_skia/LICENSE` and the project's LICENSE.

File Beholder builds directly from `../iced_tiny_skia` through the root Cargo.toml patch entry. The retained patch records the baseline; later clipping, repaint, and regression-test refinements are in the active source. `../iced_tiny_skia/SOURCE_CHECKSUMS.json` records the patched baseline's hashes, rather than the current modified files.

The renderer reuses clipping masks, coalesces damage, and optimizes supported opaque fills and glyphs. `FILE_BEHOLDER_PRESENT_TRACE` enables optional local profiling; normal operation records no trace or telemetry. Current behavior is checked by renderer pixel-oracle and application incremental-versus-clean repaint tests.
