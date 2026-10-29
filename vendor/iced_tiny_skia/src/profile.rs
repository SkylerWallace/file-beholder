// SPDX-License-Identifier: MIT
//! Opt-in local presentation measurements; no network or UI changes.
use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    sync::{Mutex, OnceLock},
    time::Instant,
};
#[derive(Default)]
struct Frame {
    draw_ms: f64,
    fills: u64,
    skips: u64,
    damage: usize,
    stages: [f64; 6],
}
struct Trace {
    file: BufWriter<File>,
    epoch: Instant,
    frame: Frame,
    last_input: Option<Instant>,
    input_seq: u64,
    input_kind: &'static str,
    foreground: bool,
    uncached: bool,
}
static TRACE: OnceLock<Option<Mutex<Trace>>> = OnceLock::new();
fn trace() -> Option<&'static Mutex<Trace>> {
    TRACE
        .get_or_init(|| {
            let path = std::env::var_os("FILE_BEHOLDER_PRESENT_TRACE")?;
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .ok()?;
            Some(Mutex::new(Trace {
                file: BufWriter::new(file),
                epoch: Instant::now(),
                frame: Frame::default(),
                last_input: None,
                input_seq: 0,
                input_kind: "none",
                foreground: false,
                uncached: std::env::var_os("FILE_BEHOLDER_UNCACHED_CLIP_MASK").is_some(),
            }))
        })
        .as_ref()
}
pub fn enabled() -> bool {
    trace().is_some()
}
pub fn uncached() -> bool {
    trace().is_some_and(|t| t.lock().map(|t| t.uncached).unwrap_or(false))
}
pub fn input(kind: &'static str) {
    if let Some(t) = trace() {
        if let Ok(mut t) = t.lock() {
            t.input_seq += 1;
            t.input_kind = kind;
            t.last_input = Some(Instant::now());
        }
    }
}
pub fn draw_start() -> Option<Instant> {
    enabled().then(Instant::now)
}
pub fn draw_end(at: Option<Instant>, (fills, skips): (u64, u64), damage: usize) {
    if let (Some(at), Some(t)) = (at, trace()) {
        if let Ok(mut t) = t.lock() {
            t.frame.draw_ms = at.elapsed().as_secs_f64() * 1000.0;
            t.frame.fills = fills;
            t.frame.skips = skips;
            t.frame.damage = damage;
        }
    }
}
pub fn present_start() -> Option<Instant> {
    if let Some(t) = trace() {
        if let Ok(mut t) = t.lock() {
            t.frame = Frame::default();
        }
        Some(Instant::now())
    } else {
        None
    }
}
pub fn present_end(at: Option<Instant>, viewport: &crate::graphics::Viewport, ok: bool) {
    if let (Some(at), Some(t)) = (at, trace()) {
        if let Ok(mut t) = t.lock() {
            let ended = Instant::now();
            let present_ms = ended.duration_since(at).as_secs_f64() * 1000.0;
            let elapsed_ms = ended.duration_since(t.epoch).as_secs_f64() * 1000.0;
            let input_age_ms = t
                .last_input
                .map(|i| ended.duration_since(i).as_secs_f64() * 1000.0)
                .unwrap_or(0.0);
            let f = &t.frame;
            let seq = t.input_seq;
            let kind = t.input_kind;
            let [quads, primitives, images, text, fill, border] = t.frame.stages;
            let line = format!(
                "{{\"elapsed_ms\":{elapsed_ms},\"fill_ms\":{fill},\"border_ms\":{border},\"quads_ms\":{quads},\"primitives_ms\":{primitives},\"images_ms\":{images},\"text_ms\":{text},\"present_ms\":{present_ms},\"raster_ms\":{},\"clip_fills\":{},\"clip_skips\":{},\"damage_regions\":{},\"width\":{},\"height\":{},\"scale\":{},\"input_seq\":{seq},\"input_kind\":\"{kind}\",\"input_age_ms\":{input_age_ms},\"ok\":{ok}}}\n",
                f.draw_ms,
                f.fills,
                f.skips,
                f.damage,
                viewport.physical_size().width,
                viewport.physical_size().height,
                viewport.scale_factor()
            );
            let _ = t.file.write_all(line.as_bytes());
            let _ = t.file.flush();
        }
    }
}

pub fn focus(value: bool) {
    if let Some(t) = trace() {
        if let Ok(mut t) = t.lock() {
            t.foreground = value;
        }
    }
}
pub fn foreground() -> bool {
    trace().is_some_and(|t| t.lock().map(|t| t.foreground).unwrap_or(false))
}

pub struct Stage(Option<(Instant, usize)>);
pub fn stage(name: &str) -> Stage {
    Stage(if enabled() {
        Some((
            Instant::now(),
            match name {
                "quads" => 0,
                "primitives" => 1,
                "images" => 2,
                "fill" => 4,
                "border" => 5,
                _ => 3,
            },
        ))
    } else {
        None
    })
}
impl Drop for Stage {
    fn drop(&mut self) {
        if let Some((at, index)) = self.0 {
            let ms = at.elapsed().as_secs_f64() * 1000.0;
            if let Some(t) = trace() {
                if let Ok(mut t) = t.lock() {
                    t.frame.stages[index] += ms;
                }
            }
        }
    }
}

pub fn reference_raster() -> bool {
    static FLAG: OnceLock<bool> = OnceLock::new();
    *FLAG.get_or_init(|| enabled() && std::env::var_os("FILE_BEHOLDER_REFERENCE_RASTER").is_some())
}
