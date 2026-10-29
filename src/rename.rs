mod view;
use crate::*;
pub struct Editor {
    pub id: u64,
    pub source: PathBuf,
    pub name: String,
    pub error: Option<String>,
    pub busy: bool,
    pub new_folder: bool,
}
pub(crate) fn destination(source: &std::path::Path, name: &str) -> Result<PathBuf, String> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.ends_with([' ', '.'])
        || name.chars().any(|c| c < ' ' || "<>:\"/\\|?*".contains(c))
    {
        return Err("Enter a valid file or folder name without path separators or trailing spaces or periods.".into());
    }
    let stem = name
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end()
        .to_ascii_uppercase();
    if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&stem.as_str())
        || ["COM", "LPT"].iter().any(|p| {
            stem.strip_prefix(p).is_some_and(|n| {
                ["1", "2", "3", "4", "5", "6", "7", "8", "9", "¹", "²", "³"].contains(&n)
            })
        })
    {
        return Err("That name is reserved by Windows.".into());
    }
    if name.encode_utf16().count() > 255 {
        return Err("The name is too long.".into());
    }
    Ok(source
        .parent()
        .ok_or("Cannot rename a drive root.")?
        .join(name))
}
pub(crate) fn rename_item(
    source: &std::path::Path,
    target: &std::path::Path,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
        let target: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
        // MoveFileW never replaces an existing destination.
        if unsafe {
            windows_sys::Win32::Storage::FileSystem::MoveFileW(source.as_ptr(), target.as_ptr())
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        if target.exists() {
            return Err("An item with that name already exists.".into());
        }
        std::fs::rename(source, target).map_err(|e| e.to_string())
    }
}
impl App {
    pub fn begin_rename(&mut self) -> Task<Message> {
        if self.rename.is_some() || self.jobs.iter().any(|j| j.report.is_none()) {
            self.notice = "Finish queued transfers before renaming.".into();
            return Task::none();
        }
        let p = &self.panes[self.active];
        if p.selections.len() != 1 {
            self.notice = "Select one file or folder to rename.".into();
            return Task::none();
        }
        let Some(entry) = p.selections.iter().next().and_then(|i| p.entries.get(*i)) else {
            return Task::none();
        };
        self.rename = Some(Editor {
            id: next_identity(),
            source: entry.path.clone(),
            name: entry.name.clone(),
            error: None,
            busy: false,
            new_folder: false,
        });
        self.context = None;
        let end = selection_end(&entry.name, entry.directory);
        iced::widget::operation::focus("rename-name").chain(iced::widget::operation::select_range(
            "rename-name",
            0,
            end,
        ))
    }
    pub fn begin_new_folder(&mut self) -> Task<Message> {
        if self.rename.is_some() || self.undo_busy {
            return Task::none();
        }
        self.rename = Some(Editor {
            id: next_identity(),
            source: self.panes[self.active].path.join("New folder"),
            name: "New folder".into(),
            error: None,
            busy: false,
            new_folder: true,
        });
        self.context = None;
        iced::widget::operation::focus("rename-name")
            .chain(iced::widget::operation::select_all("rename-name"))
    }
    pub fn remap_renamed_tabs(&mut self, from: &std::path::Path, to: &std::path::Path) {
        for p in &mut self.panes {
            for tab in &mut p.tabs {
                if let Ok(suffix) = tab.path.strip_prefix(from) {
                    tab.path = to.join(suffix);
                    tab.address = tab.path.display().to_string();
                }
                for path in tab.back.iter_mut().chain(tab.forward.iter_mut()) {
                    if let Ok(suffix) = path.strip_prefix(from) {
                        *path = to.join(suffix);
                    }
                }
            }
        }
    }
    pub fn confirm_rename(&mut self, accept: bool) -> Task<Message> {
        let Some(editor) = &mut self.rename else {
            return Task::none();
        };
        if editor.busy {
            return Task::none();
        }
        if !accept {
            self.rename = None;
            return Task::none();
        }
        let target = match destination(&editor.source, &editor.name) {
            Ok(p) => p,
            Err(e) => {
                editor.error = Some(e);
                return Task::none();
            }
        };
        if !editor.new_folder && target == editor.source {
            self.rename = None;
            return Task::none();
        }
        editor.busy = true;
        let source = editor.source.clone();
        let id = editor.id;
        let new_folder = editor.new_folder;
        let (tx, rx) = iced::futures::channel::oneshot::channel();
        std::thread::spawn(move || {
            let _ = tx.send(perform_edit(&source, &target, new_folder));
        });
        Task::perform(
            async move {
                rx.await
                    .unwrap_or_else(|_| Err("Rename worker stopped.".into()))
            },
            move |r| Message::RenameFinished(id, r),
        )
    }
    pub fn finish_rename(
        &mut self,
        id: u64,
        result: Result<Option<String>, String>,
    ) -> Task<Message> {
        let Some(editor) = &mut self.rename else {
            return Task::none();
        };
        if editor.id != id {
            return Task::none();
        }
        let operation = next_identity();
        let target = destination(&editor.source, &editor.name).ok();
        let source = if editor.new_folder {
            target.clone().unwrap_or_else(|| editor.source.clone())
        } else {
            editor.source.clone()
        };
        let kind = if editor.new_folder {
            "New folder"
        } else {
            "Rename"
        };
        let status = match &result {
            Ok(_) => "Completed".to_string(),
            Err(e) => format!("Failed: {e}"),
        };
        self.activities.push(activity::Record {
            operation,
            time: browser::format_modified(Some(std::time::SystemTime::now())),
            kind: kind.into(),
            source,
            target,
            status: status.clone(),
            completed: result.is_ok(),
        });
        let warning = match result {
            Ok(warning) => warning,
            Err(e) => {
                editor.busy = false;
                editor.error = Some(e);
                return Task::none();
            }
        };
        let editor = self.rename.take().unwrap();
        let target = destination(&editor.source, &editor.name).unwrap();
        if !editor.new_folder {
            self.remap_renamed_tabs(&editor.source, &target);
        }
        self.notice = warning.unwrap_or_else(|| {
            format!(
                "{} {}",
                if editor.new_folder {
                    "Created"
                } else {
                    "Renamed to"
                },
                editor.name
            )
        });
        self.highlights.request(&target, self.motion);
        self.history_generation = next_identity();
        let refresh = self.refresh_affected(&[editor.source, target.clone()]);
        for p in &mut self.panes {
            for tab in &mut p.tabs {
                if target.parent().is_some_and(|parent| {
                    path_utils::same(parent, &tab.path)
                        || (tab.filters.recursive && path_utils::related(parent, &tab.path))
                }) {
                    tab.pending_selection = Some(target.clone());
                }
            }
        }
        Task::batch([refresh, interactions::history_task(self.history_generation)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_windows_names_and_paths_are_rejected() {
        for name in [
            "", ".", "..", "../other", "a/b", "a\\b", "C:other", "bad?", "name.", "name ", "CON",
            "nul.txt", "COM1", "LPT9.log", "LPT¹", "bad\0",
        ] {
            assert!(
                destination(std::path::Path::new("parent/file"), name).is_err(),
                "{name:?}"
            );
        }
        assert!(destination(std::path::Path::new("parent/file"), "résumé.txt").is_ok());
    }
    #[test]
    fn renames_unicode_files_and_folders_without_overwriting_collisions() {
        let dir = std::env::current_dir().unwrap().join(format!(
            "target/rename-fixture-{}-{}",
            std::process::id(),
            next_identity()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("old.txt");
        let target = dir.join("résumé.txt");
        std::fs::write(&source, b"source").unwrap();
        std::fs::write(&target, b"existing").unwrap();
        assert!(rename_item(&source, &target).is_err());
        assert_eq!(std::fs::read(&source).unwrap(), b"source");
        assert_eq!(std::fs::read(&target).unwrap(), b"existing");
        let renamed = dir.join("新しい.txt");
        rename_item(&source, &renamed).unwrap();
        assert!(!source.exists());
        assert_eq!(std::fs::read(&renamed).unwrap(), b"source");
        let folder = dir.join("folder");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("child"), b"nested").unwrap();
        let new_folder = dir.join("renamed folder");
        rename_item(&folder, &new_folder).unwrap();
        assert_eq!(std::fs::read(new_folder.join("child")).unwrap(), b"nested");
        std::fs::remove_dir_all(dir).unwrap();
    }
}

fn selection_end(name: &str, directory: bool) -> usize {
    let end = if directory {
        name.len()
    } else {
        name.rfind('.').filter(|i| *i > 0).unwrap_or(name.len())
    };
    // Iced cursor positions count grapheme clusters.
    iced::widget::text_input::Value::new(&name[..end]).len()
}
fn perform_edit(
    source: &std::path::Path,
    target: &std::path::Path,
    new_folder: bool,
) -> Result<Option<String>, String> {
    let mut batch = undo::Batch::new(if new_folder { "New folder" } else { "Rename" })?;
    if new_folder {
        std::fs::create_dir(target).map_err(|e| e.to_string())?;
        batch.record(undo::Action::RemoveEmpty {
            path: target.to_owned(),
            created: undo::created(target),
        });
    } else {
        let identity = undo::rename_identity(source)?;
        rename_item(source, target)?;
        batch.record(undo::Action::RenameBack {
            from: target.to_owned(),
            to: source.to_owned(),
            identity,
        });
    }
    Ok(batch.finish(false))
}
impl Tab {
    pub fn restore_pending_selection(&mut self) {
        let Some(path) = self.pending_selection.clone() else {
            return;
        };
        if let Some(index) = self
            .entries
            .iter()
            .position(|e| path_utils::same(&path, &e.path))
        {
            if let Some(row) = self.order.iter().position(|i| *i == index) {
                self.selected = Some(index);
                self.selections.clear();
                self.selections.insert(index);
                self.reveal_selection = Some((next_identity(), row));
                self.pending_selection = None;
            }
        }
    }
}

#[cfg(test)]
mod workflow_tests {
    use super::*;
    #[test]
    fn rename_selects_only_the_stem_and_folders_select_the_whole_name() {
        assert_eq!(selection_end("report.txt", false), 6);
        assert_eq!(selection_end("archive.tar.gz", false), 11);
        assert_eq!(selection_end(".gitignore", false), 10);
        assert_eq!(selection_end("folder.name", true), 11);
        assert_eq!(selection_end("résumé.txt", false), 6);
        assert_eq!(selection_end("e\u{301}.txt", false), 1);
    }
    #[test]
    fn new_folder_never_reuses_an_existing_name_and_is_undoable() {
        let dir = std::env::current_dir()
            .unwrap()
            .join(format!("target/new-folder-test-{}", next_identity()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("New folder");
        assert!(perform_edit(&target, &target, true).is_ok());
        std::fs::write(target.join("kept"), b"content").unwrap();
        assert!(perform_edit(&target, &target, true).is_err());
        assert_eq!(std::fs::read(target.join("kept")).unwrap(), b"content");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
