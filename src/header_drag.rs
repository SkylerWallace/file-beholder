//! File column IDs and messages stay local; shared headers own pointer gestures.
use crate::{Message, columns::Column};
use iced::Element;
#[cfg(test)]
use iced::advanced::{
    Layout, Shell, layout, mouse,
    widget::{Operation, Tree},
};
#[cfg(test)]
use iced::{Event, Point, Rectangle, Renderer, Size, Vector};
pub fn view<'a>(
    content: Element<'a, Message>,
    pane: usize,
    columns: Vec<Column>,
    scale: f32,
) -> Element<'a, Message> {
    use sk_iced::components::{Appearance, HeaderResize, TableHeader};
    let edges = columns
        .iter()
        .map(|c| {
            if c.width_index().is_none() {
                HeaderResize::None
            } else if *c == Column::Type {
                HeaderResize::Trailing(6.0 * scale)
            } else {
                HeaderResize::Leading(6.0 * scale)
            }
        })
        .collect::<Vec<_>>();
    let sortable = columns.clone();
    TableHeader::new(
        Appearance::themed(crate::typography::font(), crate::palette::from_theme),
        content,
    )
    .columns(edges)
    .on_sort(move |i| Message::Sort(pane, sortable[i].sort()))
    .on_move(move |from, to| Message::ColumnMove(columns[from], columns[to]))
    .on_context(move |point| Message::HeaderContext(pane, point))
    .build()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn gesture(events: &[(mouse::Event, Point)]) -> Vec<Message> {
        let cols = Column::ALL.to_vec();
        let content = iced::widget::Row::with_children(cols.iter().map(|c| {
            iced::widget::container(crate::typography::text(c.label()))
                .width(100)
                .height(26)
                .into()
        }));
        let mut view = view(content.into(), 0, cols, 1.);
        let mut tree = Tree::new(&view);
        let renderer = Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let size = Size::new(600., 26.);
        let node =
            view.as_widget_mut()
                .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
        let mut messages = Vec::new();
        for (event, p) in events {
            view.as_widget_mut().update(
                &mut tree,
                &Event::Mouse(*event),
                Layout::new(&node),
                mouse::Cursor::Available(*p),
                &renderer,
                &mut iced::advanced::clipboard::Null,
                &mut Shell::new(&mut messages),
                &node.bounds(),
            );
        }
        messages
    }
    #[test]
    fn click_sorts_drag_reorders_and_right_click_opens_columns_without_sorting() {
        let p = Point::new(250., 12.);
        let q = Point::new(550., 12.);
        let click = gesture(&[
            (mouse::Event::ButtonPressed(mouse::Button::Left), p),
            (mouse::Event::ButtonReleased(mouse::Button::Left), p),
        ]);
        assert!(matches!(
            click.as_slice(),
            [Message::Sort(0, crate::Sort::Extension)]
        ));
        let drag = gesture(&[
            (mouse::Event::ButtonPressed(mouse::Button::Left), p),
            (mouse::Event::CursorMoved { position: q }, q),
            (mouse::Event::ButtonReleased(mouse::Button::Left), q),
        ]);
        assert!(matches!(
            drag.as_slice(),
            [Message::ColumnMove(Column::Extension, Column::Share)]
        ));
        let context = gesture(&[(mouse::Event::ButtonPressed(mouse::Button::Right), p)]);
        assert!(matches!(context.as_slice(), [Message::HeaderContext(0, _)]));
        let outside = Point::new(650., 12.);
        let cancelled = gesture(&[
            (mouse::Event::ButtonPressed(mouse::Button::Left), p),
            (mouse::Event::CursorMoved { position: outside }, outside),
            (mouse::Event::ButtonReleased(mouse::Button::Left), outside),
        ]);
        assert!(cancelled.is_empty());
    }
    #[test]
    fn scrolled_header_context_menu_uses_screen_coordinates() {
        struct Offset;
        impl Operation for Offset {
            fn traverse(&mut self, f: &mut dyn FnMut(&mut dyn Operation)) {
                f(self);
            }
            fn scrollable(
                &mut self,
                _: Option<&iced::advanced::widget::Id>,
                _: Rectangle,
                _: Rectangle,
                _: Vector,
                state: &mut dyn iced::advanced::widget::operation::Scrollable,
            ) {
                state.scroll_to(
                    iced::advanced::widget::operation::scrollable::AbsoluteOffset {
                        x: Some(200.),
                        y: None,
                    },
                );
            }
        }
        let content = iced::widget::Row::with_children(Column::ALL.iter().map(|c| {
            iced::widget::container(crate::typography::text(c.label()))
                .width(100)
                .height(26)
                .into()
        }))
        .width(600);
        let header = crate::tab_drag::mark(
            view(content.into(), 0, Column::ALL.to_vec(), 1.),
            crate::tab_drag::Marker::headers(0),
        );
        let scroll = iced::widget::scrollable(header)
            .direction(iced::widget::scrollable::Direction::Horizontal(
                iced::widget::scrollable::Scrollbar::hidden(),
            ))
            .width(300);
        let mut view = crate::tab_drag::layer(scroll.into(), true);
        let mut tree = Tree::new(&view);
        let renderer = Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let size = Size::new(300., 26.);
        let node =
            view.as_widget_mut()
                .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
        view.as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut Offset);
        let point = Point::new(50., 12.);
        let mut messages = Vec::new();
        view.as_widget_mut().update(
            &mut tree,
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)),
            Layout::new(&node),
            mouse::Cursor::Available(point),
            &renderer,
            &mut iced::advanced::clipboard::Null,
            &mut Shell::new(&mut messages),
            &node.bounds(),
        );
        assert!(
            matches!(messages.as_slice(),[Message::HeaderContext(0,p)] if *p==point),
            "{messages:?}"
        );
    }
}
