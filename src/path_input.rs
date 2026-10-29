//! Validate submitted paths off the UI thread; callers reject stale results.
use crate::{Message, Task};
use std::path::{Path, PathBuf};
pub(crate) fn validate(path: &Path) -> Result<PathBuf, String> {
    let path = std::path::absolute(path).map_err(|e| e.to_string())?;
    match std::fs::metadata(&path) {
        Ok(m) if m.is_dir() => Ok(path),
        Ok(_) => Err("The path is not a directory".into()),
        Err(e) => Err(e.to_string()),
    }
}
/// Expand any process environment variable, with bounded nested expansion.
fn expand_with(
    input: &str,
    mut lookup: impl FnMut(&str) -> Option<String>,
) -> Result<String, String> {
    let mut value = input.to_string();
    let mut seen = std::collections::HashSet::new();
    for _ in 0..16 {
        if !seen.insert(value.clone()) {
            return Err("Environment variable expansion contains a cycle".into());
        }
        let mut output = String::new();
        let mut rest = value.as_str();
        let mut expanded = false;
        while let Some(start) = rest.find('%') {
            output.push_str(&rest[..start]);
            let suffix = &rest[start + 1..];
            let Some(end) = suffix.find('%') else {
                output.push_str(&rest[start..]);
                rest = "";
                break;
            };
            let name = &suffix[..end];
            if let Some(replacement) = (!name.is_empty()).then(|| lookup(name)).flatten() {
                output.push_str(&replacement);
                expanded = true;
            } else {
                output.push('%');
                output.push_str(name);
                output.push('%');
            }
            rest = &suffix[end + 1..];
            if output.len() > 32768 {
                return Err("Expanded directory path is too long".into());
            }
        }
        output.push_str(rest);
        if output.len() > 32768 {
            return Err("Expanded directory path is too long".into());
        }
        if !expanded {
            return Ok(output);
        }
        value = output;
    }
    Err("Environment variables are nested too deeply".into())
}
pub(crate) fn resolve_input(input: &str, base: &Path) -> Result<PathBuf, String> {
    let expanded = expand_with(input.trim().trim_matches('"'), |name| {
        std::env::var_os(name).map(|v| v.to_string_lossy().into_owned())
    })?;
    if expanded.is_empty() {
        return Err("Enter a directory path".into());
    }
    let path = PathBuf::from(expanded);
    Ok(if path.is_absolute() {
        path
    } else {
        base.join(path)
    })
}
pub fn submit(input: String, base: PathBuf, identity: u64, request: u64) -> Task<Message> {
    let (tx, rx) = iced::futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(resolve_input(&input, &base).and_then(|path| validate(&path)));
    });
    Task::perform(
        async move {
            rx.await
                .unwrap_or_else(|_| Err("Directory validation interrupted".into()))
        },
        move |result| Message::AddressValidated(identity, request, result),
    )
}

fn search_folder(input: &str, base: &Path) -> Result<PathBuf, String> {
    resolve_input(&input.replace('"', ""), base).and_then(|path| {
        let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        if metadata.is_file() {
            validate(path.parent().ok_or("File has no parent folder")?)
        } else {
            validate(&path)
        }
    })
}

pub fn submit_search(input: String, base: PathBuf, identity: u64, request: u64) -> Task<Message> {
    let (tx, rx) = iced::futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let result = search_folder(&input, &base);
        let _ = tx.send(result);
    });
    Task::perform(
        async move {
            rx.await
                .unwrap_or_else(|_| Err("Path lookup interrupted".into()))
        },
        move |result| Message::SearchPathValidated(identity, request, result),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    #[test]
    fn validation_accepts_directories_but_rejects_files_and_missing_paths() {
        let root =
            std::env::temp_dir().join(format!("file-beholder-path-validation-{}", next_identity()));
        std::fs::create_dir_all(root.join("nested")).unwrap();
        std::fs::write(root.join("file.txt"), b"data").unwrap();
        assert!(validate(&root.join("nested")).is_ok());
        assert!(validate(&root.join("missing")).is_err());
        assert!(validate(&root.join("file.txt")).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn edits_revert_on_blur_or_invalid_submit_and_late_validation_is_ignored() {
        let mut app = App::new("original".into(), "other".into(), vec![]);
        let _ = app.update(Message::Address(0, "new draft".into()));
        assert_eq!(app.panes[0].path, PathBuf::from("original"));
        assert_eq!(app.panes[0].address, "new draft");
        let _ = app.update(Message::AddressBlur(0));
        assert_eq!(app.panes[0].address, "original");
        let identity = app.panes[0].identity;
        app.panes[0].address = "missing".into();
        app.panes[0].address_request = Some(12);
        let _ = app.update(Message::AddressValidated(
            identity,
            12,
            Err("not found".into()),
        ));
        assert_eq!(app.panes[0].address, "original");
        assert_eq!(app.panes[0].path, PathBuf::from("original"));
        assert!(app.panes[0].back.is_empty());
        app.panes[0].address = "draft".into();
        app.panes[0].address_request = Some(13);
        let _ = app.update(Message::AddressBlur(0));
        let _ = app.update(Message::AddressValidated(identity, 13, Ok("late".into())));
        assert_eq!(app.panes[0].path, PathBuf::from("original"));
    }
    #[test]
    fn validated_navigation_routes_to_the_submitting_tab_and_updates_history() {
        let root = std::env::temp_dir();
        let mut app = App::new(root.clone(), root.clone(), vec![]);
        app.preferences.scan_mode = preferences::ScanMode::Off;
        let identity = app.panes[0].identity;
        app.panes[0].address_request = Some(5);
        app.panes[0].tabs.push(Tab::new("different tab".into()));
        app.panes[0].selected_tab = 1;
        let task = app.update(Message::AddressValidated(identity, 5, Ok(root.clone())));
        assert_eq!(app.panes[0].tabs[0].path, root);
        assert!(app.panes[0].tabs[0].generation > 0);
        assert_eq!(app.panes[0].path, PathBuf::from("different tab"));
        drop(task);
    }
}

#[cfg(test)]
mod environment_tests {
    use super::*;
    fn variables(name: &str) -> Option<String> {
        match name.to_ascii_lowercase().as_str() {
            "localappdata" => Some(r"C:\Users\ExampleUser\AppData\Local".into()),
            "homedrive" => Some("C:".into()),
            "homepath" => Some(r"\Users\ExampleUser".into()),
            "custom_root" => Some("%LOCALAPPDATA%/資料".into()),
            "cycle" => Some("%CYCLE%".into()),
            "first" => Some("%SECOND%".into()),
            "second" => Some("%FIRST%".into()),
            "large" => Some("a".repeat(32769)),
            _ => None,
        }
    }
    #[test]
    fn every_named_variable_can_expand_case_insensitively_with_nested_and_adjacent_values() {
        assert_eq!(
            expand_with("%localappdata%/Folder", variables).unwrap(),
            r"C:\Users\ExampleUser\AppData\Local/Folder"
        );
        assert_eq!(
            expand_with("%HOMEDRIVE%%HOMEPATH%", variables).unwrap(),
            r"C:\Users\ExampleUser"
        );
        assert_eq!(
            expand_with("%CUSTOM_ROOT%/nested", variables).unwrap(),
            r"C:\Users\ExampleUser\AppData\Local/資料/nested"
        );
        assert_eq!(
            expand_with("100% complete/%unknown%", |_| None).unwrap(),
            "100% complete/%unknown%"
        );
        assert!(expand_with("%CYCLE%", variables).is_err());
        assert!(expand_with("%FIRST%", variables).is_err());
        assert!(expand_with("%LARGE%", variables).is_err());
    }
    #[test]
    fn real_environment_expands_before_resolving_against_the_panel_directory() {
        let base = std::env::temp_dir();
        assert_eq!(resolve_input("nested", &base).unwrap(), base.join("nested"));
        #[cfg(windows)]
        for name in [
            "LOCALAPPDATA",
            "APPDATA",
            "USERPROFILE",
            "TEMP",
            "SYSTEMROOT",
            "PROGRAMFILES",
            "PROGRAMDATA",
        ] {
            if let Some(value) = std::env::var_os(name) {
                assert_eq!(
                    resolve_input(&format!("%{}%", name.to_ascii_lowercase()), &base).unwrap(),
                    PathBuf::from(value)
                );
            }
        }
    }
}

#[cfg(test)]
mod search_path_tests {
    use super::*;
    #[test]
    fn copied_file_paths_open_the_parent_and_missing_paths_do_not_navigate() {
        let base = std::env::current_dir().unwrap();
        let file = base.join("Cargo.toml");
        assert_eq!(
            search_folder(&format!("\"{}\"", file.display()), &base).unwrap(),
            base
        );
        assert_eq!(search_folder("Cargo.toml", &base).unwrap(), base);
        assert_eq!(
            search_folder(&format!("\"{}\"", base.display()), &base).unwrap(),
            base
        );
        assert!(search_folder("missing-file-beholder-search-path", &base).is_err());
    }
}
