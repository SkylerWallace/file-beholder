//! Portable session-independent settings and Windows appearance/elevation integration.
use std::path::PathBuf;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
    Auto,
}
impl ThemeMode {
    pub fn dark(self, system: Option<bool>) -> bool {
        match self {
            Self::Light => false,
            Self::Dark => true,
            Self::Auto => system.unwrap_or(true),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SoundChoice {
    #[default]
    Warm,
    Crystal,
    Off,
    Custom,
}
impl SoundChoice {
    pub const ALL: [Self; 4] = [Self::Warm, Self::Crystal, Self::Off, Self::Custom];
    pub fn key(self) -> &'static str {
        match self {
            Self::Warm => "warm",
            Self::Crystal => "crystal",
            Self::Off => "off",
            Self::Custom => "custom",
        }
    }
    pub fn from_key(key: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|v| v.key() == key)
            .unwrap_or_default()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TintMode {
    #[default]
    DarkOnly,
    On,
    Off,
}
impl TintMode {
    pub const ALL: [Self; 3] = [Self::DarkOnly, Self::On, Self::Off];
    pub fn enabled(self, dark: bool) -> bool {
        match self {
            Self::DarkOnly => dark,
            Self::On => true,
            Self::Off => false,
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::DarkOnly => "dark-only",
            Self::On => "on",
            Self::Off => "off",
        }
    }
    pub fn from_key(key: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|mode| mode.key() == key)
            .unwrap_or_default()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DefaultLayout {
    Single,
    Remember,
    #[default]
    Vertical,
    Horizontal,
    Three,
    Grid,
}
impl DefaultLayout {
    pub const ALL: [Self; 6] = [
        Self::Single,
        Self::Vertical,
        Self::Horizontal,
        Self::Three,
        Self::Grid,
        Self::Remember,
    ];
    pub fn key(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Remember => "remember",
            Self::Vertical => "vertical",
            Self::Horizontal => "horizontal",
            Self::Three => "three",
            Self::Grid => "grid",
        }
    }
    pub fn configuration(self) -> (usize, bool, bool) {
        match self {
            Self::Single => (1, true, false),
            Self::Vertical | Self::Remember => (2, true, false),
            Self::Horizontal => (2, false, false),
            Self::Three => (3, true, true),
            Self::Grid => (4, true, true),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScanMode {
    #[default]
    Auto,
    Mft,
    Recursive,
    Off,
}
impl ScanMode {
    pub const ALL: [Self; 4] = [Self::Auto, Self::Mft, Self::Recursive, Self::Off];
    pub fn key(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Mft => "mft",
            Self::Recursive => "recursive",
            Self::Off => "off",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SizeUnits {
    #[default]
    Auto,
    Binary,
    Metric,
}
impl SizeUnits {
    pub const ALL: [Self; 3] = [Self::Auto, Self::Binary, Self::Metric];
    pub fn key(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Binary => "binary",
            Self::Metric => "metric",
        }
    }
    pub fn metric(self) -> bool {
        match self {
            Self::Auto => cfg!(target_os = "macos"),
            Self::Binary => false,
            Self::Metric => true,
        }
    }
    pub fn format(self, bytes: u64) -> String {
        let (base, units) = if self.metric() {
            (1000., ["B", "kB", "MB", "GB", "TB", "PB", "EB"])
        } else {
            (1024., ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"])
        };
        let mut value = bytes as f64;
        let mut i = 0;
        while value >= base && i < units.len() - 1 {
            value /= base;
            i += 1;
        }
        format!("{value:.1} {}", units[i])
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Look {
    #[default]
    Default,
    Neumorphic,
    Material,
    Metro,
}
impl Look {
    pub const ALL: [Self; 4] = [Self::Default, Self::Neumorphic, Self::Material, Self::Metro];
    pub fn key(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Neumorphic => "neumorphic",
            Self::Material => "material",
            Self::Metro => "metro",
        }
    }
    pub fn from_key(key: &str) -> Self {
        match key {
            "neumorphic" => Self::Neumorphic,
            "material" => Self::Material,
            "metro" => Self::Metro,
            _ => Self::Default,
        }
    }
}
impl std::fmt::Display for Look {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Default => "Default",
            Self::Neumorphic => "Neumorphic",
            Self::Material => "Material",
            Self::Metro => "Metro",
        })
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct Preferences {
    pub look: Look,
    pub theme: ThemeMode,
    pub accent: crate::palette::Accent,
    pub tint: TintMode,
    pub scan_mode: ScanMode,
    pub admin: bool,
    pub completion_notifications: bool,
    pub completion_sound: bool,
    pub completion_tone: SoundChoice,
    pub custom_sound: String,
    pub open_files: bool,
    pub alternating_rows: bool,
    pub size_units: SizeUnits,
    pub density: u16,
    pub columns: crate::columns::Configuration,
    pub default_layout: DefaultLayout,
    pub sidebar: f32,
    pub panel_trees: bool,
    pub search_subfolders: bool,
    pub skipped_confirmations: Vec<String>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            look: Look::Default,
            theme: ThemeMode::Auto,
            accent: Default::default(),
            tint: Default::default(),
            scan_mode: ScanMode::Auto,
            admin: false,
            completion_notifications: false,
            completion_sound: false,
            completion_tone: SoundChoice::Warm,
            custom_sound: String::new(),
            open_files: true,
            alternating_rows: true,
            size_units: SizeUnits::Auto,
            density: 100,
            columns: Default::default(),
            default_layout: DefaultLayout::Vertical,
            sidebar: 0.15,
            panel_trees: false,
            search_subfolders: true,
            skipped_confirmations: vec![],
        }
    }
}
impl Preferences {
    pub fn sound_choice(&self) -> SoundChoice {
        if self.completion_sound {
            self.completion_tone
        } else {
            SoundChoice::Off
        }
    }
    pub fn decode(text: &str) -> Self {
        let mut p = Self::default();
        for line in text.lines() {
            if let Some((key, value)) = line.split_once('=') {
                match key {
                    "look" => p.look = Look::from_key(value),
                    "theme" => {
                        p.theme = match value {
                            "light" => ThemeMode::Light,
                            "dark" => ThemeMode::Dark,
                            _ => ThemeMode::Auto,
                        }
                    }
                    "accent" => p.accent = crate::palette::Accent::from_key(value),
                    "tint" => p.tint = TintMode::from_key(value),
                    "size_scan" => {
                        p.scan_mode = ScanMode::ALL
                            .into_iter()
                            .find(|m| m.key() == value)
                            .unwrap_or_default()
                    }
                    "completion_notifications" => p.completion_notifications = value == "true",
                    "completion_sound" => p.completion_sound = value == "true",
                    "completion_tone" => p.completion_tone = SoundChoice::from_key(value),
                    "custom_sound" => {
                        if value.starts_with("file-beholder-sound-")
                            && std::path::Path::new(value)
                                .file_name()
                                .and_then(|s| s.to_str())
                                == Some(value)
                            && !value.contains(['/', '\\'])
                        {
                            p.custom_sound = value.into();
                        }
                    }
                    "open_files" => p.open_files = value != "false",
                    "skip_confirmations" => {
                        p.skipped_confirmations = value
                            .split(',')
                            .filter(|key| *key == "recycle")
                            .map(str::to_owned)
                            .collect()
                    }
                    "alternating_rows" => p.alternating_rows = value != "false",
                    "size_units" => {
                        p.size_units = SizeUnits::ALL
                            .into_iter()
                            .find(|u| u.key() == value)
                            .unwrap_or_default()
                    }
                    "column_order" => {
                        p.columns.order = crate::columns::Configuration::decode_order(value)
                    }
                    "hidden_columns" => {
                        p.columns.hidden = value
                            .split(',')
                            .filter_map(|key| {
                                crate::columns::Column::OPTIONAL
                                    .into_iter()
                                    .find(|c| c.key() == key)
                            })
                            .collect()
                    }
                    "density" => p.density = density(value.parse().unwrap_or(100)),
                    "admin" => p.admin = value == "true",
                    "default_layout" => {
                        p.default_layout = DefaultLayout::ALL
                            .into_iter()
                            .find(|layout| layout.key() == value)
                            .unwrap_or_default()
                    }
                    "panel_trees" => p.panel_trees = value == "true",
                    "search_subfolders" => p.search_subfolders = value != "false",
                    "sidebar" => {
                        p.sidebar = value
                            .parse::<f32>()
                            .ok()
                            .filter(|v| v.is_finite())
                            .unwrap_or(0.15)
                            .clamp(0.10, 0.45)
                    }
                    _ => {}
                }
            }
        }
        // Existing configurations predate the extra metadata columns.
        let saved_order = text
            .lines()
            .find_map(|line| line.strip_prefix("column_order="))
            .unwrap_or("");
        for column in crate::columns::Column::EXTRA {
            if !saved_order.split(',').any(|key| key == column.key())
                && !p.columns.hidden.contains(&column)
            {
                p.columns.hidden.push(column);
            }
        }
        p
    }
    pub fn encode(&self) -> String {
        let base = format!(
            "theme={}\naccent={}\ntint={}\nsize_scan={}\nadmin={}\ndefault_layout={}\nsidebar={}\ncompletion_notifications={}\ncompletion_sound={}\ncompletion_tone={}\ncustom_sound={}\nopen_files={}\nsize_units={}\ncolumn_order={}\nhidden_columns={}\ndensity={}\nalternating_rows={}\n",
            match self.theme {
                ThemeMode::Light => "light",
                ThemeMode::Dark => "dark",
                ThemeMode::Auto => "auto",
            },
            self.accent.key(),
            self.tint.key(),
            self.scan_mode.key(),
            self.admin,
            self.default_layout.key(),
            self.sidebar,
            self.completion_notifications,
            self.completion_sound,
            self.completion_tone.key(),
            self.custom_sound,
            self.open_files,
            self.size_units.key(),
            self.columns
                .order
                .iter()
                .map(|c| c.key())
                .collect::<Vec<_>>()
                .join(","),
            self.columns
                .hidden
                .iter()
                .map(|c| c.key())
                .collect::<Vec<_>>()
                .join(","),
            self.density,
            self.alternating_rows
        );
        let base = format!(
            "{base}look={}\npanel_trees={}\nsearch_subfolders={}\n",
            self.look.key(),
            self.panel_trees,
            self.search_subfolders
        );
        format!(
            "{base}skip_confirmations={}\n",
            self.skipped_confirmations.join(",")
        )
    }
    fn path() -> PathBuf {
        std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("."))
            .with_file_name("file-beholder.ini")
    }
    pub fn load() -> Self {
        Self::load_at(Self::path())
    }
    fn load_at(path: PathBuf) -> Self {
        if let Ok(text) = std::fs::read_to_string(&path) {
            return Self::decode(&text);
        }
        let legacy = path.with_file_name("file-beholder-settings.ini");
        if let Ok(text) = std::fs::read_to_string(&legacy) {
            let preferences = Self::decode(&text);
            let _ = std::fs::rename(legacy, path);
            return preferences;
        }
        Self::default()
    }
    #[cfg(not(test))]
    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::path();
        let temporary = path.with_extension("ini.tmp");
        std::fs::write(&temporary, self.encode())?;
        std::fs::rename(temporary, path)
    }
}
pub fn density(value: u16) -> u16 {
    (((value.clamp(75, 150) + 12) / 25) * 25).clamp(75, 150)
}
/// Compact mode preserves readable text while tightening row spacing separately.
pub fn content_scale(value: u16) -> f32 {
    if density(value) == 75 {
        0.95
    } else {
        density(value) as f32 / 100.0
    }
}
pub fn row_height(value: u16) -> f32 {
    if density(value) == 75 {
        18.0
    } else {
        crate::virtual_list::ROW_HEIGHT * density(value) as f32 / 100.
    }
}
pub fn system_dark() -> Option<bool> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Registry::*;
        let key: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let value: Vec<u16> = "AppsUseLightTheme".encode_utf16().chain(Some(0)).collect();
        let mut data = 0u32;
        let mut bytes = 4u32;
        if RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&mut data as *mut u32).cast(),
            &mut bytes,
        ) == 0
        {
            return Some(data == 0);
        }
    }
    None
}
pub fn theme_watch() -> impl iced::futures::Stream<Item = Option<bool>> {
    let (tx, rx) = iced::futures::channel::mpsc::unbounded();
    std::thread::spawn(move || {
        let mut last = None;
        loop {
            let now = system_dark();
            if last != Some(now) {
                if tx.unbounded_send(now).is_err() {
                    break;
                }
                last = Some(now);
            } else if tx.is_closed() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    });
    rx
}
static LAUNCH_ERROR: std::sync::OnceLock<String> = std::sync::OnceLock::new();
pub fn launch_error() -> Option<&'static str> {
    LAUNCH_ERROR.get().map(String::as_str)
}
pub fn is_elevated() -> bool {
    #[cfg(windows)]
    {
        unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
    }
    #[cfg(not(windows))]
    {
        false
    }
}
pub fn relaunch_elevated() -> Result<(), String> {
    #[cfg(windows)]
    unsafe {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let executable: Vec<u16> = exe.as_os_str().encode_wide().chain(Some(0)).collect();
        let verb: Vec<u16> = "runas".encode_utf16().chain(Some(0)).collect();
        let cwd = std::env::current_dir().ok().map(|p| {
            p.as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<_>>()
        });
        let result = ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            executable.as_ptr(),
            std::ptr::null(),
            cwd.as_ref().map_or(std::ptr::null(), |v| v.as_ptr()),
            SW_SHOWNORMAL,
        ) as isize;
        if result > 32 {
            Ok(())
        } else {
            Err(
                "Administrator relaunch was cancelled or unavailable; current window retained."
                    .into(),
            )
        }
    }
    #[cfg(not(windows))]
    {
        Err("Administrator relaunch is available on Windows.".into())
    }
}
pub fn request_admin_on_launch() {
    if Preferences::load().admin && !is_elevated() {
        match relaunch_elevated() {
            Ok(()) => std::process::exit(0),
            Err(error) => {
                let _ = LAUNCH_ERROR.set(error);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_layout_roundtrips_and_legacy_or_unknown_values_use_vertical() {
        for layout in DefaultLayout::ALL {
            let mut preferences = Preferences::default();
            preferences.default_layout = layout;
            assert_eq!(
                Preferences::decode(&preferences.encode()).default_layout,
                layout
            );
        }
        for text in [
            "",
            "panels=4\nstacked=false\ngrid=true",
            "default_layout=unknown",
        ] {
            assert_eq!(
                Preferences::decode(text).default_layout,
                DefaultLayout::Vertical
            );
        }
    }
    #[test]
    fn settings_roundtrip_and_invalid_values() {
        let p = Preferences {
            look: Look::Default,
            theme: ThemeMode::Light,
            accent: Default::default(),
            tint: Default::default(),
            scan_mode: ScanMode::Mft,
            admin: true,
            completion_notifications: true,
            completion_sound: true,
            completion_tone: SoundChoice::Warm,
            custom_sound: String::new(),
            open_files: false,
            alternating_rows: false,
            size_units: SizeUnits::Metric,
            density: 100,
            columns: Default::default(),
            default_layout: DefaultLayout::Three,
            sidebar: 0.2,
            panel_trees: true,
            search_subfolders: false,
            skipped_confirmations: vec![],
        };
        assert_eq!(Preferences::decode(&p.encode()), p);
        let p = Preferences::decode("panels=99\nsidebar=NaN\ntheme=nonsense");
        assert_eq!(p.sidebar, 0.15);
        assert_eq!(p.theme, ThemeMode::Auto);
        assert!(!p.admin);
    }
    #[test]
    fn settings_migrate_once_and_scan_modes_roundtrip() {
        let dir = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!(
                "file-beholder-settings-test-{}",
                std::process::id()
            ));
        std::fs::create_dir_all(&dir).unwrap();
        let legacy = dir.join("file-beholder-settings.ini");
        let current = dir.join("file-beholder.ini");
        std::fs::write(&legacy, "theme=light\nsize_scan=mft").unwrap();
        let p = Preferences::load_at(current.clone());
        assert_eq!(p.theme, ThemeMode::Light);
        assert_eq!(p.scan_mode, ScanMode::Mft);
        assert!(current.exists());
        assert!(!legacy.exists());
        std::fs::write(&legacy, "theme=dark").unwrap();
        assert_eq!(
            Preferences::load_at(current.clone()).theme,
            ThemeMode::Light
        );
        for mode in ScanMode::ALL {
            let mut p = Preferences::default();
            p.scan_mode = mode;
            assert_eq!(Preferences::decode(&p.encode()).scan_mode, mode);
        }
        assert_eq!(
            Preferences::decode("size_scan=unknown").scan_mode,
            ScanMode::Auto
        );
        std::fs::remove_file(current).unwrap();
        std::fs::remove_file(legacy).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn auto_follows_system_but_manual_modes_hold() {
        assert!(ThemeMode::Auto.dark(Some(true)));
        assert!(!ThemeMode::Auto.dark(Some(false)));
        assert!(ThemeMode::Dark.dark(Some(false)));
        assert!(!ThemeMode::Light.dark(Some(true)));
    }
}

#[cfg(test)]
mod feature_tests {
    use super::*;
    #[test]
    fn units_settings_and_column_customization_roundtrip() {
        assert_eq!(SizeUnits::Binary.format(1_048_576), "1.0 MiB");
        assert_eq!(SizeUnits::Metric.format(1_000_000), "1.0 MB");
        assert_eq!(SizeUnits::Binary.format(1000), "1000.0 B");
        assert_eq!(SizeUnits::Metric.format(1000), "1.0 kB");
        assert_eq!(SizeUnits::Auto.metric(), cfg!(target_os = "macos"));
        let mut p = Preferences::default();
        assert!(!p.completion_notifications);
        assert!(!p.completion_sound);
        assert!(p.open_files);
        p.completion_notifications = true;
        p.completion_sound = true;
        p.open_files = false;
        p.size_units = SizeUnits::Metric;
        p.columns.toggle(crate::columns::Column::Size);
        p.columns
            .move_to(crate::columns::Column::Name, crate::columns::Column::Share);
        assert_eq!(Preferences::decode(&p.encode()), p);
        let p = Preferences::decode(
            "hidden_columns=name,type,size\ncolumn_order=size,size,bogus\nsize_units=bogus",
        );
        assert_eq!(
            p.columns.hidden,
            [
                vec![crate::columns::Column::Size],
                crate::columns::Column::EXTRA.to_vec()
            ]
            .concat()
        );
        assert_eq!(p.columns.order.len(), 12);
        assert_eq!(p.size_units, SizeUnits::Auto);
    }
}

#[cfg(test)]
mod density_tests {
    use super::*;
    #[test]
    fn density_steps_roundtrip_and_bad_values_are_bounded() {
        for value in (75..=150).step_by(25) {
            let mut p = Preferences::default();
            p.density = value;
            assert_eq!(Preferences::decode(&p.encode()).density, value);
            assert_eq!(
                row_height(value),
                if value == 75 {
                    18.0
                } else {
                    21. * value as f32 / 100.
                }
            );
        }
        assert_eq!(Preferences::decode("density=nonsense").density, 100);
        assert_eq!(Preferences::decode("density=0").density, 75);
        assert_eq!(Preferences::decode("density=65535").density, 150);
        assert_eq!(Preferences::decode("density=139").density, 150);
    }
}

impl std::fmt::Display for ScanMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Auto => "Auto",
            Self::Mft => "MFT",
            Self::Recursive => "Recursive",
            Self::Off => "Off",
        })
    }
}
impl std::fmt::Display for SizeUnits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Auto => "Native",
            Self::Binary => "Binary",
            Self::Metric => "Metric",
        })
    }
}
impl std::fmt::Display for SoundChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Warm => "Warm",
            Self::Crystal => "Crystal",
            Self::Off => "Off",
            Self::Custom => "Custom",
        })
    }
}

impl std::fmt::Display for TintMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::DarkOnly => "Dark only",
            Self::On => "On",
            Self::Off => "Off",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileTreeMode {
    Contextual,
    PerPanel,
}
impl std::fmt::Display for FileTreeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Contextual => "Contextual",
            Self::PerPanel => "Per panel",
        })
    }
}
