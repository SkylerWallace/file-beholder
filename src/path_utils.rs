//! Lexical path comparison for UI refreshes, including paths that Undo just removed.
use std::path::Path;
pub(crate) fn key(path: &Path) -> String {
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    #[cfg(windows)]
    {
        let value = path.to_string_lossy().replace('/', "\\").to_lowercase();
        let value = if let Some(rest) = value.strip_prefix(r"\\?\unc\") {
            format!(r"\\{}", rest)
        } else {
            value.strip_prefix(r"\\?\").unwrap_or(&value).to_string()
        };
        value.trim_end_matches('\\').to_lowercase()
    }
    #[cfg(not(windows))]
    {
        path.to_string_lossy().trim_end_matches('/').to_string()
    }
}
pub fn same(a: &Path, b: &Path) -> bool {
    key(a) == key(b)
}
pub fn related(a: &Path, b: &Path) -> bool {
    let a = key(a);
    let b = key(b);
    let separator = if cfg!(windows) { '\\' } else { '/' };
    a == b
        || a.strip_prefix(&b)
            .is_some_and(|rest| rest.starts_with(separator))
        || b.strip_prefix(&a)
            .is_some_and(|rest| rest.starts_with(separator))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn path_relations_respect_components() {
        assert!(related(
            Path::new("target/folder"),
            Path::new("target/folder/file.txt")
        ));
        assert!(!related(
            Path::new("target/folder"),
            Path::new("target/folder-other/file.txt")
        ));
        #[cfg(windows)]
        {
            assert!(related(
                Path::new(r"C:\Users\ExampleUser"),
                Path::new(r"\\?\c:\users\exampleuser\file.txt")
            ));
            assert!(related(
                Path::new(r"\\server\share\folder"),
                Path::new(r"\\?\UNC\SERVER\SHARE\folder\file.txt")
            ));
            assert!(!related(
                Path::new(r"C:\foo"),
                Path::new(r"\\?\C:\foobar\file.txt")
            ));
        }
    }
}
