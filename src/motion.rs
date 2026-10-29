//! Shared motion policy and animation primitives.
pub use sk_iced::animation::primitives::*;

/// Cached detection of the PE/Recovery environment, independent of drive letters.
pub fn windows_pe() -> bool {
    static PE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *PE.get_or_init(|| {
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::System::Registry::*;
            let path: Vec<u16> = "SYSTEM\\CurrentControlSet\\Control\\MiniNT\0"
                .encode_utf16()
                .collect();
            let mut key = std::ptr::null_mut();
            let status = RegOpenKeyExW(HKEY_LOCAL_MACHINE, path.as_ptr(), 0, KEY_READ, &mut key);
            if status == 0 {
                RegCloseKey(key);
            }
            // Access denied still establishes that the marker exists.
            status == 0 || status == windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED
        }
        #[cfg(not(windows))]
        {
            false
        }
    })
}

pub fn detect_policy() -> Policy {
    Policy {
        system_enabled: !windows_pe()
            && sk_iced::motion::system_enabled(sk_iced::motion::QueryFallback::Disabled),
    }
}
