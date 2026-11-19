//! Фоновая музыка.
//!
//! Играет через MCI отдельным каналом, поэтому короткие звуки ходов её не
//! останавливают. Файл: `music/theme.wav` рядом с программой.

#[cfg(windows)]
mod imp {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[link(name = "winmm")]
    extern "system" {
        fn mciSendStringW(
            cmd: *const u16,
            ret: *mut u16,
            ret_len: u32,
            hwnd: *mut core::ffi::c_void,
        ) -> u32;
    }

    static ENABLED: AtomicBool = AtomicBool::new(false);

    fn send(cmd: &str) {
        let wide: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            mciSendStringW(
                wide.as_ptr(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
            );
        }
    }

    fn music_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        if let Some(dir) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        {
            dirs.push(dir.join("music"));
        }
        dirs.push(PathBuf::from("music"));
        dirs
    }

    fn music_path() -> Option<PathBuf> {
        music_dirs()
            .into_iter()
            .map(|d| d.join("theme.wav"))
            .find(|p| p.is_file())
    }

    pub fn available() -> bool {
        music_path().is_some()
    }

    pub fn enabled() -> bool {
        ENABLED.load(Ordering::Relaxed)
    }

    pub fn set_enabled(on: bool) {
        ENABLED.store(on, Ordering::Relaxed);
        send("stop shatmat_music");
        send("close shatmat_music");
        if !on {
            return;
        }
        let Some(path) = music_path() else {
            return;
        };
        let path = path.to_string_lossy().replace('"', "");
        send(&format!(
            "open \"{}\" type waveaudio alias shatmat_music",
            path
        ));
        send("setaudio shatmat_music volume to 260");
        send("play shatmat_music repeat");
    }
}

#[cfg(windows)]
pub use imp::{available, enabled, set_enabled};

#[cfg(not(windows))]
pub fn available() -> bool {
    false
}

#[cfg(not(windows))]
pub fn enabled() -> bool {
    false
}

#[cfg(not(windows))]
pub fn set_enabled(_on: bool) {}
