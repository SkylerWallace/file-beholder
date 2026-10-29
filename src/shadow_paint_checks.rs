//! Retained framebuffer regressions for shadow clipping and damage expansion.
use iced::advanced::Renderer as _;
use iced::{Color, Rectangle, Size, Vector};
use iced_tiny_skia::{
    Layer,
    graphics::{Viewport, damage},
};

#[test]
fn directory_size_batches_do_not_accumulate_neumorphic_layers() {
    use crate::*;
    use iced::advanced::{Layout, Shell, layout, mouse, renderer, widget::Tree};
    let mut app = App::new("left".into(), "right".into(), vec![]);
    app.preferences.look = crate::preferences::Look::Neumorphic;
    let _ = app.apply_appearance();
    for pane in &mut app.panes {
        pane.entries = (0..16)
            .map(|i| Entry {
                name: format!("Directory {i:02}"),
                path: format!("folder-{i}").into(),
                directory: true,
                hidden: false,
                bytes: None,
                partial: true,
                modified: None,
                modified_label: String::new(),
                icon: None,
            })
            .collect();
        pane.reorder();
    }
    let bounds = Rectangle::with_size(Size::new(1000., 640.));
    let viewport = Viewport::with_physical_size(Size::new(1000, 640), 1.);
    let mut renderer = iced::Renderer::new(crate::typography::font(), 13.0.into());
    let mut pixels = tiny_skia::Pixmap::new(1000, 640).unwrap();
    let mut mask = tiny_skia::Mask::new(1000, 640).unwrap();
    let mut previous: Option<Vec<Layer>> = None;
    let mut tree = Tree::empty();
    for batch in 0..12 {
        for pane in &mut app.panes {
            for (index, entry) in pane.entries.iter_mut().enumerate() {
                entry.bytes = Some((batch + 1) * 1024 * (index as u64 + 1));
            }
            pane.total_bytes = pane.entries.iter().filter_map(|e| e.bytes).sum();
            pane.view_identity = pane.view_identity.wrapping_add(1);
        }
        let mut view = app.view();
        tree.diff(&view);
        let node = view.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(bounds.size(), bounds.size()),
        );
        view.as_widget_mut().update(
            &mut tree,
            &iced::Event::Window(iced::window::Event::RedrawRequested(
                iced::time::Instant::now(),
            )),
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &renderer,
            &mut iced::advanced::clipboard::Null,
            &mut Shell::new(&mut vec![]),
            &bounds,
        );
        renderer.reset(bounds);
        view.as_widget().draw(
            &tree,
            &mut renderer,
            &app.app_theme,
            &renderer::Style::default(),
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &bounds,
        );
        let regions = previous
            .as_ref()
            .map(|old| damage::diff(old, renderer.layers(), |l| vec![l.bounds], Layer::damage))
            .unwrap_or_else(|| vec![bounds]);
        let regions =
            iced_tiny_skia::window::compositor::coalesce_damage(damage::group(regions, bounds));
        let current = renderer.layers().to_vec();
        renderer.draw(
            &mut pixels.as_mut(),
            &mut mask,
            &viewport,
            &regions,
            Color::BLACK,
        );
        let mut clean = tiny_skia::Pixmap::new(1000, 640).unwrap();
        let mut clean_mask = tiny_skia::Mask::new(1000, 640).unwrap();
        renderer.draw(
            &mut clean.as_mut(),
            &mut clean_mask,
            &viewport,
            &[bounds],
            Color::BLACK,
        );
        // Native clipped AA/gradient rasterization can round a boundary channel by
        // one unit. Bound that error on every batch; repeated blending grows well beyond it.
        let maximum_delta = pixels
            .data()
            .iter()
            .zip(clean.data())
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        if maximum_delta > 1 {
            pixels
                .save_png("target/composition/size-batch-retained.png")
                .unwrap();
            clean
                .save_png("target/composition/size-batch-clean.png")
                .unwrap();
        }
        assert!(
            maximum_delta <= 1,
            "directory size batch {batch} accumulated paint: maximum channel delta {maximum_delta}"
        );
        previous = Some(current);
    }
}

#[test]
fn repeated_partial_updates_and_shadow_moves_match_fresh_frames() {
    for scale in [0.75, 1.0, 1.25, 2.0] {
        let size = Size::new(360., 180.);
        let bounds = Rectangle::with_size(size);
        let physical = Size::new((size.width * scale) as u32, (size.height * scale) as u32);
        let viewport = Viewport::with_physical_size(physical, scale);
        let mut renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let mut retained = tiny_skia::Pixmap::new(physical.width, physical.height).unwrap();
        let mut mask = tiny_skia::Mask::new(physical.width, physical.height).unwrap();
        let mut previous: Option<Vec<Layer>> = None;
        for frame in 0..24 {
            renderer.reset(bounds);
            renderer.fill_quad(
                iced::advanced::renderer::Quad {
                    bounds,
                    ..Default::default()
                },
                Color::from_rgb8(44, 49, 58),
            );
            renderer.with_layer(
                Rectangle {
                    x: 8.,
                    y: 8.,
                    width: 320.,
                    height: 152.,
                },
                |renderer| {
                    let x = if frame < 12 {
                        12.
                    } else {
                        22. + (frame % 4) as f32 * 4.
                    };
                    renderer.fill_quad(
                        iced::advanced::renderer::Quad {
                            bounds: Rectangle {
                                x,
                                y: 12.,
                                width: 280.,
                                height: 136.,
                            },
                            border: iced::Border {
                                radius: 9.0.into(),
                                ..Default::default()
                            },
                            shadow: iced::Shadow {
                                color: Color::from_rgba(
                                    0.8,
                                    0.85,
                                    0.95,
                                    if frame >= 20 { 0. } else { 0.4 },
                                ),
                                offset: Vector::new(-4., -3.),
                                blur_radius: 9.,
                            },
                            ..Default::default()
                        },
                        Color::from_rgba(0.2, 0.23, 0.28, 0.9),
                    );
                    renderer.fill_quad(
                        iced::advanced::renderer::Quad {
                            bounds: Rectangle {
                                x: 50.,
                                y: 65.,
                                width: 20. + (frame % 6) as f32 * 3.,
                                height: 12.,
                            },
                            ..Default::default()
                        },
                        Color::from_rgb8(90, 125, 170),
                    );
                },
            );
            let regions = previous
                .as_ref()
                .map(|old| {
                    damage::diff(
                        old,
                        renderer.layers(),
                        |layer| vec![layer.bounds],
                        Layer::damage,
                    )
                })
                .unwrap_or_else(|| vec![bounds]);
            let regions =
                iced_tiny_skia::window::compositor::coalesce_damage(damage::group(regions, bounds));
            let current = renderer.layers().to_vec();
            renderer.draw(
                &mut retained.as_mut(),
                &mut mask,
                &viewport,
                &regions,
                Color::BLACK,
            );
            let mut clean = tiny_skia::Pixmap::new(physical.width, physical.height).unwrap();
            let mut clean_mask = tiny_skia::Mask::new(physical.width, physical.height).unwrap();
            renderer.draw(
                &mut clean.as_mut(),
                &mut clean_mask,
                &viewport,
                &[bounds],
                Color::BLACK,
            );
            let differences = retained
                .data()
                .iter()
                .zip(clean.data())
                .filter(|(a, b)| a != b)
                .count();
            assert_eq!(
                differences, 0,
                "accumulated shadow paint: frame {frame}, scale {scale}"
            );
            previous = Some(current);
        }
    }
}
