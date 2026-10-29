//! Shared anchored Settings shell; app-specific settings stay in this project.
use crate::Message;
use iced::Element;
pub fn settings_popup<'a>(
    base: impl Into<Element<'a, Message>>,
    popup: impl Into<Element<'a, Message>>,
    open: bool,
    policy: crate::motion::Policy,
) -> Element<'a, Message> {
    sk_iced::components::Popup::new(base, popup)
        .max_width(580.)
        .open(open)
        .policy(policy)
        .on_dismiss(Message::CloseSettings)
        .into()
}
