use crate::Primitive;
use crate::core::renderer::Quad;
use crate::core::{Background, Color, Gradient, Rectangle, Size, Transformation, Vector};
use crate::graphics::{Image, Text};
use crate::text;

#[derive(Debug)]
pub struct Engine {
    text_pipeline: text::Pipeline,
    clip_bounds: Option<[u32; 4]>,
    clip_fills: u64,
    clip_skips: u64,
    // Bounded, position-independent shadow masks avoid rerasterizing large panels on scroll.
    shadow_cache: Vec<(Vec<u32>, tiny_skia::Pixmap)>,
    shadow_cache_bytes: usize,
    gradient_cache: Vec<(
        Quad,
        Background,
        Transformation,
        u32,
        u32,
        tiny_skia::Pixmap,
    )>,
    gradient_cache_bytes: usize,
    cache_admissions: Vec<(Quad, Background, Transformation, u32, u32)>,
    gradient_scratch: Option<tiny_skia::Pixmap>,
    border_mask_scratch: Option<tiny_skia::Mask>,
    cache_capture_depth: bool,
    #[cfg(test)]
    disable_gradient_cache: bool,

    #[cfg(feature = "image")]
    pub(crate) raster_pipeline: crate::raster::Pipeline,
    #[cfg(feature = "svg")]
    pub(crate) vector_pipeline: crate::vector::Pipeline,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            text_pipeline: text::Pipeline::new(),
            clip_bounds: None,
            clip_fills: 0,
            clip_skips: 0,
            shadow_cache: Vec::new(),
            shadow_cache_bytes: 0,
            gradient_cache: vec![],
            gradient_cache_bytes: 0,
            cache_admissions: vec![],
            gradient_scratch: None,
            border_mask_scratch: None,
            cache_capture_depth: false,
            #[cfg(test)]
            disable_gradient_cache: false,
            #[cfg(feature = "image")]
            raster_pipeline: crate::raster::Pipeline::new(),
            #[cfg(feature = "svg")]
            vector_pipeline: crate::vector::Pipeline::new(),
        }
    }

    pub fn reset_clip_mask(&mut self) {
        self.clip_bounds = None;
        self.clip_fills = 0;
        self.clip_skips = 0;
    }
    pub fn clip_stats(&self) -> (u64, u64) {
        (self.clip_fills, self.clip_skips)
    }
    pub fn adjust_clip_mask(&mut self, mask: &mut tiny_skia::Mask, bounds: Rectangle) {
        let key = [
            bounds.x.to_bits(),
            bounds.y.to_bits(),
            bounds.width.to_bits(),
            bounds.height.to_bits(),
        ];
        if self.clip_bounds == Some(key) && !crate::profile::uncached() {
            self.clip_skips += 1;
            return;
        }
        adjust_clip_mask(mask, bounds);
        self.clip_bounds = Some(key);
        self.clip_fills += 1;
    }

    #[cfg(test)]
    pub fn draw_quad(
        &mut self,
        quad: &Quad,
        background: &Background,
        transformation: Transformation,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        clip_mask: &mut tiny_skia::Mask,
        clip_bounds: Rectangle,
    ) {
        self.draw_quad_with_occlusion(
            quad,
            background,
            transformation,
            pixels,
            clip_mask,
            clip_bounds,
            None,
        );
    }
    pub(crate) fn draw_quad_with_occlusion(
        &mut self,
        quad: &Quad,
        background: &Background,
        transformation: Transformation,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        clip_mask: &mut tiny_skia::Mask,
        clip_bounds: Rectangle,
        cover: Option<Rectangle>,
    ) {
        let physical_bounds = quad.bounds * transformation;

        if !clip_bounds.intersects(&(crate::layer::quad_paint_bounds(quad) * transformation)) {
            return;
        }

        // Shadows extend beyond the quad and must obey both viewport and damage clips.
        self.adjust_clip_mask(clip_mask, clip_bounds);
        let shadow_clip_mask = &*clip_mask;
        // Antialiased borders can touch pixels outside their floating-point bounds.
        let clip_mask = Some(shadow_clip_mask);

        let transform = into_transform(transformation);

        // Make sure the border radius is not larger than the bounds
        let border_width = quad
            .border
            .width
            .min(quad.bounds.width / 2.0)
            .min(quad.bounds.height / 2.0);

        let mut fill_border_radius = <[f32; 4]>::from(quad.border.radius);

        for radius in &mut fill_border_radius {
            *radius = (*radius)
                .min(quad.bounds.width / 2.0)
                .min(quad.bounds.height / 2.0);
        }

        let path = rounded_rectangle(quad.bounds, fill_border_radius);

        let shadow = quad.shadow;

        if shadow.color.a > 0.0 {
            let cover = opaque_interior(quad, background, transformation).or(cover);
            let shadow_bounds = Rectangle {
                x: quad.bounds.x + shadow.offset.x - shadow.blur_radius,
                y: quad.bounds.y + shadow.offset.y - shadow.blur_radius,
                width: quad.bounds.width + shadow.blur_radius * 2.0,
                height: quad.bounds.height + shadow.blur_radius * 2.0,
            } * transformation;

            // Rasterize only the visible damaged portion, never a full panel-sized mask.
            if let Some(shadow_bounds) = shadow_bounds.intersection(&Rectangle {
                x: 0.,
                y: 0.,
                width: pixels.width() as f32,
                height: pixels.height() as f32,
            }) {
                let radii = fill_border_radius
                    .into_iter()
                    .map(|radius| radius * transformation.scale_factor())
                    .collect::<Vec<_>>();
                let (x, y, width, height) = (
                    shadow_bounds.x.floor() as u32,
                    shadow_bounds.y.floor() as u32,
                    (shadow_bounds.x + shadow_bounds.width).ceil() as u32
                        - shadow_bounds.x.floor() as u32,
                    (shadow_bounds.y + shadow_bounds.height).ceil() as u32
                        - shadow_bounds.y.floor() as u32,
                );
                let half_width = physical_bounds.width / 2.0;
                let half_height = physical_bounds.height / 2.0;

                let mut key = vec![
                    width,
                    height,
                    (x as f32 - physical_bounds.x).to_bits(),
                    (y as f32 - physical_bounds.y).to_bits(),
                    half_width.to_bits(),
                    half_height.to_bits(),
                    shadow.color.r.to_bits(),
                    shadow.color.g.to_bits(),
                    shadow.color.b.to_bits(),
                    shadow.color.a.to_bits(),
                    shadow.offset.x.to_bits(),
                    shadow.offset.y.to_bits(),
                    shadow.blur_radius.to_bits(),
                    transformation.scale_factor().to_bits(),
                ];
                key.extend(radii.iter().map(|r| r.to_bits()));
                if let Some(index) = self
                    .shadow_cache
                    .iter()
                    .position(|(cached, _)| *cached == key)
                {
                    let entry = self.shadow_cache.remove(index);
                    draw_shadow_sprite(
                        pixels,
                        x,
                        y,
                        entry.1.as_ref(),
                        shadow_clip_mask,
                        clip_bounds,
                        cover,
                    );
                    self.shadow_cache.push(entry);
                } else {
                    // Every point safely inside the rounded shadow has distance zero.
                    // Initialize that constant span and evaluate the SDF only at the rim.
                    let inside_alpha = 1.0
                        - smoothstep(
                            -shadow.blur_radius * transformation.scale_factor(),
                            shadow.blur_radius * transformation.scale_factor(),
                            0.0,
                        );
                    let mut inside_color = into_color(shadow.color);
                    inside_color.apply_opacity(inside_alpha);
                    let mut colors = vec![
                        inside_color.to_color_u8().premultiply();
                        width as usize * height as usize
                    ];
                    let radius = radii.iter().copied().fold(0.0_f32, f32::max);
                    let offset_x = shadow.offset.x * transformation.scale_factor();
                    let offset_y = shadow.offset.y * transformation.scale_factor();
                    let left = (physical_bounds.x + offset_x + radius).ceil();
                    let right =
                        (physical_bounds.x + offset_x + physical_bounds.width - radius).floor();
                    let top = (physical_bounds.y + offset_y + radius).ceil();
                    let bottom =
                        (physical_bounds.y + offset_y + physical_bounds.height - radius).floor();
                    for py in y..y + height {
                        let spans = if py as f32 >= top && (py as f32) < bottom {
                            [
                                (x, (left.max(x as f32).min((x + width) as f32)) as u32),
                                (
                                    (right.max(x as f32).min((x + width) as f32)) as u32,
                                    x + width,
                                ),
                            ]
                        } else {
                            // Across a straight horizontal edge the SDF is constant.
                            // Keep each half separate for asymmetric corner radii.
                            let start = left.max(x as f32).min((x + width) as f32) as u32;
                            let end = right.max(start as f32).min((x + width) as f32) as u32;
                            let center = (physical_bounds.x + offset_x + half_width)
                                .ceil()
                                .max(start as f32)
                                .min(end as f32) as u32;
                            for (a, b) in [(start, center), (center, end)] {
                                if a < b {
                                    if let Some(size) =
                                        tiny_skia::Size::from_wh(half_width, half_height)
                                    {
                                        let distance = rounded_box_sdf(
                                            Vector::new(
                                                a as f32
                                                    - physical_bounds.x
                                                    - offset_x
                                                    - half_width,
                                                py as f32
                                                    - physical_bounds.y
                                                    - offset_y
                                                    - half_height,
                                            ),
                                            size,
                                            &radii,
                                        )
                                        .max(0.);
                                        let alpha = 1.
                                            - smoothstep(
                                                -shadow.blur_radius * transformation.scale_factor(),
                                                shadow.blur_radius * transformation.scale_factor(),
                                                distance,
                                            );
                                        let mut color = into_color(shadow.color);
                                        color.apply_opacity(alpha);
                                        let row = ((py - y) * width) as usize;
                                        colors[row + (a - x) as usize..row + (b - x) as usize]
                                            .fill(color.to_color_u8().premultiply());
                                    }
                                }
                            }
                            [(x, start), (end, x + width)]
                        };
                        for (start, end) in spans {
                            for px in start..end {
                                if let Some(size) =
                                    tiny_skia::Size::from_wh(half_width, half_height)
                                {
                                    let distance = rounded_box_sdf(
                                        Vector::new(
                                            px as f32 - physical_bounds.x - offset_x - half_width,
                                            py as f32 - physical_bounds.y - offset_y - half_height,
                                        ),
                                        size,
                                        &radii,
                                    )
                                    .max(0.0);
                                    let alpha = 1.0
                                        - smoothstep(
                                            -shadow.blur_radius * transformation.scale_factor(),
                                            shadow.blur_radius * transformation.scale_factor(),
                                            distance,
                                        );
                                    let mut color = into_color(shadow.color);
                                    color.apply_opacity(alpha);
                                    colors[((py - y) * width + px - x) as usize] =
                                        color.to_color_u8().premultiply();
                                }
                            }
                        }
                    }

                    if let Some(pixmap) =
                        tiny_skia::IntSize::from_wh(width, height).and_then(|size| {
                            tiny_skia::Pixmap::from_vec(bytemuck::cast_vec(colors), size)
                        })
                    {
                        draw_shadow_sprite(
                            pixels,
                            x,
                            y,
                            pixmap.as_ref(),
                            shadow_clip_mask,
                            clip_bounds,
                            cover,
                        );
                        const BUDGET: usize = 16 * 1024 * 1024;
                        let bytes = pixmap.data().len();
                        if bytes <= BUDGET {
                            while self.shadow_cache_bytes + bytes > BUDGET
                                || self.shadow_cache.len() >= 64
                            {
                                let (_, oldest) = self.shadow_cache.remove(0);
                                self.shadow_cache_bytes -= oldest.data().len();
                            }
                            self.shadow_cache_bytes += bytes;
                            self.shadow_cache.push((key, pixmap));
                        }
                    }
                }
            }
        }

        if !self.cache_capture_depth
            && quad.shadow.color.a == 0.
            && quad.border.width > 0.
            && physical_bounds.width * physical_bounds.height >= 5120.
            && matches!(background,Background::Color(c) if c.a==0.)
            && self.draw_cached_border(
                quad,
                background,
                transformation,
                pixels,
                shadow_clip_mask,
                clip_bounds,
            )
        {
            return;
        }
        let fill_stage = crate::profile::stage("fill");
        let cached = matches!(background, Background::Gradient(_))
            && physical_bounds.width * physical_bounds.height >= 32000.
            && self.draw_cached_gradient(
                quad,
                background,
                transformation,
                &path,
                transform,
                pixels,
                shadow_clip_mask,
                clip_bounds,
            );
        let fast = cached
            || match background {
                // SourceOver with a fully transparent source is a no-op.
                // Inset bevels draw borders over large transparent fills.
                Background::Color(color) if color.a == 0.0 => {
                    #[cfg(test)]
                    {
                        !self.disable_gradient_cache
                    }
                    #[cfg(not(test))]
                    {
                        true
                    }
                }
                Background::Color(color) if color.a == 1.0 => fill_solid_interior(
                    pixels,
                    &path,
                    transform,
                    physical_bounds,
                    fill_border_radius.into_iter().fold(0.0_f32, f32::max)
                        * transformation.scale_factor(),
                    *color,
                    clip_mask,
                    clip_bounds,
                ),
                _ => false,
            };
        if !fast {
            pixels.fill_path(
                &path,
                &tiny_skia::Paint {
                    shader: background_shader(background, quad.bounds),
                    anti_alias: true,
                    ..tiny_skia::Paint::default()
                },
                tiny_skia::FillRule::EvenOdd,
                transform,
                clip_mask,
            );
        }
        drop(fill_stage);
        let _border_stage = crate::profile::stage("border");
        if border_width > 0.0 {
            // Border path is offset by half the border width
            let border_bounds = Rectangle {
                x: quad.bounds.x + border_width / 2.0,
                y: quad.bounds.y + border_width / 2.0,
                width: quad.bounds.width - border_width,
                height: quad.bounds.height - border_width,
            };

            // Make sure the border radius is correct
            let mut border_radius = <[f32; 4]>::from(quad.border.radius);
            let mut is_simple_border = true;

            for radius in &mut border_radius {
                *radius = if *radius == 0.0 {
                    // Path should handle this fine
                    0.0
                } else if *radius > border_width / 2.0 {
                    *radius - border_width / 2.0
                } else {
                    is_simple_border = false;
                    0.0
                }
                .min(border_bounds.width / 2.0)
                .min(border_bounds.height / 2.0);
            }

            // Stroking a path works well in this case
            if is_simple_border {
                let border_path = rounded_rectangle(border_bounds, border_radius);

                pixels.stroke_path(
                    &border_path,
                    &tiny_skia::Paint {
                        shader: tiny_skia::Shader::SolidColor(into_color(quad.border.color)),
                        anti_alias: true,
                        ..tiny_skia::Paint::default()
                    },
                    &tiny_skia::Stroke {
                        width: border_width,
                        ..tiny_skia::Stroke::default()
                    },
                    transform,
                    clip_mask,
                );
            } else {
                // Draw corners that have too small border radii as having no border radius,
                // but mask them with the rounded rectangle with the correct border radius.
                let mut temp_pixmap =
                    tiny_skia::Pixmap::new(quad.bounds.width as u32, quad.bounds.height as u32)
                        .unwrap();

                let mut quad_mask =
                    tiny_skia::Mask::new(quad.bounds.width as u32, quad.bounds.height as u32)
                        .unwrap();

                let zero_bounds = Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: quad.bounds.width,
                    height: quad.bounds.height,
                };
                let path = rounded_rectangle(zero_bounds, fill_border_radius);

                quad_mask.fill_path(&path, tiny_skia::FillRule::EvenOdd, true, transform);
                let path_bounds = Rectangle {
                    x: border_width / 2.0,
                    y: border_width / 2.0,
                    width: quad.bounds.width - border_width,
                    height: quad.bounds.height - border_width,
                };

                let border_radius_path = rounded_rectangle(path_bounds, border_radius);

                temp_pixmap.stroke_path(
                    &border_radius_path,
                    &tiny_skia::Paint {
                        shader: tiny_skia::Shader::SolidColor(into_color(quad.border.color)),
                        anti_alias: true,
                        ..tiny_skia::Paint::default()
                    },
                    &tiny_skia::Stroke {
                        width: border_width,
                        ..tiny_skia::Stroke::default()
                    },
                    transform,
                    Some(&quad_mask),
                );

                pixels.draw_pixmap(
                    quad.bounds.x as i32,
                    quad.bounds.y as i32,
                    temp_pixmap.as_ref(),
                    &tiny_skia::PixmapPaint::default(),
                    transform,
                    clip_mask,
                );
            }
        }
    }

    pub fn draw_text(
        &mut self,
        text: &Text,
        transformation: Transformation,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        clip_mask: &mut tiny_skia::Mask,
        clip_bounds: Rectangle,
    ) {
        match text {
            Text::Paragraph {
                paragraph,
                position,
                color,
                clip_bounds: local_clip_bounds,
                transformation: local_transformation,
            } => {
                let transformation = transformation * *local_transformation;
                let Some(clip_bounds) =
                    clip_bounds.intersection(&(*local_clip_bounds * transformation))
                else {
                    return;
                };

                let physical_bounds =
                    Rectangle::new(*position, paragraph.min_bounds) * transformation;

                if !clip_bounds.intersects(&physical_bounds) {
                    return;
                }

                let clip_mask = match physical_bounds.is_within(&clip_bounds) {
                    true => None,
                    false => {
                        self.adjust_clip_mask(clip_mask, clip_bounds);
                        Some(clip_mask as &_)
                    }
                };

                self.text_pipeline.draw_paragraph(
                    paragraph,
                    *position,
                    *color,
                    pixels,
                    clip_mask,
                    transformation,
                );
            }
            Text::Editor {
                editor,
                position,
                color,
                clip_bounds: local_clip_bounds,
                transformation: local_transformation,
            } => {
                let transformation = transformation * *local_transformation;
                let Some(clip_bounds) =
                    clip_bounds.intersection(&(*local_clip_bounds * transformation))
                else {
                    return;
                };

                let physical_bounds = Rectangle::new(*position, editor.bounds) * transformation;

                if !clip_bounds.intersects(&physical_bounds) {
                    return;
                }

                let clip_mask = match physical_bounds.is_within(&clip_bounds) {
                    true => None,
                    false => {
                        self.adjust_clip_mask(clip_mask, clip_bounds);
                        Some(clip_mask as &_)
                    }
                };

                self.text_pipeline.draw_editor(
                    editor,
                    *position,
                    *color,
                    pixels,
                    clip_mask,
                    transformation,
                );
            }
            Text::Cached {
                content,
                bounds,
                color,
                size,
                line_height,
                font,
                align_x,
                align_y,
                shaping,
                clip_bounds: local_clip_bounds,
            } => {
                // Cached text (including pick-list labels) must obey its local
                // viewport even when that viewport is fully inside the damage.
                let Some(clip_bounds) = clip_bounds.intersection(&(*local_clip_bounds * transformation)) else {
                    return;
                };
                self.adjust_clip_mask(clip_mask, clip_bounds);
                let clip_mask = Some(clip_mask as &_);

                self.text_pipeline.draw_cached(
                    content,
                    *bounds,
                    *color,
                    *size,
                    *line_height,
                    *font,
                    *align_x,
                    *align_y,
                    *shaping,
                    pixels,
                    clip_mask,
                    transformation,
                );
            }
            Text::Raw {
                raw,
                transformation: local_transformation,
            } => {
                let Some(buffer) = raw.buffer.upgrade() else {
                    return;
                };

                let transformation = transformation * *local_transformation;
                let (width, height) = buffer.size();

                let physical_bounds = Rectangle::new(
                    raw.position,
                    Size::new(
                        width.unwrap_or(clip_bounds.width),
                        height.unwrap_or(clip_bounds.height),
                    ),
                ) * transformation;

                if !clip_bounds.intersects(&physical_bounds) {
                    return;
                }

                self.adjust_clip_mask(clip_mask, clip_bounds);
                let clip_mask = Some(clip_mask as &_);

                self.text_pipeline.draw_raw(
                    &buffer,
                    raw.position,
                    raw.color,
                    pixels,
                    clip_mask,
                    transformation,
                );
            }
        }
    }

    // One-off sizes during live resize use the native path. Only repeated geometry
    // earns a raster snapshot, avoiding full-frame scratch work on each resize.
    fn admit_cache(
        &mut self,
        quad: &Quad,
        background: &Background,
        transform: Transformation,
        width: u32,
        height: u32,
    ) -> bool {
        if self
            .cache_admissions
            .first()
            .is_some_and(|(_, _, _, w, h)| *w != width || *h != height)
        {
            self.cache_admissions.clear();
        }
        let key = (*quad, *background, transform, width, height);
        if self.cache_admissions.contains(&key) {
            return true;
        }
        if self.cache_admissions.len() >= 64 {
            let _ = self.cache_admissions.remove(0);
        }
        self.cache_admissions.push(key);
        false
    }
    fn draw_cached_border(
        &mut self,
        quad: &Quad,
        background: &Background,
        transformation: Transformation,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        mask: &tiny_skia::Mask,
        clip: Rectangle,
    ) -> bool {
        #[cfg(test)]
        if self.disable_gradient_cache {
            return false;
        }
        let width = pixels.width();
        let height = pixels.height();
        let bounds = quad.bounds * transformation;
        let x = bounds.x.floor().max(0.).min(width as f32) as u32;
        let y = bounds.y.floor().max(0.).min(height as f32) as u32;
        let right = (bounds.x + bounds.width)
            .ceil()
            .max(x as f32)
            .min(width as f32) as u32;
        let bottom = (bounds.y + bounds.height)
            .ceil()
            .max(y as f32)
            .min(height as f32) as u32;
        let bytes = (right - x) as usize * (bottom - y) as usize * 4;
        const BUDGET: usize = 16 * 1024 * 1024;
        if bytes == 0 || bytes > BUDGET / 2 {
            return false;
        }
        if let Some(index) = self.gradient_cache.iter().position(|(q, b, t, w, h, _)| {
            q == quad && b == background && *t == transformation && *w == width && *h == height
        }) {
            let entry = self.gradient_cache.remove(index);
            draw_shadow_sprite(pixels, x, y, entry.5.as_ref(), mask, clip, None);
            self.gradient_cache.push(entry);
            return true;
        }
        if !self.admit_cache(quad, background, transformation, width, height) {
            return false;
        }
        let Some(mut scratch) = self
            .gradient_scratch
            .take()
            .filter(|p| p.width() == width && p.height() == height)
            .or_else(|| tiny_skia::Pixmap::new(width, height))
        else {
            return false;
        };
        let Some(mut scratch_mask) = self
            .border_mask_scratch
            .take()
            .filter(|p| p.width() == width && p.height() == height)
            .or_else(|| tiny_skia::Mask::new(width, height))
        else {
            return false;
        };
        scratch.fill(tiny_skia::Color::TRANSPARENT);
        self.cache_capture_depth = true;
        self.clip_bounds = None;
        self.draw_quad_with_occlusion(
            quad,
            background,
            transformation,
            &mut scratch.as_mut(),
            &mut scratch_mask,
            Rectangle {
                x: 0.,
                y: 0.,
                width: width as f32,
                height: height as f32,
            },
            None,
        );
        self.cache_capture_depth = false;
        self.clip_bounds = None;
        let image = tiny_skia::IntRect::from_xywh(x as i32, y as i32, right - x, bottom - y)
            .and_then(|r| scratch.clone_rect(r));
        self.gradient_scratch = Some(scratch);
        self.border_mask_scratch = Some(scratch_mask);
        let Some(image) = image else {
            return false;
        };
        draw_shadow_sprite(pixels, x, y, image.as_ref(), mask, clip, None);
        while self.gradient_cache_bytes + bytes > BUDGET || self.gradient_cache.len() >= 16 {
            let (_, _, _, _, _, oldest) = self.gradient_cache.remove(0);
            self.gradient_cache_bytes -= oldest.data().len();
        }
        self.gradient_cache_bytes += bytes;
        self.gradient_cache
            .push((*quad, *background, transformation, width, height, image));
        true
    }
    fn draw_cached_gradient(
        &mut self,
        quad: &Quad,
        background: &Background,
        transformation: Transformation,
        path: &tiny_skia::Path,
        transform: tiny_skia::Transform,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        mask: &tiny_skia::Mask,
        clip: Rectangle,
    ) -> bool {
        #[cfg(test)]
        if self.disable_gradient_cache {
            return false;
        }
        let width = pixels.width();
        let height = pixels.height();
        let bounds = quad.bounds * transformation;
        let x = bounds.x.floor().max(0.).min(width as f32) as u32;
        let y = bounds.y.floor().max(0.).min(height as f32) as u32;
        let right = (bounds.x + bounds.width)
            .ceil()
            .max(x as f32)
            .min(width as f32) as u32;
        let bottom = (bounds.y + bounds.height)
            .ceil()
            .max(y as f32)
            .min(height as f32) as u32;
        let bytes = (right - x) as usize * (bottom - y) as usize * 4;
        const BUDGET: usize = 16 * 1024 * 1024;
        if bytes == 0 || bytes > BUDGET / 2 {
            return false;
        }
        if let Some(index) = self.gradient_cache.iter().position(|(q, b, t, w, h, _)| {
            q == quad && b == background && *t == transformation && *w == width && *h == height
        }) {
            let entry = self.gradient_cache.remove(index);
            draw_shadow_sprite(pixels, x, y, entry.5.as_ref(), mask, clip, None);
            self.gradient_cache.push(entry);
            return true;
        }
        if !self.admit_cache(quad, background, transformation, width, height) {
            return false;
        }
        if self
            .gradient_scratch
            .as_ref()
            .is_none_or(|p| p.width() != width || p.height() != height)
        {
            self.gradient_scratch = tiny_skia::Pixmap::new(width, height);
        }
        let Some(scratch) = self.gradient_scratch.as_mut() else {
            return false;
        };
        scratch.fill(tiny_skia::Color::TRANSPARENT);
        scratch.fill_path(
            path,
            &tiny_skia::Paint {
                shader: background_shader(background, quad.bounds),
                anti_alias: true,
                ..Default::default()
            },
            tiny_skia::FillRule::EvenOdd,
            transform,
            None,
        );
        let Some(image) = tiny_skia::IntRect::from_xywh(x as i32, y as i32, right - x, bottom - y)
            .and_then(|r| scratch.clone_rect(r))
        else {
            return false;
        };
        draw_shadow_sprite(pixels, x, y, image.as_ref(), mask, clip, None);
        while self.gradient_cache_bytes + bytes > BUDGET || self.gradient_cache.len() >= 16 {
            let (_, _, _, _, _, oldest) = self.gradient_cache.remove(0);
            self.gradient_cache_bytes -= oldest.data().len();
        }
        self.gradient_cache_bytes += bytes;
        self.gradient_cache
            .push((*quad, *background, transformation, width, height, image));
        true
    }
    pub fn draw_primitive(
        &mut self,
        primitive: &Primitive,
        transformation: Transformation,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        clip_mask: &mut tiny_skia::Mask,
        clip_bounds: Rectangle,
    ) {
        match primitive {
            Primitive::Fill { path, paint, rule } => {
                let physical_bounds = {
                    let bounds = path.bounds();

                    Rectangle {
                        x: bounds.x(),
                        y: bounds.y(),
                        width: bounds.width(),
                        height: bounds.height(),
                    } * transformation
                };

                if !clip_bounds.intersects(&physical_bounds) {
                    return;
                }

                let clip_mask =
                    (!physical_bounds.is_within(&clip_bounds)).then_some(clip_mask as &_);

                pixels.fill_path(
                    path,
                    paint,
                    *rule,
                    into_transform(transformation),
                    clip_mask,
                );
            }
            Primitive::Stroke {
                path,
                paint,
                stroke,
            } => {
                let physical_bounds = {
                    let bounds = path.bounds();

                    Rectangle {
                        x: bounds.x() - stroke.width / 2.0,
                        y: bounds.y() - stroke.width / 2.0,
                        width: bounds.width() + stroke.width,
                        height: bounds.height() + stroke.width,
                    } * transformation
                };

                if !clip_bounds.intersects(&physical_bounds) {
                    return;
                }

                let clip_mask =
                    (!physical_bounds.is_within(&clip_bounds)).then_some(clip_mask as &_);

                pixels.stroke_path(
                    path,
                    paint,
                    stroke,
                    into_transform(transformation),
                    clip_mask,
                );
            }
        }
    }

    pub fn draw_image(
        &mut self,
        image: &Image,
        _transformation: Transformation,
        _pixels: &mut tiny_skia::PixmapMut<'_>,
        _clip_mask: &mut tiny_skia::Mask,
        _clip_bounds: Rectangle,
    ) {
        match image {
            #[cfg(feature = "image")]
            Image::Raster { image, bounds, .. } => {
                let physical_bounds = *bounds * _transformation;

                if !_clip_bounds.intersects(&physical_bounds) {
                    return;
                }

                let clip_mask =
                    (!physical_bounds.is_within(&_clip_bounds)).then_some(_clip_mask as &_);

                let center = physical_bounds.center();
                let radians = f32::from(image.rotation);

                let transform = into_transform(_transformation).post_rotate_at(
                    radians.to_degrees(),
                    center.x,
                    center.y,
                );

                self.raster_pipeline.draw(
                    &image.handle,
                    image.filter_method,
                    *bounds,
                    image.opacity,
                    _pixels,
                    transform,
                    clip_mask,
                );
            }
            #[cfg(feature = "svg")]
            Image::Vector { svg, bounds, .. } => {
                let physical_bounds = *bounds * _transformation;

                if !_clip_bounds.intersects(&physical_bounds) {
                    return;
                }

                let clip_mask =
                    (!physical_bounds.is_within(&_clip_bounds)).then_some(_clip_mask as &_);

                let center = physical_bounds.center();
                let radians = f32::from(svg.rotation);

                let transform = into_transform(_transformation).post_rotate_at(
                    radians.to_degrees(),
                    center.x,
                    center.y,
                );

                self.vector_pipeline.draw(
                    &svg.handle,
                    svg.color,
                    *bounds,
                    svg.opacity,
                    _pixels,
                    transform,
                    clip_mask,
                );
            }
            #[cfg(not(feature = "image"))]
            Image::Raster { .. } => {
                log::warn!("Unsupported primitive in `iced_tiny_skia`: {image:?}",);
            }
            #[cfg(not(feature = "svg"))]
            Image::Vector { .. } => {
                log::warn!("Unsupported primitive in `iced_tiny_skia`: {image:?}",);
            }
        }
    }

    pub fn trim(&mut self) {
        self.text_pipeline.trim_cache();

        #[cfg(feature = "image")]
        self.raster_pipeline.trim_cache();

        #[cfg(feature = "svg")]
        self.vector_pipeline.trim_cache();
    }
}

pub fn into_color(color: Color) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba(color.b, color.g, color.r, color.a)
        .expect("Convert color from iced to tiny_skia")
}

fn into_transform(transformation: Transformation) -> tiny_skia::Transform {
    let translation = transformation.translation();

    tiny_skia::Transform {
        sx: transformation.scale_factor(),
        kx: 0.0,
        ky: 0.0,
        sy: transformation.scale_factor(),
        tx: translation.x,
        ty: translation.y,
    }
}

pub(crate) fn rounded_rectangle(bounds: Rectangle, border_radius: [f32; 4]) -> tiny_skia::Path {
    let [top_left, top_right, bottom_right, bottom_left] = border_radius;

    if top_left == 0.0 && top_right == 0.0 && bottom_right == 0.0 && bottom_left == 0.0 {
        return tiny_skia::PathBuilder::from_rect(
            tiny_skia::Rect::from_xywh(bounds.x, bounds.y, bounds.width, bounds.height)
                .expect("Build quad rectangle"),
        );
    }

    if top_left == top_right
        && top_left == bottom_right
        && top_left == bottom_left
        && top_left == bounds.width / 2.0
        && top_left == bounds.height / 2.0
    {
        return tiny_skia::PathBuilder::from_circle(
            bounds.x + bounds.width / 2.0,
            bounds.y + bounds.height / 2.0,
            top_left,
        )
        .expect("Build circle path");
    }

    let mut builder = tiny_skia::PathBuilder::new();

    builder.move_to(bounds.x + top_left, bounds.y);
    builder.line_to(bounds.x + bounds.width - top_right, bounds.y);

    if top_right > 0.0 {
        arc_to(
            &mut builder,
            bounds.x + bounds.width - top_right,
            bounds.y,
            bounds.x + bounds.width,
            bounds.y + top_right,
            top_right,
        );
    }

    maybe_line_to(
        &mut builder,
        bounds.x + bounds.width,
        bounds.y + bounds.height - bottom_right,
    );

    if bottom_right > 0.0 {
        arc_to(
            &mut builder,
            bounds.x + bounds.width,
            bounds.y + bounds.height - bottom_right,
            bounds.x + bounds.width - bottom_right,
            bounds.y + bounds.height,
            bottom_right,
        );
    }

    maybe_line_to(
        &mut builder,
        bounds.x + bottom_left,
        bounds.y + bounds.height,
    );

    if bottom_left > 0.0 {
        arc_to(
            &mut builder,
            bounds.x + bottom_left,
            bounds.y + bounds.height,
            bounds.x,
            bounds.y + bounds.height - bottom_left,
            bottom_left,
        );
    }

    maybe_line_to(&mut builder, bounds.x, bounds.y + top_left);

    if top_left > 0.0 {
        arc_to(
            &mut builder,
            bounds.x,
            bounds.y + top_left,
            bounds.x + top_left,
            bounds.y,
            top_left,
        );
    }

    builder.finish().expect("Build rounded rectangle path")
}

fn maybe_line_to(path: &mut tiny_skia::PathBuilder, x: f32, y: f32) {
    if path.last_point() != Some(tiny_skia::Point { x, y }) {
        path.line_to(x, y);
    }
}

fn arc_to(
    path: &mut tiny_skia::PathBuilder,
    x_from: f32,
    y_from: f32,
    x_to: f32,
    y_to: f32,
    radius: f32,
) {
    let svg_arc = kurbo::SvgArc {
        from: kurbo::Point::new(f64::from(x_from), f64::from(y_from)),
        to: kurbo::Point::new(f64::from(x_to), f64::from(y_to)),
        radii: kurbo::Vec2::new(f64::from(radius), f64::from(radius)),
        x_rotation: 0.0,
        large_arc: false,
        sweep: true,
    };

    match kurbo::Arc::from_svg_arc(&svg_arc) {
        Some(arc) => {
            arc.to_cubic_beziers(0.1, |p1, p2, p| {
                path.cubic_to(
                    p1.x as f32,
                    p1.y as f32,
                    p2.x as f32,
                    p2.y as f32,
                    p.x as f32,
                    p.y as f32,
                );
            });
        }
        None => {
            path.line_to(x_to, y_to);
        }
    }
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let x = ((x - a) / (b - a)).clamp(0.0, 1.0);

    x * x * (3.0 - 2.0 * x)
}

fn rounded_box_sdf(to_center: Vector, size: tiny_skia::Size, radii: &[f32]) -> f32 {
    let radius = match (to_center.x > 0.0, to_center.y > 0.0) {
        (true, true) => radii[2],
        (true, false) => radii[1],
        (false, true) => radii[3],
        (false, false) => radii[0],
    };

    let x = (to_center.x.abs() - size.width() + radius).max(0.0);
    let y = (to_center.y.abs() - size.height() + radius).max(0.0);

    (x.powf(2.0) + y.powf(2.0)).sqrt() - radius
}

pub fn adjust_clip_mask(clip_mask: &mut tiny_skia::Mask, bounds: Rectangle) {
    clip_mask.clear();

    let path = {
        let mut builder = tiny_skia::PathBuilder::new();
        builder.push_rect(
            tiny_skia::Rect::from_xywh(bounds.x, bounds.y, bounds.width, bounds.height).unwrap(),
        );

        builder.finish().unwrap()
    };

    clip_mask.fill_path(
        &path,
        tiny_skia::FillRule::EvenOdd,
        false,
        tiny_skia::Transform::default(),
    );
}

#[cfg(test)]
mod renderer_clip_tests {
    use super::*;
    #[test]
    fn identical_bounds_and_frame_reset_preserve_mask_bytes() {
        let mut engine = Engine::new();
        let mut cached = tiny_skia::Mask::new(160, 100).unwrap();
        let mut original = tiny_skia::Mask::new(160, 100).unwrap();
        let rects = [
            Rectangle {
                x: 2.25,
                y: 3.75,
                width: 90.5,
                height: 70.0,
            },
            Rectangle {
                x: 2.25,
                y: 3.75,
                width: 90.5,
                height: 70.0,
            },
            Rectangle {
                x: 0.0,
                y: 10.0,
                width: 80.0,
                height: 30.0,
            },
        ];
        for b in rects {
            engine.adjust_clip_mask(&mut cached, b);
            adjust_clip_mask(&mut original, b);
            assert_eq!(cached.data(), original.data());
        }
        assert_eq!(engine.clip_stats(), (2, 1));
        engine.reset_clip_mask();
        let mut fresh = tiny_skia::Mask::new(160, 100).unwrap();
        engine.adjust_clip_mask(&mut fresh, rects[2]);
        assert_eq!(fresh.data(), original.data());
        assert_eq!(engine.clip_stats(), (1, 0));
        let mut second = Engine::new();
        second.adjust_clip_mask(&mut original, rects[0]);
        engine.adjust_clip_mask(&mut fresh, rects[2]);
        let mut oracle = tiny_skia::Mask::new(160, 100).unwrap();
        adjust_clip_mask(&mut oracle, rects[2]);
        assert_eq!(fresh.data(), oracle.data());
    }
}

// Keep the reference rasterizer for curved edge bands. Only the known opaque,
// integer-aligned interior is filled directly; clipped/translucent cases fall back.
fn fill_solid_interior(
    pixels: &mut tiny_skia::PixmapMut<'_>,
    path: &tiny_skia::Path,
    transform: tiny_skia::Transform,
    bounds: Rectangle,
    radius: f32,
    color: Color,
    mask: Option<&tiny_skia::Mask>,
    clip: Rectangle,
) -> bool {
    if crate::profile::reference_raster() {
        return false;
    }
    let left = bounds.x;
    let right = bounds.x + bounds.width;
    if ![clip.x, clip.y, clip.width, clip.height]
        .into_iter()
        .all(f32::is_finite)
    {
        return false;
    }
    // Keep raster cap boundaries independent of the current damage rectangle.
    let top = (bounds.y + radius).ceil().max(0.0);
    let bottom = (bounds.y + bounds.height - radius)
        .floor()
        .min(pixels.height() as f32);
    if ![left, right, top, bottom].into_iter().all(f32::is_finite)
        || left.fract() != 0.0
        || right.fract() != 0.0
        || top < 0.0
        || bottom > pixels.height() as f32
        || bottom <= top
    {
        return false;
    }
    let Some(physical) = path.clone().transform(transform) else {
        return false;
    };
    let width = pixels.width();
    let paint = tiny_skia::Paint {
        shader: tiny_skia::Shader::SolidColor(into_color(color)),
        anti_alias: true,
        ..Default::default()
    };
    let pixel: u32 = bytemuck::cast(into_color(color).to_color_u8().premultiply());
    let top = top as usize;
    let bottom = bottom as usize;
    let stride = width as usize * 4;
    let data = pixels.data_mut();
    let start = left.max(0.0) as usize;
    let end = right.min(width as f32).max(0.0) as usize;
    if start >= end || start > width as usize {
        return false;
    }
    let fill_top = (clip.y.floor().max(top as f32).min(bottom as f32)) as usize;
    let fill_bottom = ((clip.y + clip.height)
        .ceil()
        .max(fill_top as f32)
        .min(bottom as f32)) as usize;
    for (index, row) in data[fill_top * stride..fill_bottom * stride]
        .chunks_exact_mut(stride)
        .enumerate()
    {
        // Engine clip masks are non-antialiased axis-aligned rectangles (0/255).
        let (begin, finish) = if let Some(mask) = mask {
            let line = &mask.data()[(fill_top + index) * width as usize + start
                ..(fill_top + index) * width as usize + end];
            let Some(first) = line.iter().position(|v| *v == 255) else {
                continue;
            };
            let last = line.iter().rposition(|v| *v == 255).unwrap();
            (start + first, start + last + 1)
        } else {
            (start, end)
        };
        let target = &mut row[begin * 4..finish * 4];
        if let Ok(words) = bytemuck::try_cast_slice_mut::<u8, u32>(target) {
            words.fill(pixel);
        } else {
            for bytes in target.chunks_exact_mut(4) {
                bytes.copy_from_slice(&pixel.to_ne_bytes());
            }
        }
    }
    // One even-odd path retains the original curve coordinates while excluding
    // the opaque middle already filled above. No full-canvas mask copies.
    let mut caps = tiny_skia::PathBuilder::new();
    caps.push_path(&physical);
    caps.push_rect(
        tiny_skia::Rect::from_xywh(left, top as f32, right - left, (bottom - top) as f32).unwrap(),
    );
    if let Some(caps) = caps.finish() {
        pixels.fill_path(
            &caps,
            &paint,
            tiny_skia::FillRule::EvenOdd,
            tiny_skia::Transform::identity(),
            mask,
        );
    }
    true
}
#[cfg(test)]
mod renderer_fill_tests {
    use super::*;
    #[test]
    fn opaque_interior_matches_reference_pixels() {
        for scale in [1.0_f32, 1.25, 1.5, 2.0] {
            for y in [3.0_f32, 3.25, 3.5, 3.75] {
                for radius in [0.0_f32, 2.0, 6.0, 15.0] {
                    let bounds = Rectangle {
                        x: 8.0,
                        y,
                        width: 96.0,
                        height: 46.0,
                    };
                    let transform = tiny_skia::Transform::from_scale(scale, scale);
                    let physical = Rectangle {
                        x: 8.0 * scale,
                        y: y * scale,
                        width: 96.0 * scale,
                        height: 46.0 * scale,
                    };
                    let path = rounded_rectangle(bounds, [radius; 4]);
                    let color = Color::from_rgb8(39, 48, 61);
                    let mut actual = tiny_skia::Pixmap::new(240, 160).unwrap();
                    actual.fill(tiny_skia::Color::from_rgba8(15, 22, 31, 255));
                    let mut expected = actual.clone();
                    let fast = fill_solid_interior(
                        &mut actual.as_mut(),
                        &path,
                        transform,
                        physical,
                        radius * scale,
                        color,
                        None,
                        Rectangle {
                            x: 0.0,
                            y: 0.0,
                            width: 240.0,
                            height: 160.0,
                        },
                    );
                    assert!(fast);
                    expected.fill_path(
                        &path,
                        &tiny_skia::Paint {
                            shader: tiny_skia::Shader::SolidColor(into_color(color)),
                            anti_alias: true,
                            ..Default::default()
                        },
                        tiny_skia::FillRule::EvenOdd,
                        transform,
                        None,
                    );
                    assert!(
                        actual.data() == expected.data(),
                        "scale={scale} y={y} radius={radius}"
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod renderer_clipped_fill_tests {
    use super::*;
    #[test]
    fn clipped_opaque_interiors_match_reference() {
        for scale in [1.0_f32, 1.25, 1.5, 2.0] {
            for y in [-25.75_f32, 3.25, 89.875] {
                for clip in [
                    Rectangle {
                        x: 0.0,
                        y: 0.0,
                        width: 240.0,
                        height: 160.0,
                    },
                    Rectangle {
                        x: 12.25,
                        y: 7.75,
                        width: 180.5,
                        height: 133.25,
                    },
                ] {
                    let bounds = Rectangle {
                        x: 8.0,
                        y,
                        width: 96.0,
                        height: 120.0,
                    };
                    let radii = [2.0, 6.0, 15.0, 0.0];
                    let path = rounded_rectangle(bounds, radii);
                    let transform = tiny_skia::Transform::from_scale(scale, scale);
                    let physical = Rectangle {
                        x: 8.0 * scale,
                        y: y * scale,
                        width: 96.0 * scale,
                        height: 120.0 * scale,
                    };
                    let color = Color::from_rgb8(39, 48, 61);
                    let mut actual = tiny_skia::Pixmap::new(240, 160).unwrap();
                    actual.fill(tiny_skia::Color::from_rgba8(15, 22, 31, 255));
                    let mut expected = actual.clone();
                    let mut mask = tiny_skia::Mask::new(240, 160).unwrap();
                    adjust_clip_mask(&mut mask, clip);
                    if fill_solid_interior(
                        &mut actual.as_mut(),
                        &path,
                        transform,
                        physical,
                        15.0 * scale,
                        color,
                        Some(&mask),
                        clip,
                    ) {
                        expected.fill_path(
                            &path,
                            &tiny_skia::Paint {
                                shader: tiny_skia::Shader::SolidColor(into_color(color)),
                                anti_alias: true,
                                ..Default::default()
                            },
                            tiny_skia::FillRule::EvenOdd,
                            transform,
                            Some(&mask),
                        );
                        assert!(
                            actual.data() == expected.data(),
                            "scale={scale} y={y} clip={clip:?} diffs={:?}",
                            actual
                                .data()
                                .iter()
                                .zip(expected.data())
                                .enumerate()
                                .filter(|(_, (a, b))| a != b)
                                .take(12)
                                .map(|(i, (a, b))| (i / 4 % 240, i / 4 / 240, i % 4, *a, *b))
                                .collect::<Vec<_>>()
                        );
                    } else {
                        assert!(actual.data() == expected.data());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod shadow_cache_tests {
    #[test]
    fn straight_shadow_spans_match_per_pixel_sdf() {
        for scale in [1., 1.25, 2.] {
            let quad = Quad {
                bounds: Rectangle {
                    x: 20.25,
                    y: 22.75,
                    width: 170.5,
                    height: 75.5,
                },
                border: crate::core::Border {
                    radius: crate::core::border::Radius {
                        top_left: 3.,
                        top_right: 7.,
                        bottom_right: 11.,
                        bottom_left: 5.,
                    },
                    ..Default::default()
                },
                shadow: crate::core::Shadow {
                    color: Color::from_rgba(0.9, 0.8, 0.7, 0.3),
                    offset: Vector::new(-3., -2.),
                    blur_radius: 9.,
                },
                ..Default::default()
            };
            let mut engine = Engine::new();
            let mut pixels = tiny_skia::Pixmap::new(450, 250).unwrap();
            let mut mask = tiny_skia::Mask::new(450, 250).unwrap();
            engine.draw_quad(
                &quad,
                &Background::Color(Color::TRANSPARENT),
                Transformation::scale(scale),
                &mut pixels.as_mut(),
                &mut mask,
                Rectangle {
                    x: 0.,
                    y: 0.,
                    width: 450.,
                    height: 250.,
                },
            );
            let (key, image) = &engine.shadow_cache[0];
            let f = |i: usize| f32::from_bits(key[i]);
            let size = tiny_skia::Size::from_wh(f(4), f(5)).unwrap();
            let radii = key[14..]
                .iter()
                .map(|v| f32::from_bits(*v))
                .collect::<Vec<_>>();
            for py in 0..image.height() {
                for px in 0..image.width() {
                    let distance = rounded_box_sdf(
                        Vector::new(
                            px as f32 + f(2) - f(10) * scale - f(4),
                            py as f32 + f(3) - f(11) * scale - f(5),
                        ),
                        size,
                        &radii,
                    )
                    .max(0.);
                    let alpha = 1. - smoothstep(-f(12) * scale, f(12) * scale, distance);
                    let mut color = into_color(quad.shadow.color);
                    color.apply_opacity(alpha);
                    assert_eq!(
                        image.pixel(px, py).unwrap(),
                        color.to_color_u8().premultiply(),
                        "scale={scale} x={px} y={py}"
                    );
                }
            }
        }
    }

    use super::*;
    #[test]
    fn shadow_raster_is_reused_across_damage_clips_and_matches_cold_paint() {
        let mut engine = Engine::new();
        let quad = Quad {
            bounds: Rectangle {
                x: 20.,
                y: 20.,
                width: 200.,
                height: 100.,
            },
            border: crate::core::Border {
                radius: 10.0.into(),
                ..Default::default()
            },
            shadow: crate::core::Shadow {
                color: Color::from_rgba(0., 0., 0., 0.6),
                offset: Vector::new(2., 2.),
                blur_radius: 6.,
            },
            ..Default::default()
        };
        let mut pixels = tiny_skia::Pixmap::new(260, 160).unwrap();
        let mut mask = tiny_skia::Mask::new(260, 160).unwrap();
        for clip in [
            Rectangle {
                x: 0.,
                y: 0.,
                width: 260.,
                height: 160.,
            },
            Rectangle {
                x: 15.,
                y: 15.,
                width: 110.,
                height: 115.,
            },
            Rectangle {
                x: 125.,
                y: 15.,
                width: 110.,
                height: 115.,
            },
        ] {
            pixels.fill(tiny_skia::Color::TRANSPARENT);
            engine.draw_quad(
                &quad,
                &Background::Color(Color::WHITE),
                Transformation::IDENTITY,
                &mut pixels.as_mut(),
                &mut mask,
                clip,
            );
            let mut cold = tiny_skia::Pixmap::new(260, 160).unwrap();
            Engine::new().draw_quad(
                &quad,
                &Background::Color(Color::WHITE),
                Transformation::IDENTITY,
                &mut cold.as_mut(),
                &mut mask,
                clip,
            );
            assert_eq!(pixels.data(), cold.data());
            assert_eq!(
                engine.shadow_cache.len(),
                1,
                "damage must reuse a whole shadow raster"
            );
        }
        assert!(engine.shadow_cache_bytes <= 16 * 1024 * 1024);
    }
}

/// Same shader and blending as tiny-skia's draw_pixmap, scissored before shading.
fn draw_shadow_sprite(
    pixels: &mut tiny_skia::PixmapMut<'_>,
    x: u32,
    y: u32,
    source: tiny_skia::PixmapRef<'_>,
    mask: &tiny_skia::Mask,
    clip: Rectangle,
    cover: Option<Rectangle>,
) {
    let sprite = Rectangle {
        x: x as f32,
        y: y as f32,
        width: source.width() as f32,
        height: source.height() as f32,
    };
    let Some(visible) = sprite.intersection(&clip) else {
        return;
    };
    let visible = Rectangle {
        x: visible.x.floor(),
        y: visible.y.floor(),
        width: (visible.x + visible.width).ceil() - visible.x.floor(),
        height: (visible.y + visible.height).ceil() - visible.y.floor(),
    };
    let paint = tiny_skia::Paint {
        shader: tiny_skia::Pattern::new(
            source,
            tiny_skia::SpreadMode::Pad,
            tiny_skia::FilterQuality::Nearest,
            1.,
            tiny_skia::Transform::from_translate(x as f32, y as f32),
        ),
        anti_alias: false,
        ..Default::default()
    };
    let mut draw = |bounds: Rectangle| {
        if let Some(rect) =
            tiny_skia::Rect::from_xywh(bounds.x, bounds.y, bounds.width, bounds.height)
        {
            pixels.fill_rect(rect, &paint, tiny_skia::Transform::identity(), Some(mask));
        }
    };
    if let Some(cover) = cover.and_then(|c| c.intersection(&visible)) {
        // All skipped pixels are overwritten by an opaque fill later in this batch.
        draw(Rectangle {
            height: cover.y - visible.y,
            ..visible
        });
        draw(Rectangle {
            y: cover.y + cover.height,
            height: visible.y + visible.height - cover.y - cover.height,
            ..visible
        });
        draw(Rectangle {
            y: cover.y,
            width: cover.x - visible.x,
            height: cover.height,
            ..visible
        });
        draw(Rectangle {
            x: cover.x + cover.width,
            y: cover.y,
            width: visible.x + visible.width - cover.x - cover.width,
            height: cover.height,
        });
    } else {
        draw(visible);
    }
}
/// A conservative integer rectangle fully covered by this quad's opaque fill.
pub(crate) fn opaque_interior(
    quad: &Quad,
    background: &Background,
    transformation: Transformation,
) -> Option<Rectangle> {
    if !matches!(background, Background::Color(c) if c.a==1.) {
        return None;
    }
    let b = quad.bounds * transformation;
    let radius = <[f32; 4]>::from(quad.border.radius)
        .into_iter()
        .fold(0., f32::max)
        .min(quad.bounds.width / 2.)
        .min(quad.bounds.height / 2.)
        * transformation.scale_factor();
    let x = (b.x + radius + 1.).ceil();
    let y = (b.y + radius + 1.).ceil();
    let right = (b.x + b.width - radius - 1.).floor();
    let bottom = (b.y + b.height - radius - 1.).floor();
    (right > x && bottom > y).then_some(Rectangle {
        x,
        y,
        width: right - x,
        height: bottom - y,
    })
}

#[cfg(test)]
mod shadow_scissor_tests {
    use super::*;
    #[test]
    fn scissored_shadow_sprite_matches_native_blending() {
        let mut source = tiny_skia::Pixmap::new(80, 60).unwrap();
        for (i, p) in source.pixels_mut().iter_mut().enumerate() {
            *p = tiny_skia::Color::from_rgba8((i % 255) as u8, 64, 128, (i % 256) as u8)
                .to_color_u8()
                .premultiply();
        }
        for clip in [
            Rectangle {
                x: 0.,
                y: 0.,
                width: 140.,
                height: 100.,
            },
            Rectangle {
                x: 27.25,
                y: 18.75,
                width: 32.5,
                height: 49.25,
            },
            Rectangle {
                x: 100.,
                y: 90.,
                width: 20.,
                height: 5.,
            },
        ] {
            let mut actual = tiny_skia::Pixmap::new(140, 100).unwrap();
            actual.fill(tiny_skia::Color::from_rgba8(37, 48, 63, 255));
            let mut expected = actual.clone();
            let mut mask = tiny_skia::Mask::new(140, 100).unwrap();
            adjust_clip_mask(&mut mask, clip);
            draw_shadow_sprite(
                &mut actual.as_mut(),
                20,
                15,
                source.as_ref(),
                &mask,
                clip,
                None,
            );
            expected.draw_pixmap(
                20,
                15,
                source.as_ref(),
                &tiny_skia::PixmapPaint::default(),
                tiny_skia::Transform::identity(),
                Some(&mask),
            );
            assert_eq!(actual.data(), expected.data(), "clip={clip:?}");
            let cover = Rectangle {
                x: 36.,
                y: 28.,
                width: 40.,
                height: 24.,
            };
            actual.fill(tiny_skia::Color::from_rgba8(37, 48, 63, 255));
            expected = actual.clone();
            draw_shadow_sprite(
                &mut actual.as_mut(),
                20,
                15,
                source.as_ref(),
                &mask,
                clip,
                Some(cover),
            );
            expected.draw_pixmap(
                20,
                15,
                source.as_ref(),
                &tiny_skia::PixmapPaint::default(),
                tiny_skia::Transform::identity(),
                Some(&mask),
            );
            let rect =
                tiny_skia::Rect::from_xywh(cover.x, cover.y, cover.width, cover.height).unwrap();
            let paint = tiny_skia::Paint {
                shader: tiny_skia::Shader::SolidColor(tiny_skia::Color::from_rgba8(
                    80, 100, 120, 255,
                )),
                anti_alias: false,
                ..Default::default()
            };
            actual.fill_rect(rect, &paint, tiny_skia::Transform::identity(), Some(&mask));
            expected.fill_rect(rect, &paint, tiny_skia::Transform::identity(), Some(&mask));
            assert_eq!(actual.data(), expected.data(), "opaque cover clip={clip:?}");
        }
    }
}

fn background_shader(background: &Background, bounds: Rectangle) -> tiny_skia::Shader<'static> {
    match background {
        Background::Color(color) => tiny_skia::Shader::SolidColor(into_color(*color)),
        Background::Gradient(Gradient::Linear(linear)) => {
            let (start, end) = linear.angle.to_distance(&bounds);

            let stops: Vec<tiny_skia::GradientStop> = linear
                .stops
                .into_iter()
                .flatten()
                .map(|stop| {
                    tiny_skia::GradientStop::new(
                        stop.offset,
                        tiny_skia::Color::from_rgba(
                            stop.color.b,
                            stop.color.g,
                            stop.color.r,
                            stop.color.a,
                        )
                        .expect("Create color"),
                    )
                })
                .collect();

            tiny_skia::LinearGradient::new(
                tiny_skia::Point {
                    x: start.x,
                    y: start.y,
                },
                tiny_skia::Point { x: end.x, y: end.y },
                if stops.is_empty() {
                    vec![tiny_skia::GradientStop::new(0.0, tiny_skia::Color::BLACK)]
                } else {
                    stops
                },
                tiny_skia::SpreadMode::Pad,
                tiny_skia::Transform::identity(),
            )
            .expect("Create linear gradient")
        }
    }
}

#[cfg(test)]
mod gradient_cache_tests {
    use super::*;
    #[test]
    fn cached_large_gradient_matches_native_with_bounded_quantization() {
        use crate::core::{Radians, gradient};
        let background = Background::Gradient(
            gradient::Linear::new(Radians(0.8))
                .add_stop(0., Color::from_rgba(0.9, 0.95, 1., 0.13))
                .add_stop(1., Color::from_rgba(0.12, 0.2, 0.3, 0.46))
                .into(),
        );
        for scale in [1., 1.25, 2.] {
            let quad = Quad {
                bounds: Rectangle {
                    x: 10.25,
                    y: 12.75,
                    width: 230.5,
                    height: 160.5,
                },
                border: crate::core::Border {
                    radius: 10.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut cached = Engine::new();
            let mut native = Engine::new();
            native.disable_gradient_cache = true;
            let mut a = tiny_skia::Pixmap::new(520, 380).unwrap();
            a.fill(tiny_skia::Color::from_rgba8(24, 37, 51, 255));
            let mut b = a.clone();
            let mut mask = tiny_skia::Mask::new(520, 380).unwrap();
            let clip = Rectangle {
                x: 0.,
                y: 0.,
                width: 520.,
                height: 380.,
            };
            let mut warmup = a.clone();
            cached.draw_quad(
                &quad,
                &background,
                Transformation::scale(scale),
                &mut warmup.as_mut(),
                &mut mask,
                clip,
            );
            cached.draw_quad(
                &quad,
                &background,
                Transformation::scale(scale),
                &mut a.as_mut(),
                &mut mask,
                clip,
            );
            native.draw_quad(
                &quad,
                &background,
                Transformation::scale(scale),
                &mut b.as_mut(),
                &mut mask,
                clip,
            );
            let delta = a
                .data()
                .iter()
                .zip(b.data())
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap();
            assert!(delta <= 1, "scale={scale} delta={delta}");
            assert_eq!(cached.gradient_cache.len(), 1);
        }
    }
}

#[cfg(test)]
mod border_cache_tests {
    use super::*;
    #[test]
    fn cached_inset_border_matches_native_pixels() {
        for scale in [1., 1.25, 2.] {
            let quad = Quad {
                bounds: Rectangle {
                    x: 10.25,
                    y: 12.75,
                    width: 350.5,
                    height: 30.5,
                },
                border: crate::core::Border {
                    radius: 7.0.into(),
                    width: 2.5,
                    color: Color::from_rgba(0.1, 0.2, 0.3, 0.23),
                },
                ..Default::default()
            };
            let background = Background::Color(Color::TRANSPARENT);
            let mut cached = Engine::new();
            let mut native = Engine::new();
            native.disable_gradient_cache = true;
            let mut a = tiny_skia::Pixmap::new(800, 200).unwrap();
            a.fill(tiny_skia::Color::from_rgba8(37, 48, 63, 255));
            let mut b = a.clone();
            let mut mask = tiny_skia::Mask::new(800, 200).unwrap();
            let clip = Rectangle {
                x: 0.,
                y: 0.,
                width: 800.,
                height: 200.,
            };
            let mut warmup = a.clone();
            cached.draw_quad(
                &quad,
                &background,
                Transformation::scale(scale),
                &mut warmup.as_mut(),
                &mut mask,
                clip,
            );
            cached.draw_quad(
                &quad,
                &background,
                Transformation::scale(scale),
                &mut a.as_mut(),
                &mut mask,
                clip,
            );
            native.draw_quad(
                &quad,
                &background,
                Transformation::scale(scale),
                &mut b.as_mut(),
                &mut mask,
                clip,
            );
            let delta = a
                .data()
                .iter()
                .zip(b.data())
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap();
            assert!(delta <= 1, "scale={scale} delta={delta}");
        }
    }
}

#[cfg(test)]
mod cached_text_clip_tests {
    use super::*;
    #[test]
    fn cached_labels_never_paint_outside_the_scroll_viewport() {
        let mut engine = Engine::new();
        let mut pixels = tiny_skia::Pixmap::new(120, 70).unwrap();
        let mut mask = tiny_skia::Mask::new(120, 70).unwrap();
        let viewport = Rectangle { x: 4., y: 4., width: 90., height: 12. };
        let label = Text::Cached {
            content: "MMMMMMMM".into(),
            bounds: Rectangle { x: 5., y: 5., width: 110., height: 30. },
            color: Color::WHITE,
            size: 24.0.into(),
            line_height: 30.0.into(),
            font: crate::core::Font::DEFAULT,
            align_x: crate::core::alignment::Horizontal::Left.into(),
            align_y: crate::core::alignment::Vertical::Top,
            shaping: crate::core::text::Shaping::Basic,
            clip_bounds: viewport,
        };
        engine.draw_text(&label, Transformation::IDENTITY, &mut pixels.as_mut(), &mut mask,
            Rectangle { x: 0., y: 0., width: 120., height: 70. });
        assert!(pixels.pixels().iter().any(|p| p.alpha() > 0));
        for y in 0..70 { for x in 0..120 {
            if x < 4 || x >= 94 || y < 4 || y >= 16 {
                assert_eq!(pixels.pixel(x, y).unwrap().alpha(), 0, "outside viewport at {x},{y}");
            }
        }}
    }
}
