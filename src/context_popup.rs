//! Shared pointer-positioned menu shell.
use crate::Message;
use iced::{Element, Point};
pub fn context_popup<'a>(
    base: impl Into<Element<'a, Message>>,
    popup: impl Into<Element<'a, Message>>,
    position: Option<Point>,
) -> Element<'a, Message> {
    sk_iced::components::Popup::new(base, popup)
        .open(position.is_some())
        .at(position.unwrap_or(Point::ORIGIN))
        .max_width(f32::MAX)
        .capture_outside(true)
        .dismiss_any_button(true)
        .on_dismiss(Message::CloseContext)
        .into()
}
