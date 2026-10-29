use crate::core::alignment;
use crate::core::text::{Alignment, Shaping};
use crate::core::{Color, Font, Pixels, Point, Rectangle, Transformation};
use crate::graphics::text::cache::{self, Cache};
use crate::graphics::text::editor;
use crate::graphics::text::font_system;
use crate::graphics::text::paragraph;

use rustc_hash::{FxHashMap, FxHashSet};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::hash_map;

#[derive(Debug)]
pub struct Pipeline {
    glyph_cache: GlyphCache,
    cache: RefCell<Cache>,
}

impl Pipeline {
    pub fn new() -> Self {
        Pipeline {
            glyph_cache: GlyphCache::new(),
            cache: RefCell::new(Cache::new()),
        }
    }

    #[allow(dead_code)]
    pub fn load_font(&mut self, bytes: Cow<'static, [u8]>) {
        font_system()
            .write()
            .expect("Write font system")
            .load_font(bytes);

        self.cache = RefCell::new(Cache::new());
    }

    pub fn draw_paragraph(
        &mut self,
        paragraph: &paragraph::Weak,
        position: Point,
        color: Color,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        clip_mask: Option<&tiny_skia::Mask>,
        transformation: Transformation,
    ) {
        let Some(paragraph) = paragraph.upgrade() else {
            return;
        };

        let mut font_system = font_system().write().expect("Write font system");

        draw(
            font_system.raw(),
            &mut self.glyph_cache,
            paragraph.buffer(),
            position,
            color,
            pixels,
            clip_mask,
            transformation,
        );
    }

    pub fn draw_editor(
        &mut self,
        editor: &editor::Weak,
        position: Point,
        color: Color,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        clip_mask: Option<&tiny_skia::Mask>,
        transformation: Transformation,
    ) {
        let Some(editor) = editor.upgrade() else {
            return;
        };

        let mut font_system = font_system().write().expect("Write font system");

        draw(
            font_system.raw(),
            &mut self.glyph_cache,
            editor.buffer(),
            position,
            color,
            pixels,
            clip_mask,
            transformation,
        );
    }

    pub fn draw_cached(
        &mut self,
        content: &str,
        bounds: Rectangle,
        color: Color,
        size: Pixels,
        line_height: Pixels,
        font: Font,
        align_x: Alignment,
        align_y: alignment::Vertical,
        shaping: Shaping,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        clip_mask: Option<&tiny_skia::Mask>,
        transformation: Transformation,
    ) {
        let line_height = f32::from(line_height);

        let mut font_system = font_system().write().expect("Write font system");
        let font_system = font_system.raw();

        let key = cache::Key {
            bounds: bounds.size(),
            content,
            font,
            size: size.into(),
            line_height,
            shaping,
            align_x,
        };

        let (_, entry) = self.cache.get_mut().allocate(font_system, key);

        let width = entry.min_bounds.width;
        let height = entry.min_bounds.height;

        let x = match align_x {
            Alignment::Default | Alignment::Left | Alignment::Justified => bounds.x,
            Alignment::Center => bounds.x - width / 2.0,
            Alignment::Right => bounds.x - width,
        };

        let y = match align_y {
            alignment::Vertical::Top => bounds.y,
            alignment::Vertical::Center => bounds.y - height / 2.0,
            alignment::Vertical::Bottom => bounds.y - height,
        };

        draw(
            font_system,
            &mut self.glyph_cache,
            &entry.buffer,
            Point::new(x, y),
            color,
            pixels,
            clip_mask,
            transformation,
        );
    }

    pub fn draw_raw(
        &mut self,
        buffer: &cosmic_text::Buffer,
        position: Point,
        color: Color,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        clip_mask: Option<&tiny_skia::Mask>,
        transformation: Transformation,
    ) {
        let mut font_system = font_system().write().expect("Write font system");

        draw(
            font_system.raw(),
            &mut self.glyph_cache,
            buffer,
            position,
            color,
            pixels,
            clip_mask,
            transformation,
        );
    }

    pub fn trim_cache(&mut self) {
        self.cache.get_mut().trim();
        self.glyph_cache.trim();
    }
}

fn draw(
    font_system: &mut cosmic_text::FontSystem,
    glyph_cache: &mut GlyphCache,
    buffer: &cosmic_text::Buffer,
    position: Point,
    color: Color,
    pixels: &mut tiny_skia::PixmapMut<'_>,
    clip_mask: Option<&tiny_skia::Mask>,
    transformation: Transformation,
) {
    let position = position * transformation;

    let mut swash = cosmic_text::SwashCache::new();

    for run in buffer.layout_runs() {
        for glyph in run.glyphs {
            let physical_glyph =
                glyph.physical((position.x, position.y), transformation.scale_factor());

            if let Some((buffer, placement)) = glyph_cache.allocate(
                physical_glyph.cache_key,
                glyph.color_opt.map(from_color).unwrap_or(color),
                font_system,
                &mut swash,
            ) {
                let pixmap =
                    tiny_skia::PixmapRef::from_bytes(buffer, placement.width, placement.height)
                        .expect("Create glyph pixel map");

                let opacity =
                    color.a * glyph.color_opt.map(|c| c.a() as f32 / 255.0).unwrap_or(1.0);

                let x = physical_glyph.x + placement.left;
                let y = physical_glyph.y - placement.top
                    + (run.line_y * transformation.scale_factor()).round() as i32;
                if opacity == 1.0
                    && !crate::profile::reference_raster()
                    && blit_glyph(pixels, pixmap, x, y, clip_mask)
                {
                    continue;
                }
                pixels.draw_pixmap(
                    x,
                    y,
                    pixmap,
                    &tiny_skia::PixmapPaint {
                        opacity,
                        ..tiny_skia::PixmapPaint::default()
                    },
                    tiny_skia::Transform::identity(),
                    clip_mask,
                );
            }
        }
    }
}

fn from_color(color: cosmic_text::Color) -> Color {
    let [r, g, b, a] = color.as_rgba();

    Color::from_rgba8(r, g, b, a as f32 / 255.0)
}

#[derive(Debug, Clone, Default)]
struct GlyphCache {
    entries:
        FxHashMap<(cosmic_text::CacheKey, [u8; 3]), Option<(Vec<u32>, cosmic_text::Placement)>>,
    recently_used: FxHashSet<(cosmic_text::CacheKey, [u8; 3])>,
    trim_count: usize,
}

impl GlyphCache {
    const TRIM_INTERVAL: usize = 300;
    const CAPACITY_LIMIT: usize = 16 * 1024;

    fn new() -> Self {
        GlyphCache::default()
    }

    fn allocate(
        &mut self,
        cache_key: cosmic_text::CacheKey,
        color: Color,
        font_system: &mut cosmic_text::FontSystem,
        swash: &mut cosmic_text::SwashCache,
    ) -> Option<(&[u8], cosmic_text::Placement)> {
        let [r, g, b, _a] = color.into_rgba8();
        let key = (cache_key, [r, g, b]);

        let _ = self.recently_used.insert(key);
        if let hash_map::Entry::Vacant(entry) = self.entries.entry(key) {
            let Some(image) = swash.get_image_uncached(font_system, cache_key) else {
                // Whitespace and other non-rendering glyphs must also be cached.
                if !crate::profile::reference_raster() {
                    let _ = entry.insert(None);
                }
                return None;
            };

            let glyph_size = image.placement.width as usize * image.placement.height as usize;

            if glyph_size == 0 {
                if !crate::profile::reference_raster() {
                    let _ = entry.insert(None);
                }
                return None;
            }

            let mut buffer = vec![0u32; glyph_size];

            match image.content {
                cosmic_text::SwashContent::Mask => {
                    let mut i = 0;


                    for _y in 0..image.placement.height {
                        for _x in 0..image.placement.width {
                            buffer[i] = bytemuck::cast(
                                tiny_skia::ColorU8::from_rgba(b, g, r, image.data[i]).premultiply(),
                            );

                            i += 1;
                        }
                    }
                }
                cosmic_text::SwashContent::Color => {
                    let mut i = 0;

                    for _y in 0..image.placement.height {
                        for _x in 0..image.placement.width {
                            buffer[i >> 2] = bytemuck::cast(
                                tiny_skia::ColorU8::from_rgba(
                                    image.data[i + 2],
                                    image.data[i + 1],
                                    image.data[i],
                                    image.data[i + 3],
                                )
                                .premultiply(),
                            );

                            i += 4;
                        }
                    }
                }
                cosmic_text::SwashContent::SubpixelMask => {
                }
            }

            let _ = entry.insert(Some((buffer, image.placement)));
        }

        let _ = self.recently_used.insert(key);

        self.entries
            .get(&key)
            .and_then(|value| value.as_ref())
            .map(|(buffer, placement)| (bytemuck::cast_slice(buffer.as_slice()), *placement))
    }

    pub fn trim(&mut self) {
        if self.trim_count > Self::TRIM_INTERVAL || self.recently_used.len() >= Self::CAPACITY_LIMIT
        {
            self.entries
                .retain(|key, _| self.recently_used.contains(key));

            self.recently_used.clear();

            self.entries.shrink_to(Self::CAPACITY_LIMIT);
            self.recently_used.shrink_to(Self::CAPACITY_LIMIT);

            self.trim_count = 0;
        } else {
            self.trim_count += 1;
        }
    }
}

// Premultiplied source-over for an integer-positioned, unscaled glyph. This
// preserves the software rasterizer's float arithmetic and nearest-even output
// rounding without building a general image-sampling pipeline for every glyph.
fn blit_glyph(
    pixels: &mut tiny_skia::PixmapMut<'_>,
    glyph: tiny_skia::PixmapRef<'_>,
    x: i32,
    y: i32,
    mask: Option<&tiny_skia::Mask>,
) -> bool {
    if x < 0 || y < 0 {
        return false;
    }
    let width = pixels.width() as usize;
    let height = pixels.height() as usize;
    let left = (x as i64).max(0).min(width as i64) as usize;
    let top = (y as i64).max(0).min(height as i64) as usize;
    let right = (x as i64 + glyph.width() as i64).max(0).min(width as i64) as usize;
    let bottom = (y as i64 + glyph.height() as i64).max(0).min(height as i64) as usize;
    if right <= left || bottom <= top {
        return true;
    }
    let source = glyph.data();
    let destination = pixels.data_mut();
    const FACTOR: f32 = 1.0 / 255.0;
    for row in top..bottom {
        let source_start = ((row as i64 - y as i64) as usize * glyph.width() as usize
            + (left as i64 - x as i64) as usize)
            * 4;
        let target_start = (row * width + left) * 4;
        let src = &source[source_start..source_start + (right - left) * 4];
        let dst = &mut destination[target_start..target_start + (right - left) * 4];
        for (column, (s, d)) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)).enumerate() {
            let coverage = mask
                .map(|m| m.data()[row * width + left + column])
                .unwrap_or(255);
            if coverage == 0 || s[3] == 0 {
                continue;
            }
            let coverage = coverage as f32 / 255.0;
            let alpha = (s[3] as f32 * FACTOR) * coverage;
            let inverse = 1.0 - alpha;
            for channel in 0..4 {
                let source = (s[channel] as f32 * FACTOR) * coverage;
                let target = d[channel] as f32 * FACTOR;
                d[channel] =
                    ((target * inverse + source).clamp(0.0, 1.0) * 255.0).round_ties_even() as u8;
            }
        }
    }
    true
}
#[cfg(test)]
mod renderer_glyph_tests {
    use super::*;
    #[test]
    fn direct_glyph_blending_matches_reference() {
        for w in [7, 17, 256] {
            for (x, y) in [(3, 4), (-3, -4), (9, 11)] {
                for clipped in [false, true] {
                    let mut source = tiny_skia::Pixmap::new(w, 19).unwrap();
                    for (i, p) in source.data_mut().chunks_exact_mut(4).enumerate() {
                        let alpha = (i % 256) as u8;
                        p.copy_from_slice(&[alpha / 2, alpha / 3, alpha, alpha]);
                    }
                    let mut actual = tiny_skia::Pixmap::new(270, 40).unwrap();
                    for (i, p) in actual.data_mut().chunks_exact_mut(4).enumerate() {
                        let alpha = (i % 256) as u8;
                        p.copy_from_slice(&[alpha / 2, alpha / 4, alpha / 3, alpha]);
                    }
                    let mut expected = actual.clone();
                    let mut mask = tiny_skia::Mask::new(270, 40).unwrap();
                    for (i, p) in mask.data_mut().iter_mut().enumerate() {
                        *p = if i % 3 == 0 { 0 } else { 255 };
                    }
                    let mask = clipped.then_some(&mask);
                    if !blit_glyph(&mut actual.as_mut(), source.as_ref(), x, y, mask) {
                        actual.draw_pixmap(
                            x,
                            y,
                            source.as_ref(),
                            &tiny_skia::PixmapPaint::default(),
                            tiny_skia::Transform::identity(),
                            mask,
                        );
                    }
                    expected.draw_pixmap(
                        x,
                        y,
                        source.as_ref(),
                        &tiny_skia::PixmapPaint::default(),
                        tiny_skia::Transform::identity(),
                        mask,
                    );
                    assert!(
                        actual.data() == expected.data(),
                        "width={w} x={x} y={y} clipped={clipped}"
                    );
                }
            }
        }
    }
}
