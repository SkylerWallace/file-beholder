//! Opt-in release-mode CPU scrolling measurements; no disk scans or live-window timing.
use crate::*;
use iced::advanced::{
    Layout, Renderer as _, Shell, layout, mouse, renderer,
    widget::{Operation, Tree},
};
use iced_tiny_skia::{
    Layer,
    graphics::{Viewport, damage},
};
use std::time::Instant;
#[derive(Default)]
struct Lists(Vec<(usize, iced::Rectangle)>);
impl Operation for Lists {
    fn traverse(&mut self, f: &mut dyn FnMut(&mut dyn Operation)) {
        f(self);
    }
    fn custom(
        &mut self,
        _: Option<&iced::widget::Id>,
        b: iced::Rectangle,
        state: &mut dyn std::any::Any,
    ) {
        if let Some(m) = state.downcast_ref::<crate::tab_drag::Marker>() {
            if m.kind == 3 {
                self.0.push((m.pane, b));
            }
        }
    }
}
fn quantile(v: &[f64], q: f64) -> f64 {
    let mut v = v.to_vec();
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * q).round() as usize]
}
#[test]
#[ignore = "opt-in release-mode benchmark"]
fn scroll_cpu_benchmark() {
    for dark in [true, false] {
        for neo in [false, true] {
            for drag in [false, true] {
                let mut app = App::new("benchmark-left".into(), "benchmark-right".into(), vec![]);
                app.preferences.theme = if dark {
                    preferences::ThemeMode::Dark
                } else {
                    preferences::ThemeMode::Light
                };
                app.preferences.look = if neo {
                    preferences::Look::Neumorphic
                } else {
                    preferences::Look::Default
                };
                app.motion = crate::motion::Policy::DISABLED;
                let _ = app.apply_appearance();
                app.motion = crate::motion::Policy::DISABLED;
                for p in &mut app.panes {
                    p.entries = (0..2000)
                        .map(|i| Entry {
                            name: format!("Document {i:05}.txt"),
                            path: format!("document-{i:05}.txt").into(),
                            directory: false,
                            hidden: false,
                            bytes: Some(8192 + i * 37),
                            partial: false,
                            modified: None,
                            modified_label: "2026-10-26 12:30".into(),
                            icon: None,
                        })
                        .collect();
                    p.total_bytes = p.entries.iter().filter_map(|e| e.bytes).sum();
                    p.reorder();
                }
                let bounds = iced::Rectangle::with_size(iced::Size::new(1400., 900.));
                let viewport = Viewport::with_physical_size(iced::Size::new(1400, 900), 1.);
                let mut renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
                let mut pixels = tiny_skia::Pixmap::new(1400, 900).unwrap();
                let mut mask = tiny_skia::Mask::new(1400, 900).unwrap();
                let mut view = app.view();
                let mut tree = Tree::new(&view);
                let limits = layout::Limits::new(bounds.size(), bounds.size());
                let mut node = view.as_widget_mut().layout(&mut tree, &renderer, &limits);
                let mut lists = Lists::default();
                view.as_widget_mut()
                    .operate(&mut tree, Layout::new(&node), &renderer, &mut lists);
                let b = lists
                    .0
                    .iter()
                    .find(|(id, _)| *id == 0)
                    .expect("file list")
                    .1;
                let rail = iced::Point::new((b.x + b.width - 7.).min(1382.), b.y + 7.);
                let mut previous: Option<Vec<Layer>> = None;
                let mut samples: Vec<[f64; 5]> = vec![];
                let mut changed = 0;
                for frame in 0..100 {
                    let present = iced_tiny_skia::profile::present_start();
                    iced_tiny_skia::profile::input(match (dark, neo, drag) {
                        (true, false, false) => "default-dark-wheel",
                        (true, false, true) => "default-dark-drag",
                        (true, true, false) => "neo-dark-wheel",
                        (true, true, true) => "neo-dark-drag",
                        (false, false, false) => "default-light-wheel",
                        (false, false, true) => "default-light-drag",
                        (false, true, false) => "neo-light-wheel",
                        (false, true, true) => "neo-light-drag",
                    });
                    let start = Instant::now();
                    let point = if drag {
                        iced::Point::new(
                            rail.x,
                            rail.y + ((frame % 60) as f32 / 60.) * (b.height - 40.),
                        )
                    } else {
                        b.center()
                    };
                    let cursor = mouse::Cursor::Available(point);
                    let event = if drag {
                        if frame == 0 {
                            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                                iced::mouse::Button::Left,
                            ))
                        } else {
                            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position: point })
                        }
                    } else {
                        iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
                            delta: iced::mouse::ScrollDelta::Lines {
                                x: 0.,
                                y: if frame < 60 { -1. } else { 1. },
                            },
                        })
                    };
                    let mut messages = vec![];
                    let invalid = {
                        let mut shell = Shell::new(&mut messages);
                        view.as_widget_mut().update(
                            &mut tree,
                            &event,
                            Layout::new(&node),
                            cursor,
                            &renderer,
                            &mut iced::advanced::clipboard::Null,
                            &mut shell,
                            &bounds,
                        );
                        shell.is_layout_invalid()
                    };
                    if invalid {
                        node = view.as_widget_mut().layout(&mut tree, &renderer, &limits);
                    }
                    view.as_widget_mut().update(
                        &mut tree,
                        &iced::Event::Window(iced::window::Event::RedrawRequested(
                            iced::time::Instant::now(),
                        )),
                        Layout::new(&node),
                        cursor,
                        &renderer,
                        &mut iced::advanced::clipboard::Null,
                        &mut Shell::new(&mut messages),
                        &bounds,
                    );
                    let events = start.elapsed().as_secs_f64() * 1000.;
                    let at = Instant::now();
                    renderer.reset(bounds);
                    view.as_widget().draw(
                        &tree,
                        &mut renderer,
                        &app.app_theme,
                        &renderer::Style::default(),
                        Layout::new(&node),
                        cursor,
                        &bounds,
                    );
                    let record = at.elapsed().as_secs_f64() * 1000.;
                    let at = Instant::now();
                    let regions = previous
                        .as_ref()
                        .map(|old| {
                            damage::diff(old, renderer.layers(), |l| vec![l.bounds], Layer::damage)
                        })
                        .unwrap_or_else(|| vec![bounds]);
                    let regions = iced_tiny_skia::window::compositor::coalesce_damage(
                        damage::group(regions, bounds),
                    );
                    let current = renderer.layers().to_vec();
                    let damage_ms = at.elapsed().as_secs_f64() * 1000.;
                    if !regions.is_empty() {
                        changed += 1;
                    }
                    let at = Instant::now();
                    renderer.draw(
                        &mut pixels.as_mut(),
                        &mut mask,
                        &viewport,
                        &regions,
                        iced::Color::BLACK,
                    );
                    let raster = at.elapsed().as_secs_f64() * 1000.;
                    previous = Some(current);
                    if frame >= 20 {
                        samples.push([
                            start.elapsed().as_secs_f64() * 1000.,
                            events,
                            record,
                            damage_ms,
                            raster,
                        ]);
                    }
                    iced_tiny_skia::profile::present_end(present, &viewport, true);
                }
                assert!(
                    changed > 80,
                    "must actually scroll, {changed} changing frames; list={b:?} rail={rail:?}"
                );
                let totals = samples.iter().map(|s| s[0]).collect::<Vec<_>>();
                let means = (1..5)
                    .map(|i| samples.iter().map(|s| s[i]).sum::<f64>() / samples.len() as f64)
                    .collect::<Vec<_>>();
                println!(
                    r#"SCROLL {{"dark":{dark},"neo":{neo},"drag":{drag},"p50_ms":{:.3},"p95_ms":{:.3},"event_layout_ms":{:.3},"record_ms":{:.3},"damage_ms":{:.3},"raster_ms":{:.3},"changed":{changed}}}"#,
                    quantile(&totals, 0.5),
                    quantile(&totals, 0.95),
                    means[0],
                    means[1],
                    means[2],
                    means[3]
                );
            }
        }
    }
}

#[test]
#[ignore = "opt-in release-mode resize benchmark"]
fn resize_cpu_benchmark() {
    for neo in [false, true] {
        let mut app = App::new("left".into(), "right".into(), vec![]);
        app.preferences.look = if neo {
            preferences::Look::Neumorphic
        } else {
            preferences::Look::Default
        };
        app.preferences.theme = preferences::ThemeMode::Dark;
        let _ = app.apply_appearance();
        app.motion = crate::motion::Policy::DISABLED;
        for p in &mut app.panes {
            p.entries = (0..500)
                .map(|i| Entry {
                    name: format!("Document {i:05}.txt"),
                    path: format!("document-{i}.txt").into(),
                    directory: false,
                    hidden: false,
                    bytes: Some(1024 + i),
                    partial: false,
                    modified: None,
                    modified_label: "2026-10-26 12:30".into(),
                    icon: None,
                })
                .collect();
            p.reorder();
        }
        let mut renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let mut tree = Tree::empty();
        let mut samples = vec![];
        for frame in 0..60 {
            let width = 1400 - ((frame % 30) as u32) * 14;
            let bounds = iced::Rectangle::with_size(iced::Size::new(width as f32, 900.));
            let _ = app.update(Message::SidebarWindowResized(bounds.size()));
            let start = Instant::now();
            let mut pixels = tiny_skia::Pixmap::new(width, 900).unwrap();
            let mut mask = tiny_skia::Mask::new(width, 900).unwrap();
            let viewport = Viewport::with_physical_size(iced::Size::new(width, 900), 1.);
            let mut view = app.view();
            tree.diff(&view);
            let node = view.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(bounds.size(), bounds.size()),
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
            renderer.draw(
                &mut pixels.as_mut(),
                &mut mask,
                &viewport,
                &[bounds],
                iced::Color::BLACK,
            );
            if frame >= 10 {
                samples.push(start.elapsed().as_secs_f64() * 1000.);
            }
        }
        println!(
            "RESIZE neo={neo} p50_ms={:.3} p95_ms={:.3}",
            quantile(&samples, 0.5),
            quantile(&samples, 0.95)
        );
    }
}
