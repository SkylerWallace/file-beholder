//! Persisted column identity, visibility and ordering, independent of panel geometry.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Column {
    Type,
    Name,
    Extension,
    Modified,
    Size,
    Share,
    Created,
    Description,
    Attributes,
    Comments,
    Authors,
    Company,
}
impl Column {
    pub const ALL: [Self; 12] = [
        Self::Type,
        Self::Name,
        Self::Extension,
        Self::Modified,
        Self::Size,
        Self::Share,
        Self::Created,
        Self::Description,
        Self::Attributes,
        Self::Comments,
        Self::Authors,
        Self::Company,
    ];
    pub const EXTRA: [Self; 6] = [
        Self::Created,
        Self::Description,
        Self::Attributes,
        Self::Comments,
        Self::Authors,
        Self::Company,
    ];
    pub const OPTIONAL: [Self; 10] = [
        Self::Extension,
        Self::Modified,
        Self::Size,
        Self::Share,
        Self::Created,
        Self::Description,
        Self::Attributes,
        Self::Comments,
        Self::Authors,
        Self::Company,
    ];
    pub fn property_index(self) -> Option<usize> {
        Self::EXTRA.iter().position(|c| *c == self)
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Type => "Type",
            Self::Name => "Name",
            Self::Extension => "Extension",
            Self::Modified => "Date modified",
            Self::Size => "Size",
            Self::Share => "Share",
            Self::Created => "Date created",
            Self::Description => "Description",
            Self::Attributes => "Attributes",
            Self::Comments => "Comments",
            Self::Authors => "Authors",
            Self::Company => "Company",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::Name => "name",
            Self::Extension => "extension",
            Self::Modified => "modified",
            Self::Size => "size",
            Self::Share => "share",
            Self::Created => "created",
            Self::Description => "description",
            Self::Attributes => "attributes",
            Self::Comments => "comments",
            Self::Authors => "authors",
            Self::Company => "company",
        }
    }
    pub fn sort(self) -> crate::Sort {
        match self {
            Self::Type => crate::Sort::Type,
            Self::Name => crate::Sort::Name,
            Self::Extension => crate::Sort::Extension,
            Self::Modified => crate::Sort::Modified,
            Self::Size => crate::Sort::Size,
            Self::Share => crate::Sort::Share,
            column => crate::Sort::Property(column.property_index().unwrap()),
        }
    }
    pub fn width_index(self) -> Option<usize> {
        match self {
            Self::Type => Some(0),
            Self::Modified => Some(1),
            Self::Size => Some(2),
            Self::Share => Some(3),
            Self::Extension => Some(4),
            _ => None,
        }
    }
    pub fn width(self, widths: [f32; 4], extension: f32) -> iced::Length {
        if self.property_index().is_some() {
            return iced::Length::Fixed(160.0);
        }
        self.width_index().map_or(iced::Length::Fill, |i| {
            iced::Length::Fixed(if i == 4 { extension } else { widths[i] })
        })
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct Configuration(sk_iced::components::TableColumns<Column>);
impl std::ops::Deref for Configuration {
    type Target = sk_iced::components::TableColumns<Column>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for Configuration {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Default for Configuration {
    fn default() -> Self {
        Self(sk_iced::components::TableColumns::new(Column::ALL).hidden(Column::EXTRA))
    }
}
impl Configuration {
    pub fn visible(&self) -> Vec<Column> {
        self.0.visible()
    }
    pub fn toggle(&mut self, c: Column) {
        if Column::OPTIONAL.contains(&c) {
            self.0.toggle(c);
        }
    }
    pub fn move_to(&mut self, from: Column, to: Column) {
        self.0.move_to(from, to);
    }
    pub fn decode_order(value: &str) -> Vec<Column> {
        let mut out = Vec::new();
        for key in value.split(',') {
            if let Some(c) = Column::ALL.into_iter().find(|c| c.key() == key) {
                if !out.contains(&c) {
                    out.push(c);
                }
            }
        }
        for c in Column::ALL {
            if !out.contains(&c) {
                out.push(c);
            }
        }
        out
    }
}
pub fn extension(path: &std::path::Path, directory: bool) -> String {
    if directory {
        String::new()
    } else {
        path.extension()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hide_restore_and_move_columns_preserves_every_identity_once() {
        let mut c = Configuration::default();
        c.move_to(Column::Type, Column::Share);
        assert_eq!(c.order[5], Column::Type);
        c.move_to(Column::Type, Column::Name);
        assert_eq!(c.order[0], Column::Type);
        c.toggle(Column::Extension);
        assert!(!c.visible().contains(&Column::Extension));
        c.move_to(Column::Extension, Column::Share);
        let order = c.order.clone();
        c.toggle(Column::Extension);
        assert_eq!(c.order, order);
        assert!(c.visible().contains(&Column::Extension));
        c.toggle(Column::Name);
        c.toggle(Column::Type);
        assert!(c.visible().contains(&Column::Name));
        assert!(c.visible().contains(&Column::Type));
        assert_eq!(
            Configuration::decode_order("size,size,bogus"),
            vec![
                Column::Size,
                Column::Type,
                Column::Name,
                Column::Extension,
                Column::Modified,
                Column::Share,
                Column::Created,
                Column::Description,
                Column::Attributes,
                Column::Comments,
                Column::Authors,
                Column::Company
            ]
        );
        assert_eq!(
            extension(std::path::Path::new("report.final.TXT"), false),
            "TXT"
        );
        assert_eq!(extension(std::path::Path::new("folder.txt"), true), "");
        assert_eq!(extension(std::path::Path::new(".gitignore"), false), "");
    }
}
