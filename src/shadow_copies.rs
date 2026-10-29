//! Browse existing VSS snapshots through their device paths; never create or mount one.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ShadowCopy {
    pub device: PathBuf,
    pub drive: String,
    /// Local time, formatted by Windows as YYYY-MM-DD HH:mm:ss.
    pub date: String,
}
impl ShadowCopy {
    pub fn root(&self) -> PathBuf {
        self.device.join("")
    }
    pub fn contains(&self, path: &Path) -> bool {
        #[cfg(windows)]
        {
            let value = path.to_string_lossy().replace('/', "\\").to_lowercase();
            let root = self
                .device
                .to_string_lossy()
                .trim_end_matches('\\')
                .to_lowercase();
            value.trim_end_matches('\\') == root
                || value
                    .strip_prefix(&root)
                    .is_some_and(|rest| rest.starts_with('\\'))
        }
        #[cfg(not(windows))]
        {
            path.starts_with(&self.device)
        }
    }
    pub fn valid(&self) -> bool {
        let name = self.device.to_string_lossy();
        name.strip_prefix(r"\\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy")
            .is_some_and(|suffix| {
                !suffix.trim_end_matches('\\').is_empty()
                    && suffix
                        .trim_end_matches('\\')
                        .chars()
                        .all(|c| c.is_ascii_digit())
            })
            && self.date.len() >= 10
    }
    pub fn tab_label(&self) -> String {
        format!(
            "VSS {} {}",
            self.drive.trim_end_matches('\\'),
            self.date.get(..10).unwrap_or(&self.date)
        )
    }
    pub fn description(&self) -> String {
        format!("Shadow copy · {} · {} (local time)", self.drive, self.date)
    }
}
pub struct Picker {
    pub panel: usize,
    pub request: u64,
    pub result: Option<Result<Vec<ShadowCopy>, String>>,
}
pub fn load(request: u64) -> iced::Task<crate::Message> {
    let (send, receive) = iced::futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let _ = send.send(enumerate());
    });
    iced::Task::perform(
        async move {
            receive
                .await
                .unwrap_or_else(|_| Err("Shadow copy worker stopped".into()))
        },
        move |result| crate::Message::ShadowsLoaded(request, result),
    )
}
#[cfg(windows)]
fn enumerate() -> Result<Vec<ShadowCopy>, String> {
    use std::os::windows::process::CommandExt;
    // Literal script only. No paths or UI text are interpolated into shell code.
    const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
try {
    $volumes = @(Get-CimInstance -ClassName Win32_Volume -OperationTimeoutSec 15)
    $copies = @(Get-CimInstance -ClassName Win32_ShadowCopy -OperationTimeoutSec 15)
    $rows = @($copies | Sort-Object InstallDate -Descending | ForEach-Object {
        $copy = $_
        $volume = $volumes | Where-Object { $_.DeviceID -eq $copy.VolumeName } | Select-Object -First 1
        $drive = if ($volume.DriveLetter) { $volume.DriveLetter } elseif ($volume.Name) { $volume.Name } else { $copy.VolumeName }
        @{ device = $copy.DeviceObject; drive = $drive; date = $copy.InstallDate.ToLocalTime().ToString('yyyy-MM-dd HH:mm:ss') }
    })
    ConvertTo-Json -InputObject $rows -Compress
} catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    exit 1
}
"#;
    let system = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .ok_or("Windows directory is unavailable")?;
    let output =
        std::process::Command::new(system.join("System32/WindowsPowerShell/v1.0/powershell.exe"))
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                SCRIPT,
            ])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .output()
            .map_err(|error| format!("Could not list shadow copies: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Could not list shadow copies. Administrator access may be required. {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    decode(&output.stdout)
}
#[cfg(not(windows))]
fn enumerate() -> Result<Vec<ShadowCopy>, String> {
    Err("Shadow copies are available on Windows only.".into())
}
fn decode(bytes: &[u8]) -> Result<Vec<ShadowCopy>, String> {
    let copies: Vec<ShadowCopy> = serde_json::from_slice(bytes)
        .map_err(|error| format!("Could not read shadow copy information: {error}"))?;
    if copies.iter().any(|copy| !copy.valid()) {
        return Err("Windows returned an invalid shadow copy path or date.".into());
    }
    Ok(copies)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_paths_dates_and_compact_labels_are_preserved() {
        let copies = decode(br#"[{"device":"\\\\?\\GLOBALROOT\\Device\\HarddiskVolumeShadowCopy42","drive":"C:","date":"2026-10-24 10:30:00"}]"#).unwrap();
        let copy = &copies[0];
        assert_eq!(copy.tab_label(), "VSS C: 2026-10-24");
        assert!(copy.contains(&copy.root().join("Users/file.txt")));
        assert!(!copy.contains(Path::new(r"C:\Users")));
        assert!(!copy.contains(Path::new(
            r"\\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy420\Users"
        )));
        assert!(decode(b"[]").unwrap().is_empty());
        assert!(decode(br#"[{"device":"C:\\","drive":"C:","date":"2026-10-24"}]"#).is_err());
    }
}
