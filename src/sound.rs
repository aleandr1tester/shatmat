//! Короткие звуки игры.
//!
//! Папка `sounds` рядом с программой — точка замены: положите туда свои
//! `move.wav`, `capture.wav` и `twilight.wav`, и они будут играть вместо
//! встроенных. Если файла нет, звук синтезируется в памяти.

#[cfg(windows)]
mod imp {
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    #[link(name = "winmm")]
    extern "system" {
        fn PlaySoundW(sound: *const u16, module: *mut core::ffi::c_void, flags: u32) -> i32;
    }

    const SND_ASYNC: u32 = 0x0001;
    const SND_NODEFAULT: u32 = 0x0002;
    const SND_MEMORY: u32 = 0x0004;
    const SND_FILENAME: u32 = 0x0008;
    // SND_LOOP намеренно не используем: каждый звук играет один раз.

    const RATE: u32 = 22_050;
    const TWILIGHT_SECS: f32 = 1.6;
    const MOVE_FILE: &str = "move.wav";
    const CAPTURE_FILE: &str = "capture.wav";
    const TWILIGHT_FILE: &str = "twilight.wav";

    static MOVE_WAV: OnceLock<Vec<u8>> = OnceLock::new();
    static CAPTURE_WAV: OnceLock<Vec<u8>> = OnceLock::new();
    static TWILIGHT_WAV: OnceLock<Vec<u8>> = OnceLock::new();
    static TWILIGHT_LAST: OnceLock<Mutex<Option<(Instant, Duration)>>> = OnceLock::new();

    /// Короткий одиночный «тык» — фигуру поставили.
    pub fn piece_move() {
        play(MOVE_FILE, &MOVE_WAV, move_samples);
    }

    /// Низкий удар при взятии.
    pub fn capture() {
        play(CAPTURE_FILE, &CAPTURE_WAV, capture_samples);
    }

    /// Долгий шум-волна при сумерках.
    pub fn twilight() {
        // Не перезапускаем длинный звук, пока он ещё играет: иначе при
        // быстрой смене фаз он звучал бы повторно.
        let path = sounds_dir().join(TWILIGHT_FILE);
        let duration = if path.is_file() {
            std::fs::read(&path)
                .ok()
                .and_then(|wav| wav_duration(&wav))
                .unwrap_or_else(|| Duration::from_secs(2))
        } else {
            Duration::from_secs_f32(TWILIGHT_SECS)
        };
        let last = TWILIGHT_LAST.get_or_init(|| Mutex::new(None));
        let mut guard = last.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((at, d)) = *guard {
            if at.elapsed() < d {
                return;
            }
        }
        *guard = Some((Instant::now(), duration));
        drop(guard);
        play(TWILIGHT_FILE, &TWILIGHT_WAV, twilight_samples);
    }

    fn play(file: &str, cache: &'static OnceLock<Vec<u8>>, samples: fn() -> Vec<i16>) {
        let path = sounds_dir().join(file);
        if path.is_file() {
            let wide: Vec<u16> = path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            unsafe {
                PlaySoundW(
                    wide.as_ptr(),
                    core::ptr::null_mut(),
                    SND_FILENAME | SND_ASYNC | SND_NODEFAULT,
                );
            }
            return;
        }
        let wav = cache.get_or_init(|| wav_bytes(&samples()));
        unsafe {
            PlaySoundW(
                wav.as_ptr() as *const u16,
                core::ptr::null_mut(),
                SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
            );
        }
    }

    /// Создаёт папку `sounds` и недостающие стандартные файлы.
    pub fn prepare() {
        let dir = sounds_dir();
        let _ = std::fs::create_dir_all(&dir);
        write_missing(&dir, MOVE_FILE, &move_samples());
        write_missing(&dir, CAPTURE_FILE, &capture_samples());
        write_missing(&dir, TWILIGHT_FILE, &twilight_samples());
    }

    /// Записывает все стандартные звуки (для сборки релиза).
    pub fn write_defaults(dir: &Path) {
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(dir.join(MOVE_FILE), wav_bytes(&move_samples()));
        let _ = std::fs::write(dir.join(CAPTURE_FILE), wav_bytes(&capture_samples()));
        let _ = std::fs::write(dir.join(TWILIGHT_FILE), wav_bytes(&twilight_samples()));
    }

    fn write_missing(dir: &Path, name: &str, samples: &[i16]) {
        let path = dir.join(name);
        if !path.exists() {
            let _ = std::fs::write(path, wav_bytes(samples));
        }
    }

    fn sounds_dir() -> PathBuf {
        if let Some(dir) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        {
            let p = dir.join("sounds");
            if p.is_dir() || std::fs::create_dir_all(&p).is_ok() {
                return p;
            }
        }
        PathBuf::from("sounds")
    }

    fn wav_duration(wav: &[u8]) -> Option<Duration> {
        if wav.len() < 12 || &wav[0..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
            return None;
        }
        let mut pos = 12;
        let mut byte_rate = None;
        while pos + 8 <= wav.len() {
            let id = &wav[pos..pos + 4];
            let size = u32::from_le_bytes(wav[pos + 4..pos + 8].try_into().ok()?) as usize;
            let body = pos + 8;
            if id == b"fmt " && size >= 12 && body + 12 <= wav.len() {
                byte_rate = Some(u32::from_le_bytes(
                    wav[body + 8..body + 12].try_into().ok()?,
                ));
            } else if id == b"data" {
                if let Some(br) = byte_rate {
                    if br > 0 {
                        return Some(Duration::from_secs_f32(size as f32 / br as f32));
                    }
                }
                return None;
            }
            pos = body + size + (size & 1);
        }
        None
    }

    fn wav_bytes(samples: &[i16]) -> Vec<u8> {
        let data_len = (samples.len() * 2) as u32;
        let mut out = Vec::with_capacity(44 + data_len as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&1u16.to_le_bytes()); // mono
        out.extend_from_slice(&RATE.to_le_bytes());
        out.extend_from_slice(&(RATE * 2).to_le_bytes()); // byte rate
        out.extend_from_slice(&2u16.to_le_bytes()); // block align
        out.extend_from_slice(&16u16.to_le_bytes()); // bits
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out
    }

    fn move_samples() -> Vec<i16> {
        // 32 мс: щелчок + короткий тон с быстрым затуханием.
        let dur = 0.032f32;
        let n = (RATE as f32 * dur) as usize;
        let mut v = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let p = t / dur;
            let click = if p < 0.18 {
                let x = ((i * 1_103_515_245 + 12_345) & 0x7fff) as f32 / 32767.0;
                (x * 2.0 - 1.0) * (1.0 - p / 0.18) * 0.55
            } else {
                0.0
            };
            let tone = (t * 1_150.0 * std::f32::consts::TAU).sin() * (-t * 95.0).exp() * 0.38;
            v.push(((click + tone).clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
        }
        v
    }

    fn capture_samples() -> Vec<i16> {
        let dur = 0.16f32;
        let n = (RATE as f32 * dur) as usize;
        let mut v = Vec::with_capacity(n);
        let mut noise = 0x1234_5678u32;
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let p = t / dur;
            let freq = 190.0 - 90.0 * p;
            let env = (1.0 - p).powf(1.5);
            let thud = (t * freq * std::f32::consts::TAU).sin() * env * 0.42;
            noise = noise.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let nz = ((noise >> 16) as f32 / 32768.0) - 1.0;
            let click = if p < 0.08 {
                nz * (1.0 - p / 0.08) * 0.30
            } else {
                0.0
            };
            v.push(((thud + click).clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
        }
        v
    }

    fn twilight_samples() -> Vec<i16> {
        // 1,6 с: нарастающая и стихающая шумовая волна с низким гулом.
        let dur = TWILIGHT_SECS;
        let n = (RATE as f32 * dur) as usize;
        let mut v = Vec::with_capacity(n);
        let mut noise = 0x9E37_79B9_7F4A_7C15u64;
        let mut lp = 0.0f32;
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let p = t / dur;
            noise = noise
                .wrapping_mul(636_413_622_384_679_3005)
                .wrapping_add(1_442_695_040_888_963_407);
            let white = ((noise >> 33) as f32 / 2_147_483_648.0) - 1.0;
            lp += (white - lp) * 0.025;
            let swell = (std::f32::consts::PI * p).sin().powf(1.3);
            let gust = 0.65 + 0.35 * (t * 2.1).sin();
            let low = (t * 65.0 * std::f32::consts::TAU).sin() * 0.14 * swell;
            let s = (lp * 2.6 * swell * gust + low) * 0.9;
            v.push((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
        }
        v
    }
}

#[cfg(windows)]
pub use imp::{capture, piece_move, prepare, twilight, write_defaults};

#[cfg(not(windows))]
pub fn piece_move() {}

#[cfg(not(windows))]
pub fn capture() {}

#[cfg(not(windows))]
pub fn twilight() {}

#[cfg(not(windows))]
pub fn prepare() {}

#[cfg(not(windows))]
pub fn write_defaults(_dir: &std::path::Path) {}
