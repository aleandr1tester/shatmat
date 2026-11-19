//! Фоновая музыка.
//!
//! Играет через MCI отдельным каналом, поэтому короткие звуки ходов её не
//! останавливают. Файл: `music/theme.wav` рядом с программой.
//!
//! Важно: waveaudio-драйвер MCI не понимает `play ... repeat` — команда
//! возвращает ошибку 259 и музыка вообще не стартует. Поэтому включаем
//! обычный `play`, а когда трек заканчивается, `tick` запускает его снова.

#[cfg(windows)]
mod imp {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

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
    static LAST_TICK: OnceLock<Mutex<Instant>> = OnceLock::new();

    fn wide(cmd: &str) -> Vec<u16> {
        cmd.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn send(cmd: &str) -> u32 {
        let cmd = wide(cmd);
        unsafe {
            mciSendStringW(
                cmd.as_ptr(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
            )
        }
    }

    fn query(cmd: &str) -> String {
        let cmd = wide(cmd);
        let mut buf = [0u16; 128];
        let rc = unsafe {
            mciSendStringW(
                cmd.as_ptr(),
                buf.as_mut_ptr(),
                buf.len() as u32,
                std::ptr::null_mut(),
            )
        };
        if rc != 0 {
            return String::new();
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..len])
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
        // Без repeat: waveaudio не поддерживает этот параметр.
        send("play shatmat_music from 0");
        let last = LAST_TICK.get_or_init(|| Mutex::new(Instant::now()));
        if let Ok(mut t) = last.lock() {
            *t = Instant::now();
        }
    }

    /// Вызывается из кадра: когда трек закончился, запускает его заново.
    pub fn tick() {
        if !ENABLED.load(Ordering::Relaxed) {
            return;
        }
        let last = LAST_TICK.get_or_init(|| Mutex::new(Instant::now() - Duration::from_secs(1)));
        let Ok(mut guard) = last.lock() else {
            return;
        };
        if guard.elapsed() < Duration::from_millis(400) {
            return;
        }
        *guard = Instant::now();
        let mode = query("status shatmat_music mode");
        if mode == "stopped" {
            send("play shatmat_music from 0");
        }
    }
}

#[cfg(windows)]
pub use imp::{available, enabled, set_enabled, tick};

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

#[cfg(not(windows))]
pub fn tick() {}
