//! Lazily enumerated directory tree. No recursive size scans on expansion.
use crate::{
    theme::{Tokens, blend, color},
    typography::text,
    virtual_list::ROW_HEIGHT,
};
use iced::widget::{Space, button, container};
use iced::{Element, Fill, Task};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Debug, Clone)]
pub enum Message {
    Toggle(PathBuf),
    DriveLabels(u64, Vec<(PathBuf, String)>),
    Loaded(u64, PathBuf, Result<Vec<PathBuf>, String>),
    Navigate(PathBuf),
    Refresh,
    CollapseAll,
    NetworkInput(String),
    AddNetwork,
}
#[derive(Default)]
struct Node {
    expanded: bool,
    loading: bool,
    children: Option<Vec<PathBuf>>,
    error: Option<String>,
}
pub struct Locations {
    roots: Vec<PathBuf>,
    labels: BTreeMap<PathBuf, String>,
    nodes: BTreeMap<PathBuf, Node>,
    generation: u64,
    network_input: String,
    reveal_path: Option<PathBuf>,
    keyboard_cursor: Option<PathBuf>,
}
impl Locations {
    pub fn new(home: PathBuf, drives: Vec<PathBuf>) -> Self {
        let mut roots = vec![home];
        roots.extend(drives);
        Self {
            roots,
            labels: BTreeMap::new(),
            nodes: BTreeMap::new(),
            generation: 0,
            network_input: String::new(),
            reveal_path: None,
            keyboard_cursor: None,
        }
    }
    pub fn reveal(&mut self, path: PathBuf) -> Task<Message> {
        self.reveal_path = Some(std::path::absolute(&path).unwrap_or(path));
        self.resume_reveal()
    }
    fn resume_reveal(&mut self) -> Task<Message> {
        let Some(target) = self.reveal_path.clone() else {
            return Task::none();
        };
        let root = self
            .roots
            .iter()
            .filter(|r| node_key(&target).starts_with(node_key(r)))
            .max_by_key(|r| r.components().count())
            .cloned();
        let root = match root {
            Some(r) => r,
            None => {
                let r = target.ancestors().last().unwrap_or(&target).to_path_buf();
                if r.as_os_str().is_empty() {
                    return Task::none();
                }
                self.roots.push(r.clone());
                r
            }
        };
        let mut chain = target
            .ancestors()
            .take_while(|p| node_key(p).starts_with(node_key(&root)))
            .map(|p| p.to_path_buf())
            .collect::<Vec<_>>();
        chain.reverse();
        let mut tasks = Vec::new();
        for ancestor in chain {
            let node = self.nodes.entry(node_key(&ancestor)).or_default();
            if !node.expanded {
                node.expanded = true;
            }
            if node.children.is_none() && !node.loading && node.error.is_none() {
                node.expanded = false;
                tasks.push(self.update(Message::Toggle(ancestor)));
            }
        }
        Task::batch(tasks)
    }
    pub fn load_labels(&self) -> Task<Message> {
        let generation = self.generation;
        let roots = self.roots.iter().skip(1).cloned().collect::<Vec<_>>();
        let (tx, rx) = iced::futures::channel::oneshot::channel();
        std::thread::spawn(move || {
            let labels = roots
                .into_iter()
                .map(|p| {
                    let label = drive_label(&p);
                    (p, label)
                })
                .collect();
            let _ = tx.send(labels);
        });
        Task::perform(async move { rx.await.unwrap_or_default() }, move |labels| {
            Message::DriveLabels(generation, labels)
        })
    }
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DriveLabels(generation, labels) if generation == self.generation => {
                self.labels.extend(labels);
            }
            Message::NetworkInput(value) => self.network_input = value,
            Message::AddNetwork => {
                let value = self.network_input.trim().replace('/', "\\");
                if value.starts_with("\\\\")
                    && value
                        .trim_start_matches('\\')
                        .split('\\')
                        .filter(|s| !s.is_empty())
                        .count()
                        >= 2
                {
                    let path = PathBuf::from(value);
                    if !self
                        .roots
                        .iter()
                        .any(|root| node_key(root) == node_key(&path))
                    {
                        self.roots.push(path);
                    }
                    self.network_input.clear();
                }
            }
            Message::CollapseAll => {
                self.reveal_path = None;
                self.keyboard_cursor = None;
                for node in self.nodes.values_mut() {
                    node.expanded = false;
                }
            }
            Message::Refresh => {
                self.generation += 1;
                self.nodes.clear();
                for path in drive_roots() {
                    if !self.roots.contains(&path) {
                        self.roots.push(path);
                    }
                }
                return Task::batch([self.load_labels(), self.resume_reveal()]);
            }
            Message::Toggle(path) => {
                let node = self.nodes.entry(node_key(&path)).or_default();
                if node.children.as_ref().is_some_and(Vec::is_empty) {
                    return Task::none();
                }
                node.expanded = !node.expanded;
                if node.expanded && !node.loading && node.children.is_none() {
                    node.loading = true;
                    node.error = None;
                    let generation = self.generation;
                    let (sender, receiver) = iced::futures::channel::oneshot::channel();
                    let worker_path = path.clone();
                    std::thread::spawn(move || {
                        let _ = sender.send(directories(&worker_path));
                    });
                    return Task::perform(
                        async move {
                            receiver
                                .await
                                .unwrap_or_else(|_| Err("Directory worker stopped".into()))
                        },
                        move |result| Message::Loaded(generation, path.clone(), result),
                    );
                }
            }
            Message::Loaded(generation, path, result) if generation == self.generation => {
                if let Some(node) = self.nodes.get_mut(&node_key(&path)) {
                    node.loading = false;
                    match result {
                        Ok(children) => {
                            if children.is_empty() {
                                node.expanded = false;
                            }
                            node.children = Some(children);
                        }
                        Err(error) => node.error = Some(error),
                    }
                }
            }
            _ => {}
        }
        Task::none()
    }
    fn flatten<'a>(&'a self, path: &'a PathBuf, depth: usize, rows: &mut Vec<TreeRow<'a>>) {
        let node = self.nodes.get(&node_key(path));
        rows.push(TreeRow::Directory(path, depth));
        if let Some(node) = node.filter(|n| n.expanded) {
            if node.loading {
                rows.push(TreeRow::Status("Loading…", depth + 1));
            } else if let Some(error) = &node.error {
                rows.push(TreeRow::Status(error, depth + 1));
            } else if let Some(children) = &node.children {
                for child in children {
                    self.flatten(child, depth + 1, rows);
                }
            }
        }
    }
    pub fn set_keyboard_cursor(&mut self, path: PathBuf) {
        self.keyboard_cursor = Some(path);
    }
    pub fn clear_keyboard_cursor(&mut self) {
        self.keyboard_cursor = None;
    }
    pub fn keyboard_cursor(&self) -> Option<&PathBuf> {
        self.keyboard_cursor.as_ref()
    }
    fn row_label(&self, path: &PathBuf) -> String {
        if path == &self.roots[0] {
            "Home".into()
        } else if let Some(label) = self.labels.get(path) {
            label.clone()
        } else {
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string())
        }
    }
    fn visible_paths(&self) -> Vec<PathBuf> {
        let mut rows = Vec::new();
        for path in &self.roots {
            self.flatten(path, 0, &mut rows);
        }
        rows.into_iter()
            .filter_map(|r| match r {
                TreeRow::Directory(p, _) => Some(p.clone()),
                _ => None,
            })
            .collect()
    }
    pub fn jump_initial(&mut self, c: char) {
        let paths = self.visible_paths();
        let start = self
            .keyboard_cursor
            .as_ref()
            .filter(|p| crate::jump_navigation::starts_with_initial(&self.row_label(p), c))
            .and_then(|p| paths.iter().position(|v| v == p))
            .map_or(0, |r| r + 1);
        if let Some(row) = (start..paths.len())
            .chain(0..start)
            .find(|r| crate::jump_navigation::starts_with_initial(&self.row_label(&paths[*r]), c))
        {
            self.keyboard_cursor = Some(paths[row].clone());
        }
    }
    pub fn arrow_selection(&mut self, direction: i8) -> Task<Message> {
        let paths = self.visible_paths();
        if paths.is_empty() {
            return Task::none();
        }
        let current = self
            .keyboard_cursor
            .as_ref()
            .and_then(|p| paths.iter().position(|v| v == p));
        let expanded = current.is_some_and(|row| {
            self.nodes
                .get(&node_key(&paths[row]))
                .is_some_and(|n| n.expanded)
        });
        let parent = current
            .and_then(|row| paths[row].parent())
            .and_then(|p| paths.iter().position(|v| v == p));
        match sk_iced::components::tree_navigation(
            paths.len(),
            current,
            direction,
            expanded,
            parent,
        ) {
            sk_iced::components::TreeNavigation::Toggle(row) => {
                return self.update(Message::Toggle(paths[row].clone()));
            }
            sk_iced::components::TreeNavigation::Select(row) => {
                self.keyboard_cursor = Some(paths[row].clone())
            }
            sk_iced::components::TreeNavigation::None => {}
        }
        Task::none()
    }
    pub fn view<'a>(
        &'a self,
        active: &'a PathBuf,
        t: Tokens,
        policy: crate::motion::Policy,
    ) -> Element<'a, crate::Message> {
        let active = self.keyboard_cursor.as_ref().unwrap_or(active);
        let mut rows = Vec::new();
        for path in &self.roots {
            self.flatten(path, 0, &mut rows);
        }
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        node_key(active).hash(&mut hasher);
        let key = hasher.finish();
        let reveal = rows
            .iter()
            .position(
                |row| matches!(row,TreeRow::Directory(path,_) if node_key(path)==node_key(active)),
            )
            .map(|i| (key, i));
        let tree = sk_iced::components::TreeView::new(
            sk_iced::components::Appearance::themed(
                crate::typography::font(),
                crate::palette::from_theme,
            ),
            rows.len(),
            self.generation,
            move |index| match rows[index] {
                TreeRow::Status(label, depth) => {
                    container(text(label).size(11).color(color(t.secondary)))
                        .padding(iced::Padding {
                            left: (depth.min(12) * 14 + 26) as f32,
                            ..Default::default()
                        })
                        .height(ROW_HEIGHT)
                        .center_y(ROW_HEIGHT)
                        .into()
                }
                TreeRow::Directory(path, depth) => {
                    let node = self.nodes.get(&node_key(path));
                    let expanded = node.is_some_and(|n| n.expanded);
                    let leaf = node.is_some_and(|n| n.children.as_ref().is_some_and(Vec::is_empty));
                    let label = self.row_label(path);
                    let selected = node_key(path) == node_key(active);
                    let arrow: Element<'a, Message> = if leaf {
                        Space::new().width(24).height(24).into()
                    } else {
                        crate::ui_icons::disclosure(expanded, t.navigation_text, 24.0)
                    };
                    let toggle = button(arrow)
                        .width(24)
                        .height(ROW_HEIGHT)
                        .padding(0)
                        .on_press_maybe((!leaf).then(|| Message::Toggle(path.clone())))
                        .style(move |_, status| iced::widget::button::Style {
                            text_color: color(
                                if expanded || status == iced::widget::button::Status::Hovered {
                                    t.accent
                                } else {
                                    t.secondary
                                },
                            ),
                            background: (status == iced::widget::button::Status::Hovered).then(
                                || blend(t.navigation_background, t.text, t.opacity_hover).into(),
                            ),
                            border: iced::Border {
                                radius: 4.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        });
                    let navigate = button(
                        container(
                            text(label.clone())
                                .size(13)
                                .wrapping(iced::widget::text::Wrapping::None),
                        )
                        .width(Fill)
                        .clip(true),
                    )
                    .height(ROW_HEIGHT)
                    .width(Fill)
                    .padding([3, 3])
                    .on_press(Message::Navigate(path.clone()))
                    .style(move |_, status| iced::widget::button::Style {
                        background: Some(
                            blend(
                                t.navigation_background,
                                t.accent,
                                if selected {
                                    0.22
                                } else if status == iced::widget::button::Status::Hovered {
                                    t.opacity_hover
                                } else {
                                    0.0
                                },
                            )
                            .into(),
                        ),
                        text_color: color(t.navigation_text),
                        border: iced::Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    });
                    let row: Element<'a, Message> =
                        sk_iced::components::TreeRow::new(toggle, navigate)
                            .depth(depth)
                            .height(ROW_HEIGHT)
                            .into();
                    crate::tab_drag::mark(
                        row.map(crate::Message::Locations),
                        crate::tab_drag::Marker::tree_directory(path.clone(), label),
                    )
                }
            },
        )
        .reveal(reveal)
        .build();
        let network: Element<'a, Message> = iced::widget::row![
            crate::typography::text_input("\\\\server\\share", &self.network_input)
                .on_input(Message::NetworkInput)
                .on_submit(Message::AddNetwork)
                .size(12)
                .padding([5, 6]),
            crate::components::tab_add(policy, Message::AddNetwork)
        ]
        .spacing(3)
        .align_y(iced::Center)
        .into();
        iced::widget::column![tree, network.map(crate::Message::Locations)]
            .spacing(6)
            .into()
    }
}
fn node_key(path: &std::path::Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    #[cfg(windows)]
    {
        let text = absolute.to_string_lossy().replace('/', "\\");
        let plain = text.strip_prefix("\\\\?\\").unwrap_or(&text);
        let plain = if let Some(unc) = plain.strip_prefix("UNC\\") {
            format!("\\\\{unc}")
        } else {
            plain.to_string()
        };
        PathBuf::from(plain.to_lowercase())
    }
    #[cfg(not(windows))]
    {
        absolute
    }
}
enum TreeRow<'a> {
    Directory(&'a PathBuf, usize),
    Status(&'a str, usize),
}
fn directories(path: &PathBuf) -> Result<Vec<PathBuf>, String> {
    let mut children = Vec::new();
    for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let metadata = entry.metadata().map_err(|e| e.to_string())?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            continue;
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                continue;
            }
        }
        children.push(entry.path());
    }
    children.sort_by_cached_key(|p| {
        p.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase()
    });
    Ok(children)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_folders_collapse_without_a_placeholder_and_cannot_expand() {
        let root = PathBuf::from("root");
        let mut tree = Locations::new(root.clone(), vec![]);
        tree.nodes.insert(
            node_key(&root),
            Node {
                expanded: true,
                loading: true,
                ..Default::default()
            },
        );
        let _ = tree.update(Message::Loaded(0, root.clone(), Ok(vec![])));
        assert!(!tree.nodes[&node_key(&root)].expanded);
        let _ = tree.update(Message::Toggle(root.clone()));
        assert!(!tree.nodes[&node_key(&root)].expanded);
        let mut rows = vec![];
        tree.flatten(&root, 0, &mut rows);
        assert_eq!(rows.len(), 1);
        assert!(matches!(rows[0], TreeRow::Directory(_, 0)));
    }
    #[test]
    fn collapse_all_keeps_cached_and_pending_children_collapsed() {
        let root = PathBuf::from("root");
        let child = root.join("child");
        let mut tree = Locations::new(root.clone(), vec![]);
        tree.nodes.insert(
            node_key(&root),
            Node {
                expanded: true,
                children: Some(vec![child.clone()]),
                ..Default::default()
            },
        );
        tree.nodes.insert(
            node_key(&child),
            Node {
                expanded: true,
                loading: true,
                ..Default::default()
            },
        );
        tree.reveal_path = Some(child.clone());
        tree.keyboard_cursor = Some(child.clone());
        let generation = tree.generation;
        let _ = tree.update(Message::CollapseAll);
        assert_eq!(tree.generation, generation);
        assert!(tree.nodes.values().all(|n| !n.expanded));
        assert!(tree.reveal_path.is_none());
        assert!(tree.keyboard_cursor.is_none());
        let _ = tree.update(Message::Loaded(
            generation,
            child.clone(),
            Ok(vec![child.join("nested")]),
        ));
        assert!(!tree.nodes[&node_key(&child)].expanded);
        assert_eq!(
            tree.nodes[&node_key(&root)].children.as_ref().unwrap(),
            &vec![child.clone()]
        );
        let _ = tree.update(Message::Toggle(child.clone()));
        assert!(tree.nodes[&node_key(&child)].expanded);
        assert!(!tree.nodes[&node_key(&child)].loading);
    }
    #[test]
    fn stale_tree_loads_are_ignored() {
        let path = PathBuf::from(".");
        let mut tree = Locations::new(path.clone(), vec![]);
        tree.nodes.insert(node_key(&path), Node::default());
        let _ = tree.update(Message::Refresh);
        let _ = tree.update(Message::Loaded(0, path.clone(), Ok(vec![path.clone()])));
        assert!(tree.nodes.is_empty());
    }
    #[test]
    fn collapsing_does_not_discard_children() {
        let path = PathBuf::from(".");
        let mut tree = Locations::new(path.clone(), vec![]);
        tree.nodes.insert(
            node_key(&path),
            Node {
                expanded: true,
                children: Some(vec![PathBuf::from("child")]),
                ..Default::default()
            },
        );
        let _ = tree.update(Message::Toggle(path.clone()));
        let mut rows = Vec::new();
        tree.flatten(&path, 0, &mut rows);
        assert_eq!(rows.len(), 1);
        drop(rows);
        let _ = tree.update(Message::Toggle(path.clone()));
        let mut rows = Vec::new();
        tree.flatten(&path, 0, &mut rows);
        assert_eq!(rows.len(), 2);
    }
}

pub fn drive_roots() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let mask = unsafe { windows_sys::Win32::Storage::FileSystem::GetLogicalDrives() };
        (0..26)
            .filter(|bit| mask & (1 << bit) != 0)
            .map(|bit| PathBuf::from(format!("{}:\\", (b'A' + bit) as char)))
            .collect()
    }
    #[cfg(not(windows))]
    {
        vec![]
    }
}

fn drive_label(path: &std::path::Path) -> String {
    let root = path.display().to_string();
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetVolumeInformationW};
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let kind = unsafe { GetDriveTypeW(wide.as_ptr()) };
        let fallback = match kind {
            2 => "Removable Disk",
            3 => "Local Disk",
            4 => "Network Drive",
            5 => "Optical Drive",
            6 => "RAM Disk",
            _ => "Drive",
        };
        let mut name = [0u16; 261];
        let label = if kind == 3
            && unsafe {
                GetVolumeInformationW(
                    wide.as_ptr(),
                    name.as_mut_ptr(),
                    name.len() as u32,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    0,
                )
            } != 0
        {
            String::from_utf16_lossy(
                &name[..name.iter().position(|&x| x == 0).unwrap_or(name.len())],
            )
        } else {
            String::new()
        };
        return format!(
            "{root} ({})",
            if label.trim().is_empty() {
                fallback
            } else {
                label.trim()
            }
        );
    }
    #[cfg(not(windows))]
    {
        format!("{root} (Drive)")
    }
}

#[cfg(test)]
mod label_tests {
    use super::*;
    #[test]
    fn stale_labels_do_not_replace_current_drive_names() {
        let root = PathBuf::from("C:\\");
        let mut tree = Locations::new(PathBuf::from("home"), vec![root.clone()]);
        tree.generation = 2;
        let _ = tree.update(Message::DriveLabels(
            1,
            vec![(root.clone(), "stale".into())],
        ));
        assert!(tree.labels.is_empty());
        let _ = tree.update(Message::DriveLabels(
            2,
            vec![(root.clone(), "C:\\ (Local Disk)".into())],
        ));
        assert_eq!(tree.labels[&root], "C:\\ (Local Disk)");
    }
}

#[cfg(test)]
mod reveal_tests {
    use super::*;
    #[test]
    fn revealing_a_location_expands_ancestors_without_collapsing_other_branches() {
        let root = PathBuf::from("C:\\");
        let home = root.join("Users/ExampleUser");
        let other = root.join("Other");
        let target = root.join("Projects/Nested");
        let mut locations = Locations::new(home, vec![root.clone()]);
        locations.nodes.insert(
            node_key(&other),
            Node {
                expanded: true,
                ..Default::default()
            },
        );
        let _ = locations.reveal(target.clone());
        assert!(locations.nodes[&node_key(&root)].expanded);
        assert!(locations.nodes[&node_key(&root.join("Projects"))].expanded);
        assert!(locations.nodes[&node_key(&target)].expanded);
        assert!(locations.nodes[&node_key(&other)].expanded);
        assert_eq!(locations.reveal_path, Some(target));
    }
}

#[cfg(test)]
mod alias_tests {
    use super::*;
    #[test]
    fn adding_network_root_does_not_orphan_inflight_branch_loads() {
        let root = PathBuf::from("C:\\");
        let mut tree = Locations::new(root.clone(), vec![]);
        tree.nodes.insert(
            node_key(&root),
            Node {
                expanded: true,
                loading: true,
                ..Default::default()
            },
        );
        let generation = tree.generation;
        let _ = tree.update(Message::NetworkInput("\\\\example\\share".into()));
        let _ = tree.update(Message::AddNetwork);
        let _ = tree.update(Message::Loaded(generation, root.clone(), Ok(vec![])));
        assert!(!tree.nodes[&node_key(&root)].loading);
        assert_eq!(tree.roots.len(), 2);
    }
    #[cfg(windows)]
    #[test]
    fn path_case_aliases_share_expanded_nodes() {
        let home = PathBuf::from("C:\\Users\\ExampleUser");
        let downloads = home.join("Downloads");
        let mut tree = Locations::new(home.clone(), vec![]);
        tree.nodes.insert(
            node_key(&home),
            Node {
                expanded: false,
                children: Some(vec![downloads.clone()]),
                ..Default::default()
            },
        );
        let _ = tree.reveal(PathBuf::from("c:\\users\\exampleuser\\downloads"));
        assert!(tree.nodes[&node_key(&home)].expanded);
        assert!(tree.nodes[&node_key(&downloads)].expanded);
        assert_eq!(tree.roots.len(), 1);
    }
}

#[cfg(test)]
mod jump_tests {
    use super::*;
    #[test]
    fn tree_jump_only_targets_visible_directory_labels() {
        let home = PathBuf::from("home");
        let apple = home.join("Apple");
        let hidden = apple.join("7 hidden");
        let banana = home.join("banana");
        let mut tree = Locations::new(home.clone(), vec![]);
        tree.nodes.insert(
            node_key(&home),
            Node {
                expanded: true,
                children: Some(vec![apple.clone(), banana.clone()]),
                ..Default::default()
            },
        );
        tree.nodes.insert(
            node_key(&apple),
            Node {
                expanded: false,
                children: Some(vec![hidden.clone()]),
                ..Default::default()
            },
        );
        tree.jump_initial('b');
        assert_eq!(tree.keyboard_cursor(), Some(&banana));
        tree.jump_initial('7');
        assert_eq!(tree.keyboard_cursor(), Some(&banana));
        tree.nodes.get_mut(&node_key(&apple)).unwrap().expanded = true;
        tree.jump_initial('7');
        assert_eq!(tree.keyboard_cursor(), Some(&hidden));
        tree.jump_initial('a');
        assert_eq!(tree.keyboard_cursor(), Some(&apple));
        tree.jump_initial('h');
        assert_eq!(tree.keyboard_cursor(), Some(&home));
    }
}

#[cfg(test)]
mod keyboard_cycle_tests {
    use super::*;
    #[test]
    fn cycle_and_arrows_visit_visible_tree_nodes() {
        let mut tree = Locations::new(
            "home".into(),
            vec!["alpha".into(), "apricot".into(), "beta".into()],
        );
        tree.jump_initial('a');
        assert_eq!(tree.keyboard_cursor(), Some(&PathBuf::from("alpha")));
        tree.jump_initial('A');
        assert_eq!(tree.keyboard_cursor(), Some(&PathBuf::from("apricot")));
        tree.jump_initial('a');
        assert_eq!(tree.keyboard_cursor(), Some(&PathBuf::from("alpha")));
        let _ = tree.arrow_selection(1);
        assert_eq!(tree.keyboard_cursor(), Some(&PathBuf::from("apricot")));
        let _ = tree.arrow_selection(-1);
        assert_eq!(tree.keyboard_cursor(), Some(&PathBuf::from("alpha")));
    }
}

#[cfg(test)]
impl Locations {
    pub(crate) fn screenshot_demo() -> Self {
        let root = PathBuf::from(r"C:\Demo");
        let mut tree = Self::new(root.clone(), vec![r"D:\Archive".into(), r"E:\Media".into()]);
        let children: Vec<PathBuf> = [
            "Projects",
            "Documents",
            "Design assets",
            "Media",
            "Releases",
            "Shared",
            "Templates",
        ]
        .into_iter()
        .map(|name| root.join(name))
        .collect();
        for path in &children {
            tree.nodes.insert(
                node_key(path),
                Node {
                    children: Some(vec![]),
                    ..Default::default()
                },
            );
        }
        tree.nodes.insert(
            node_key(&root),
            Node {
                expanded: true,
                children: Some(children),
                ..Default::default()
            },
        );
        tree.labels
            .insert(r"D:\Archive".into(), "Archive (D:)".into());
        tree.labels.insert(r"E:\Media".into(), "Media (E:)".into());
        tree
    }
}
