//! File Beholder row sizing and profiling adapter for the shared viewport.
use iced::{Element, Event};
pub const ROW_HEIGHT: f32 = sk_iced::components::virtual_list::ROW_HEIGHT;
fn appearance() -> sk_iced::components::Appearance {
    sk_iced::components::Appearance::themed(crate::typography::font(), crate::palette::from_theme)
}
fn profile(event: &Event) {
    iced_tiny_skia::profile::input(
        if matches!(
            event,
            Event::Mouse(iced::mouse::Event::WheelScrolled { .. })
        ) {
            "wheel"
        } else {
            "thumb"
        },
    );
}
#[cfg(test)]
pub fn file_list<'a, M: 'a>(
    len: usize,
    identity: u64,
    row: impl Fn(usize) -> Element<'a, M> + 'a,
    on_double: impl Fn(usize) -> M + 'a,
) -> Element<'a, M> {
    file_list_sized(len, identity, ROW_HEIGHT, row, on_double)
}
pub fn file_list_sized<'a, M: 'a>(
    len: usize,
    identity: u64,
    height: f32,
    row: impl Fn(usize) -> Element<'a, M> + 'a,
    on_double: impl Fn(usize) -> M + 'a,
) -> Element<'a, M> {
    file_list_revealed(len, identity, height, None, row, on_double)
}
pub fn file_list_revealed<'a, M: 'a>(
    len: usize,
    identity: u64,
    height: f32,
    reveal: Option<(u64, usize)>,
    row: impl Fn(usize) -> Element<'a, M> + 'a,
    on_double: impl Fn(usize) -> M + 'a,
) -> Element<'a, M> {
    sk_iced::components::VirtualList::new(appearance(), len, identity, row)
        .row_height(height)
        .reveal(reveal)
        .on_double_click(on_double)
        .event_hook(profile)
        .build()
}
