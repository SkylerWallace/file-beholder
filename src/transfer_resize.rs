//! App-owned transfer dimensions/messages; shared resizing and paint clipping.
use crate::Message;
use iced::Element;
#[cfg(test)]
use iced::advanced::{Layout, Shell, layout, mouse, widget::Tree};
#[cfg(test)]
use iced::{Event, Rectangle, Renderer, Size};
pub fn handle(height: f32, maximum: f32) -> Element<'static, Message> {
    sk_iced::components::ResizeGrip::new(
        sk_iced::components::Appearance::themed(
            crate::typography::font(),
            crate::palette::from_theme,
        ),
        height,
    )
    .limits(60.0, maximum)
    .on_resize(Message::ResizeTransfers)
    .build()
}
pub fn filled_text<'a>(content: Element<'a, Message>, percent: f32) -> Element<'a, Message> {
    sk_iced::components::FillClip::new(content, percent).build()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idle_grip_has_no_line_and_hover_keeps_resize_feedback() {
        let app = crate::App::new("home".into(), "other".into(), vec![]);
        let size = Size::new(300, 6);
        let idle = crate::final_ui_checks::render_view(handle(150.0, 400.0), &app, size, None);
        let hover = crate::final_ui_checks::render_view(
            handle(150.0, 400.0),
            &app,
            size,
            Some(iced::Point::new(100.0, 3.0)),
        );
        assert!(idle.chunks_exact(4).all(|p| p == &idle[..4]));
        assert_ne!(idle, hover);
    }
    #[test]
    fn dragging_up_grows_only_list_height_and_clamps_at_both_limits() {
        let mut widget = handle(150.0, 400.0);
        let mut tree = Tree::new(&widget);
        let renderer = Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let node = widget
            .as_widget_mut()
            .layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(Size::ZERO, Size::new(900.0, 6.0)),
            )
            .move_to(iced::Point::new(0.0, 300.0));
        let viewport = Rectangle::with_size(Size::new(900.0, 900.0));
        let mut messages = vec![];
        let events = [
            (mouse::Event::ButtonPressed(mouse::Button::Left), 303.0),
            (
                mouse::Event::CursorMoved {
                    position: iced::Point::new(20.0, 253.0),
                },
                253.0,
            ),
            (
                mouse::Event::CursorMoved {
                    position: iced::Point::new(20.0, -500.0),
                },
                -500.0,
            ),
            (
                mouse::Event::CursorMoved {
                    position: iced::Point::new(20.0, 800.0),
                },
                800.0,
            ),
            (mouse::Event::ButtonReleased(mouse::Button::Left), 800.0),
        ];
        for (event, y) in events {
            widget.as_widget_mut().update(
                &mut tree,
                &Event::Mouse(event),
                Layout::new(&node),
                mouse::Cursor::Available(iced::Point::new(20.0, y)),
                &renderer,
                &mut iced::advanced::clipboard::Null,
                &mut Shell::new(&mut messages),
                &viewport,
            );
        }
        assert!(matches!(
            messages.as_slice(),
            [
                Message::ResizeTransfers(200.0),
                Message::ResizeTransfers(400.0),
                Message::ResizeTransfers(60.0)
            ]
        ));
        let count = messages.len();
        widget.as_widget_mut().update(
            &mut tree,
            &Event::Mouse(mouse::Event::CursorMoved {
                position: iced::Point::new(20.0, 100.0),
            }),
            Layout::new(&node),
            mouse::Cursor::Available(iced::Point::new(20.0, 100.0)),
            &renderer,
            &mut iced::advanced::clipboard::Null,
            &mut Shell::new(&mut messages),
            &viewport,
        );
        assert_eq!(messages.len(), count);
    }
}
