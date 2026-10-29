//! File column dimensions/messages and app-specific tab backgrounds.
use crate::{Message, palette};
use iced::Element;
#[cfg(test)]
use iced::advanced::Renderer as _;
#[cfg(test)]
use iced::advanced::{Layout, Shell, layout, mouse, renderer, widget::Tree};
#[cfg(test)]
use iced::{Event, Rectangle, Renderer, Size};
pub fn handle_scaled(
    pane: usize,
    column: usize,
    width: f32,
    maximum: f32,
    scale: f32,
) -> Element<'static, Message> {
    let minimum = match column {
        0 => 38.0,
        1 => 92.0,
        2 | 4 => 64.0,
        _ => 100.0,
    } * scale;
    sk_iced::components::ColumnResize::new(
        sk_iced::components::Appearance::themed(crate::typography::font(), palette::from_theme),
        width,
    )
    .limits(minimum, maximum)
    .scale(scale)
    .leading(column != 0)
    .on_resize(move |width| Message::ResizeColumn(pane, column, width / scale))
    .build()
}
pub fn tab_background<'a, M: 'a>(content: Element<'a, M>, selected: bool) -> Element<'a, M> {
    sk_iced::components::TabSurface::new(
        sk_iced::components::Appearance::themed(crate::typography::font(), palette::from_theme),
        content,
        selected,
    )
    .build()
}
#[cfg(test)]
mod tests {
    use super::*;
    use iced::Point;
    #[test]
    fn column_drag_survives_rebuilds_and_stops_after_release() {
        let mut element = handle_scaled(1, 1, 132.0, 240.0, 1.);
        let mut tree = Tree::new(&element);
        let renderer = Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let size = Size::new(6.0, 26.0);
        let node =
            element
                .as_widget_mut()
                .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
        let mut messages = vec![];
        let send = |element: &mut Element<'_, Message>,
                    tree: &mut Tree,
                    messages: &mut Vec<Message>,
                    event,
                    point| {
            element.as_widget_mut().update(
                tree,
                &Event::Mouse(event),
                Layout::new(&node),
                mouse::Cursor::Available(point),
                &renderer,
                &mut iced::advanced::clipboard::Null,
                &mut Shell::new(messages),
                &Rectangle::with_size(size),
            );
        };
        send(
            &mut element,
            &mut tree,
            &mut messages,
            mouse::Event::ButtonPressed(mouse::Button::Left),
            Point::new(3.0, 10.0),
        );
        send(
            &mut element,
            &mut tree,
            &mut messages,
            mouse::Event::CursorMoved {
                position: Point::new(-37.0, 10.0),
            },
            Point::new(-37.0, 10.0),
        );
        assert!(matches!(messages.last(),Some(Message::ResizeColumn(1,1,w)) if *w==172.0));
        // Iced recreates the view after each published width; preserve original drag origin.
        element = handle_scaled(1, 1, 172.0, 240.0, 1.);
        tree.diff(element.as_widget());
        send(
            &mut element,
            &mut tree,
            &mut messages,
            mouse::Event::CursorMoved {
                position: Point::new(-57.0, 10.0),
            },
            Point::new(-57.0, 10.0),
        );
        assert!(matches!(messages.last(),Some(Message::ResizeColumn(1,1,w)) if *w==192.0));
        send(
            &mut element,
            &mut tree,
            &mut messages,
            mouse::Event::CursorMoved {
                position: Point::new(-500.0, 10.0),
            },
            Point::new(-500.0, 10.0),
        );
        assert!(matches!(messages.last(),Some(Message::ResizeColumn(1,1,w)) if *w==240.0));
        send(
            &mut element,
            &mut tree,
            &mut messages,
            mouse::Event::ButtonReleased(mouse::Button::Left),
            Point::new(-500.0, 10.0),
        );
        let count = messages.len();
        send(
            &mut element,
            &mut tree,
            &mut messages,
            mouse::Event::CursorMoved {
                position: Point::new(50.0, 10.0),
            },
            Point::new(50.0, 10.0),
        );
        assert_eq!(messages.len(), count);
    }
    #[test]
    fn hovering_close_does_not_highlight_the_tab_title() {
        use iced::advanced::renderer::Headless as _;
        let mut renderer = Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let mut element = crate::components::browser_tab(
            "Folder".into(),
            Message::Noop,
            Some(Message::Noop),
            false,
        );
        let mut tree = Tree::new(&element);
        let size = Size::new(188.0, 30.0);
        let node =
            element
                .as_widget_mut()
                .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
        let theme = iced::Theme::Dark;
        let bounds = Rectangle::with_size(size);
        let mut render = |cursor| {
            renderer.reset(bounds);
            element.as_widget().draw(
                &tree,
                &mut renderer,
                &theme,
                &renderer::Style::default(),
                Layout::new(&node),
                cursor,
                &bounds,
            );
            renderer.screenshot(iced::Size::new(188, 30), 1.0, iced::Color::BLACK)
        };
        let normal = render(mouse::Cursor::Unavailable);
        let title = render(mouse::Cursor::Available(Point::new(50.0, 15.0)));
        let close = render(mouse::Cursor::Available(Point::new(175.0, 15.0)));
        let offset = (3 * 188 + 40) * 4;
        assert_ne!(&normal[offset..offset + 4], &title[offset..offset + 4]);
        assert_eq!(&normal[offset..offset + 4], &close[offset..offset + 4]);
    }
}
