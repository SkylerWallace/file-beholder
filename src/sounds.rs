// SPDX-License-Identifier: MIT
//! Completion tones and bounded PCM WAV handling.
use crate::{Message, Task, preferences::SoundChoice};
use std::{io::Read, path::PathBuf};
static PLAYBACK: std::sync::Mutex<()> = std::sync::Mutex::new(());
pub fn play(choice: SoundChoice, custom: &str) -> Result<(), String> {
    if choice == SoundChoice::Off {
        return Ok(());
    }
    let _guard = PLAYBACK.lock().map_err(|e| e.to_string())?;
    let bytes = match choice {
        SoundChoice::Warm => include_bytes!("../assets/audio/warm_confirm.wav").to_vec(),
        SoundChoice::Crystal => include_bytes!("../assets/audio/crystal_ripple.wav").to_vec(),
        SoundChoice::Custom => {
            if custom.is_empty()
                || PathBuf::from(custom).file_name().and_then(|s| s.to_str()) != Some(custom)
                || custom.contains(['/', '\\'])
            {
                return Err("Choose a custom WAV first.".into());
            }
            load_wav(&executable_directory().join(custom))?
        }
        SoundChoice::Off => return Ok(()),
    };
    #[cfg(windows)]
    {
        #[link(name = "winmm")]
        unsafe extern "system" {
            fn PlaySoundW(sound: *const u16, module: *mut std::ffi::c_void, flags: u32) -> i32;
        }
        if unsafe { PlaySoundW(bytes.as_ptr().cast(), std::ptr::null_mut(), 4 | 2) } == 0 {
            return Err("Completion sound was unavailable.".into());
        }
    }
    #[cfg(not(windows))]
    {
        let _ = bytes;
        return Err("Sound playback is available on Windows.".into());
    }
    #[allow(unreachable_code)]
    Ok(())
}
fn executable_directory() -> PathBuf {
    std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("."))
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf()
}
fn load_wav(path: &std::path::Path) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    bounded_wav(&bytes)
}
fn import(path: &std::path::Path) -> Result<String, String> {
    use std::io::Write;
    let bytes = load_wav(path)?;
    let name = format!(
        "file-beholder-sound-{}-{}.wav",
        std::process::id(),
        crate::next_identity()
    );
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(executable_directory().join(&name))
        .map_err(|e| e.to_string())?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(name)
}
pub fn choose_task() -> Task<Message> {
    let (tx, rx) = iced::futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let result = choose().and_then(|path| path.map(|p| import(&p)).transpose());
        let _ = tx.send(result);
    });
    Task::perform(
        async move {
            rx.await
                .unwrap_or_else(|_| Err("Sound selection stopped.".into()))
        },
        Message::SoundChosen,
    )
}
pub fn preview_task(choice: SoundChoice, custom: String) -> Task<Message> {
    let (tx, rx) = iced::futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(play(choice, &custom));
    });
    Task::perform(
        async move {
            rx.await
                .unwrap_or_else(|_| Err("Sound preview stopped.".into()))
        },
        Message::SoundPreviewed,
    )
}
#[cfg(windows)]
fn choose() -> Result<Option<PathBuf>, String> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::UI::Controls::Dialogs::*;
    let filter: Vec<u16> = "PCM WAV audio (*.wav)\0*.wav\0\0".encode_utf16().collect();
    let title: Vec<u16> = "Choose completion sound"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut path = vec![0u16; 32768];
    let mut info: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    info.lStructSize = std::mem::size_of_val(&info) as u32;
    info.lpstrFilter = filter.as_ptr();
    info.lpstrTitle = title.as_ptr();
    info.lpstrFile = path.as_mut_ptr();
    info.nMaxFile = path.len() as u32;
    info.Flags =
        OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR | OFN_EXPLORER | OFN_HIDEREADONLY;
    if unsafe { GetOpenFileNameW(&mut info) } != 0 {
        let n = path.iter().position(|c| *c == 0).unwrap_or(path.len());
        return Ok(Some(std::ffi::OsString::from_wide(&path[..n]).into()));
    }
    let error = unsafe { CommDlgExtendedError() };
    if error == 0 {
        Ok(None)
    } else {
        Err(format!("Could not open WAV picker ({error})."))
    }
}
#[cfg(not(windows))]
fn choose() -> Result<Option<PathBuf>, String> {
    Err("WAV selection is available on Windows.".into())
}
pub fn bounded_wav(input: &[u8]) -> Result<Vec<u8>, String> {
    let fail =
        || "Choose a valid PCM WAV (8/16-bit, mono/stereo, 8–96 kHz), at most 16 MiB.".to_owned();
    let read = |at: usize, n: usize| -> Result<u32, String> {
        let b = input.get(at..at + n).ok_or_else(fail)?;
        Ok(b.iter()
            .enumerate()
            .fold(0, |v, (i, &x)| v | (x as u32) << (i * 8)))
    };
    if input.len() < 44
        || input.len() > 16 * 1024 * 1024
        || input.get(..4) != Some(b"RIFF")
        || input.get(8..12) != Some(b"WAVE")
        || read(4, 4)? as u64 + 8 != input.len() as u64
    {
        return Err(fail());
    }
    let (mut rate, mut channels, mut bits, mut data, mut length) = (0, 0, 0, 0, 0);
    let mut offset = 12usize;
    while offset < input.len() {
        if input.len() - offset < 8 {
            return Err(fail());
        }
        let size = read(offset + 4, 4)? as usize;
        if size > input.len() - offset - 8 {
            return Err(fail());
        }
        match &input[offset..offset + 4] {
            b"fmt " => {
                if rate != 0 || size < 16 || read(offset + 8, 2)? != 1 {
                    return Err(fail());
                }
                channels = read(offset + 10, 2)?;
                rate = read(offset + 12, 4)?;
                bits = read(offset + 22, 2)?;
                if !matches!(channels, 1 | 2)
                    || !matches!(bits, 8 | 16)
                    || !(8000..=96000).contains(&rate)
                    || read(offset + 20, 2)? != channels * bits / 8
                    || read(offset + 16, 4)? != rate * channels * bits / 8
                {
                    return Err(fail());
                }
            }
            b"data" => {
                if data != 0 || size == 0 {
                    return Err(fail());
                }
                data = offset + 8;
                length = size;
            }
            _ => {}
        }
        offset += 8 + size + (size & 1);
        if offset > input.len() {
            return Err(fail());
        }
    }
    if rate == 0 || data == 0 || length % (channels * bits / 8) as usize != 0 {
        return Err(fail());
    }
    length = length.min((rate * channels * bits / 8 * 5) as usize);
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(36 + length as u32 + (length as u32 & 1)).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    for v in [1u16, channels as u16] {
        out.extend_from_slice(&v.to_le_bytes())
    }
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * channels * bits / 8).to_le_bytes());
    out.extend_from_slice(&((channels * bits / 8) as u16).to_le_bytes());
    out.extend_from_slice(&(bits as u16).to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(length as u32).to_le_bytes());
    out.extend_from_slice(&input[data..data + length]);
    if length & 1 != 0 {
        out.push(0)
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_tones_are_valid_and_bad_wavs_are_rejected() {
        for wav in [
            include_bytes!("../assets/audio/warm_confirm.wav").as_slice(),
            include_bytes!("../assets/audio/crystal_ripple.wav").as_slice(),
        ] {
            assert_eq!(bounded_wav(wav).unwrap(), wav);
        }
        assert!(bounded_wav(b"not a wav").is_err());
        let mut wav = include_bytes!("../assets/audio/warm_confirm.wav").to_vec();
        wav[20] = 3;
        assert!(bounded_wav(&wav).is_err());
        assert!(play(SoundChoice::Off, "").is_ok());
        assert!(play(SoundChoice::Custom, "../elsewhere.wav").is_err());
    }
    #[test]
    fn importing_sound_copies_validated_audio_without_changing_source() {
        let source = std::env::current_dir().unwrap().join(format!(
            "target/sound-fixture-{}-{}.wav",
            std::process::id(),
            crate::next_identity()
        ));
        let wav = include_bytes!("../assets/audio/crystal_ripple.wav");
        std::fs::write(&source, wav).unwrap();
        let name = import(&source).unwrap();
        assert_eq!(std::fs::read(&source).unwrap(), wav);
        let copy = executable_directory().join(name);
        assert_eq!(std::fs::read(&copy).unwrap(), wav);
        std::fs::remove_file(source).unwrap();
        std::fs::remove_file(copy).unwrap();
    }
    #[test]
    fn choices_migrate_and_roundtrip_and_cancellation_preserves_settings() {
        use crate::preferences::Preferences;
        assert_eq!(
            Preferences::decode("completion_sound=true").sound_choice(),
            SoundChoice::Warm
        );
        assert_eq!(Preferences::decode("").sound_choice(), SoundChoice::Off);
        assert!(
            Preferences::decode("custom_sound=../outside.wav")
                .custom_sound
                .is_empty()
        );
        for choice in SoundChoice::ALL {
            let mut p = Preferences::default();
            p.completion_sound = choice != SoundChoice::Off;
            p.completion_tone = choice;
            assert_eq!(Preferences::decode(&p.encode()).sound_choice(), choice);
        }
        let mut app = crate::App::new("home".into(), "other".into(), vec![]);
        app.preferences.completion_sound = true;
        app.preferences.completion_tone = SoundChoice::Crystal;
        app.sound_busy = true;
        let _ = app.update(Message::SoundChosen(Ok(None)));
        assert!(!app.sound_busy);
        assert_eq!(app.preferences.sound_choice(), SoundChoice::Crystal);
        let _ = app.update(Message::SoundChosen(Err("bad file".into())));
        assert_eq!(app.preferences.sound_choice(), SoundChoice::Crystal);
    }
}
