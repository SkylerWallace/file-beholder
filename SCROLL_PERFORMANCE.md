# Scrolling performance

Measured 2026-10-26 on this Windows machine using release builds and the tiny-skia CPU renderer.

## Method

The opt-in `scroll_cpu_benchmark` drives actual widget wheel and scrollbar-thumb events through the full 1400 × 900 workspace at 100% display scale, with two panels containing 2,000 synthetic files each. Animations are disabled so the measurements isolate scrolling. No directory scans, file-icon loading, or live transfers run during the test.

Each case renders 100 frames, discards 20 warm-up frames, and asserts that more than 80 frames actually change. Dark/light mode and wheel/drag cases are measured separately. Two serial before/optimized/historical passes run without concurrent compilation. The numbers below average the per-case medians or per-case 95th percentiles across the two passes and both color modes; they are not pooled percentiles.

CPU time includes event dispatch, invalidated layout, draw-command recording, damage tracking, and rasterization. It excludes Windows presentation, input-device latency, and monitor refresh. These results do not establish an interactive FPS guarantee.

Historical comparison: commit `4c651e4`, before shared-library/theme extraction, built from an isolated archive with the same harness (neumorphic cases omitted). The current before/after builds otherwise have identical UI/theme configuration.

## Results

Milliseconds per CPU frame; lower is better.

| Design/input | Historical typical | Before typical | Optimized typical | Before p95 | Optimized p95 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Default, wheel | 2.76 | 2.10 | 1.70 | 2.55 | 2.04 |
| Default, thumb drag | 8.81 | 12.38 | 7.49 | 15.50 | 10.36 |
| Neumorphic, wheel | — | 9.53 | 3.49 | 10.63 | 4.32 |
| Neumorphic, thumb drag | — | 18.76 | 7.91 | 45.04 | 19.74 |

The default-design drag regression relative to the historical build is resolved in this workload. This comparison identifies a regression in the intervening code, rather than attributing it solely to shared component extraction. Neumorphic drag tail latency remains above a 16.7 ms frame budget in some frames; no claim of consistent 60 FPS is made.

Detailed stage tracing identified rasterization as the main cost. Changes:

- Render rounded opaque caps using one even-odd path at their original coordinates, excluding the already-filled middle. This removes large mask-band copies while retaining native antialiasing.
- Scissor cached-shadow compositing to the damage region before running the image shader.
- Skip shadow pixels conservatively inside an opaque quad's fill, or inside the immediately following opaque quad in the same render batch. Transparent surfaces retain normal compositing.

No palette, dimensions, layout, input handling, scroll offset behavior, or animation policy changes are involved.

## Focused checks and reproduction

Renderer checks compare rounded fills with the native path at multiple scales and fractional clips, compare scissored/occluded shadow blending against native full-sprite painting, and exercise cache reuse. Retained-frame checks cover changing directory sizes and moving/removing shadows.

```powershell
cargo test -p iced_tiny_skia --lib --locked --offline
cargo test --locked --offline shadow_paint_checks
cargo test --release --locked --offline scroll_cpu_benchmark -- --ignored --nocapture --test-threads=1
```

The benchmark is ignored during ordinary test runs. Optional existing renderer stage tracing is enabled with `FILE_BEHOLDER_PRESENT_TRACE` set to a new JSONL file; leave it unset for normal timing runs. Raw local measurements are in `target/composition/scroll-results.json` and the serial benchmark logs.

## Additional preset measurements (2026-10-26)

Native gradient fills and transparent inset borders use bounded raster caches after repeated geometry; cold resize dimensions retain native drawing. Shadow masks initialize constant interior spans and evaluate straight horizontal edges once per half-row, preserving asymmetric corner radii. Pixel-reference checks pass at 1×, 1.25× and 2×.

Final serial release measurements use the existing workload and exclude Windows presentation:

| Preset | Wheel median, dark/light | Drag median, dark/light | Drag p95, dark/light |
| --- | --- | --- | --- |
| Default | 1.83 / 1.82 ms | 6.85 / 6.84 ms | 9.71 / 9.25 ms |
| Neumorphic | 3.28 / 3.26 ms | 7.84 / 7.59 ms | 18.58 / 18.58 ms |

Live-resize workload: 60 full frames with changing width, two panes with 500 entries each, dark mode, 10 warm-up frames excluded. Default median/p95: 7.98 / 8.79 ms. Neumorphic: 17.95 / 20.35 ms. Neumorphic resizing remains slower than Default; these changes do not establish performance parity or a 60 FPS guarantee.

Logs: `target/composition/{resize-final-span-bench,scroll-final-span-bench}.log`. Focused checks cover retained directory-size repaints, theme round trips, PE policy, sidebar geometry and scroll-state preservation.

## Neumorphic transparent-fill optimization (2026-10-27)

Inset bevels emit border-only quads with fully transparent backgrounds. The
software renderer now skips their transparent SourceOver fill, while retaining
normal border rasterization, shadows, clipping and cache behavior. This avoids
rasterizing a large no-op interior during every live resize. The inset-border
pixel comparison uses the original transparent-fill path as its reference at
1×, 1.25× and 2×; renderer reference checks pass. The Neumorphic retained-frame
directory-size regression also passes.

Same serial release resize workload, before versus optimized (CPU only):

| Style | Before median / p95 | Optimized median / p95 |
| --- | --- | --- |
| Default | 7.57 / 8.00 ms | 8.05 / 8.99 ms |
| Neumorphic | 16.90 / 19.64 ms | 13.34 / 15.05 ms |

Neumorphic median cost fell about 21%, and p95 about 23%. Default remains faster;
these measurements include the new toolbar and tint handling and are not an
interactive FPS guarantee. Raw logs: `target/composition/resize-before-next.log`
and `target/composition/resize-transparent-next.log`.
