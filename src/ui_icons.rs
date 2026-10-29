//! Embedded Material Design SVGs and application disclosure artwork.
use iced::{Element, widget::svg};
#[derive(Clone, Copy)]
pub enum Icon {
    NewPanel,
    Copy,
    Rename,
    NewFolder,
    OpenFile,
    OpenFolder,
    OpenWith,
    CopyPath,
    Terminal,
    Reveal,
    Properties,
    Licenses,
    Admin,
    Cut,
    Paste,
    Delete,
    Undo,
    Search,
    AdvancedSearch,
    Restore,
}
impl Icon {
    pub fn key(self) -> &'static str {
        match self {
            Self::NewPanel => "new_panel",
            Self::Copy => "copy",
            Self::Rename => "rename",
            Self::NewFolder => "new_folder",
            Self::OpenFile => "open_file",
            Self::OpenFolder => "open_folder",
            Self::OpenWith => "open_with",
            Self::CopyPath => "copy_path",
            Self::Terminal => "terminal",
            Self::Reveal => "reveal",
            Self::Properties => "properties",
            Self::Licenses => "licenses",
            Self::Admin => "admin",
            Self::Cut => "cut",
            Self::Paste => "paste",
            Self::Delete => "delete",
            Self::Undo => "undo",
            Self::Search => "search",
            Self::AdvancedSearch => "advanced_search",
            Self::Restore => "restore",
        }
    }
}
pub fn material<'a, M: 'a>(icon: Icon, danger: bool, enabled: bool) -> Element<'a, M> {
    static NEW_PANEL: std::sync::OnceLock<svg::Handle> = std::sync::OnceLock::new();
    static NEW_FOLDER: std::sync::OnceLock<svg::Handle> = std::sync::OnceLock::new();
    static RENAME: std::sync::OnceLock<svg::Handle> = std::sync::OnceLock::new();
    static OPENFILE: std::sync::OnceLock<svg::Handle> = std::sync::OnceLock::new();
    static OPENFOLDER: std::sync::OnceLock<svg::Handle> = std::sync::OnceLock::new();
    static OPENWITH: std::sync::OnceLock<svg::Handle> = std::sync::OnceLock::new();
    static COPYPATH: std::sync::OnceLock<svg::Handle> = std::sync::OnceLock::new();
    static TERMINAL: std::sync::OnceLock<svg::Handle> = std::sync::OnceLock::new();
    static REVEAL: std::sync::OnceLock<svg::Handle> = std::sync::OnceLock::new();
    static PROPERTIES: std::sync::OnceLock<svg::Handle> = std::sync::OnceLock::new();
    static ADMIN: std::sync::OnceLock<svg::Handle> = std::sync::OnceLock::new();
    static COPY: std::sync::OnceLock<svg::Handle> = std::sync::OnceLock::new();
    static CUT: std::sync::OnceLock<svg::Handle> = std::sync::OnceLock::new();
    static PASTE: std::sync::OnceLock<svg::Handle> = std::sync::OnceLock::new();
    static DELETE: std::sync::OnceLock<svg::Handle> = std::sync::OnceLock::new();
    static UNDO: std::sync::OnceLock<svg::Handle> = std::sync::OnceLock::new();
    static SEARCH: std::sync::OnceLock<svg::Handle> = std::sync::OnceLock::new();
    static ADVANCED_SEARCH: std::sync::OnceLock<svg::Handle> = std::sync::OnceLock::new();
    static RESTORE: std::sync::OnceLock<svg::Handle> = std::sync::OnceLock::new();
    let (cache, bytes): (&_, &[u8]) = match icon {
        Icon::NewPanel => (&NEW_PANEL, include_bytes!("../assets/material/add_box.svg")),
        Icon::NewFolder => (
            &NEW_FOLDER,
            include_bytes!("../assets/material/create_new_folder.svg"),
        ),
        Icon::Rename => (
            &RENAME,
            include_bytes!("../assets/material/drive_file_rename_outline.svg"),
        ),
        Icon::OpenFile => (
            &OPENFILE,
            include_bytes!("../assets/material/open_in_new.svg"),
        ),
        Icon::OpenFolder => (
            &OPENFOLDER,
            include_bytes!("../assets/material/folder_open.svg"),
        ),
        Icon::OpenWith => (
            &OPENWITH,
            include_bytes!("../assets/material/open_with.svg"),
        ),
        Icon::CopyPath => (&COPYPATH, include_bytes!("../assets/material/link.svg")),
        Icon::Terminal => (&TERMINAL, include_bytes!("../assets/material/terminal.svg")),
        Icon::Reveal => (&REVEAL, include_bytes!("../assets/material/source.svg")),
        Icon::Properties | Icon::Licenses => {
            (&PROPERTIES, include_bytes!("../assets/material/info.svg"))
        }
        Icon::Admin => (&ADMIN, include_bytes!("../assets/material/security.svg")),
        Icon::Undo => (&UNDO, include_bytes!("../assets/material/undo.svg")),
        Icon::Restore => (&RESTORE, include_bytes!("../assets/material/restore.svg")),
        Icon::AdvancedSearch => (
            &ADVANCED_SEARCH,
            include_bytes!("../assets/material/manage_search.svg"),
        ),
        Icon::Search => (&SEARCH, include_bytes!("../assets/material/search.svg")),
        Icon::Copy => (&COPY, include_bytes!("../assets/material/content_copy.svg")),
        Icon::Cut => (&CUT, include_bytes!("../assets/material/content_cut.svg")),
        Icon::Paste => (
            &PASTE,
            include_bytes!("../assets/material/content_paste.svg"),
        ),
        Icon::Delete => (&DELETE, include_bytes!("../assets/material/delete.svg")),
    };
    sk_iced::icons::Icon::svg(
        sk_iced::components::Appearance::themed(
            crate::typography::font(),
            crate::palette::from_theme,
        ),
        cache
            .get_or_init(|| svg::Handle::from_memory(bytes))
            .clone(),
    )
    .enabled(enabled)
    .tone(if danger {
        sk_iced::icons::IconTone::OnDanger
    } else {
        sk_iced::icons::IconTone::Text
    })
    .semantic(icon.key())
    .build()
}
pub fn disclosure<'a, M: 'a>(expanded: bool, tint: u32, size: f32) -> Element<'a, M> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<sk_iced::icons::AlphaIconCache<bool>>> =
        std::sync::OnceLock::new();
    let mut cache = CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let alpha: &[u8] = if expanded {
        include_bytes!("../assets/collapse-alpha.bin")
    } else {
        include_bytes!("../assets/expand-alpha.bin")
    };
    let handle = cache
        .get_or_insert(expanded, tint, 32, 32, alpha)
        .expect("Verified disclosure mask");
    sk_iced::icons::Icon::raster(
        sk_iced::components::Appearance::themed(
            crate::typography::font(),
            crate::palette::from_theme,
        ),
        handle,
    )
    .size(size)
    .build()
}

pub fn shortcut(label: &str) -> &'static str {
    match label {
        "Close panel" => "Ctrl+Shift+W",
        "Close tab" => "Ctrl+W",
        "New tab" => "Ctrl+T",
        "Search" => "Ctrl+F",
        "New panel" => "Ctrl+N",
        "Copy" => "Ctrl+C",
        "Cut" => "Ctrl+X",
        "Paste" => "Ctrl+V",
        "Delete" => "Delete",
        "Rename" => "F2",
        "Undo" => "Ctrl+Z",
        "New folder" => "Ctrl+Shift+N",
        _ => "",
    }
}

/// Theme icon pack; semantic names keep the artwork in the application.
pub fn material_pack(name: &str) -> Option<sk_iced::visual::IconAsset> {
    Some(sk_iced::visual::IconAsset::Svg(match name {
        "new_panel" => include_bytes!("../assets/material/add_box.svg"),
        "new_folder" => include_bytes!("../assets/material/create_new_folder.svg"),
        "rename" => include_bytes!("../assets/material/drive_file_rename_outline.svg"),
        "open_file" => include_bytes!("../assets/material/open_in_new.svg"),
        "open_folder" => include_bytes!("../assets/material/folder_open.svg"),
        "open_with" => include_bytes!("../assets/material/open_with.svg"),
        "copy_path" => include_bytes!("../assets/material/link.svg"),
        "terminal" => include_bytes!("../assets/material/terminal.svg"),
        "reveal" => include_bytes!("../assets/material/source.svg"),
        "properties" | "licenses" => include_bytes!("../assets/material/info.svg"),
        "admin" => include_bytes!("../assets/material/security.svg"),
        "undo" => include_bytes!("../assets/material/undo.svg"),
        "restore" => include_bytes!("../assets/material/restore.svg"),
        "advanced_search" => include_bytes!("../assets/material/manage_search.svg"),
        "search" => include_bytes!("../assets/material/search.svg"),
        "copy" => include_bytes!("../assets/material/content_copy.svg"),
        "cut" => include_bytes!("../assets/material/content_cut.svg"),
        "paste" => include_bytes!("../assets/material/content_paste.svg"),
        "delete" => include_bytes!("../assets/material/delete.svg"),
        _ => return None,
    }))
}

/// Match the label ink of a toggle, including its selected accent surface.
pub fn toggle_material<'a, M: 'a>(
    icon: Icon,
    t: crate::theme::Tokens,
    selected: bool,
) -> Element<'a, M> {
    let Some(sk_iced::visual::IconAsset::Svg(bytes)) = material_pack(icon.key()) else {
        return material(icon, false, true);
    };
    svg(svg::Handle::from_memory(bytes))
        .width(16)
        .height(16)
        .style(move |theme, _| svg::Style {
            color: Some(
                t.toggle_button(selected)(theme, iced::widget::button::Status::Active).text_color,
            ),
        })
        .into()
}
