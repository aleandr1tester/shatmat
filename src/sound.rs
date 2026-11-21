//! Короткие звуки игры.
//!
//! Папка `sounds` рядом с программой — точка замены: положите туда свои
//! `move.wav`, `capture.wav` и `twilight.wav`, и они будут играть вместо
//! встроенных. Если файла нет, звук синтезируется в памяти.

use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(true);
static USE_FILES: AtomicBool = AtomicBool::new(true);

/// Включает или выключает звуковые эффекты (музыка управляется отдельно).
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
    if !on {
        stop();
    }
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// true — сначала пробовать файлы из `sounds`, false — только встроенные.
pub fn set_use_files(on: bool) {
    USE_FILES.store(on, Ordering::Relaxed);
}

pub fn use_files() -> bool {
    USE_FILES.load(Ordering::Relaxed)
}

fn stop() {
    #[cfg(windows)]
    imp::stop();
}

#[cfg(windows)]
mod imp {
    use std::collections::HashMap;
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    #[link(name = "winmm")]
    extern "system" {
        fn PlaySoundW(sound: *const u16, module: *mut core::ffi::c_void, flags: u32) -> i32;
        fn mciSendStringW(
            cmd: *const u16,
            ret: *mut u16,
            ret_len: u32,
            hwnd: *mut core::ffi::c_void,
        ) -> u32;
    }

    const SND_ASYNC: u32 = 0x0001;
    const SND_NODEFAULT: u32 = 0x0002;
    const SND_MEMORY: u32 = 0x0004;
    const SND_FILENAME: u32 = 0x0008;
    // SND_LOOP намеренно не используем: каждый звук играет один раз.

    const RATE: u32 = 22_050;
    const TWILIGHT_SECS: f32 = 1.6;
    /// Имена пользовательских звуков: сначала ищется `.wav`, затем `.mp3`.
    const MOVE_FILE: &str = "move";
    const CAPTURE_FILE: &str = "capture";
    const TWILIGHT_FILE: &str = "twilight";

    static MOVE_WAV: OnceLock<Vec<u8>> = OnceLock::new();
    static CAPTURE_WAV: OnceLock<Vec<u8>> = OnceLock::new();
    static TWILIGHT_WAV: OnceLock<Vec<u8>> = OnceLock::new();
    static TWILIGHT_LAST: OnceLock<Mutex<Option<(Instant, Duration)>>> = OnceLock::new();
    static FILE_WAVS: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();

    /// Алиас отдельного MCI-канала для сумерек. Он не пересекается с PlaySound,
    /// поэтому звук хода или взятия не обрывает длинную «музыку» перехода.
    const TWILIGHT_ALIAS: &str = "shatmat_twilight";

    fn wide(cmd: &str) -> Vec<u16> {
        cmd.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn mci(cmd: &str) -> u32 {
        let cmd = wide(cmd);
        unsafe { mciSendStringW(cmd.as_ptr(), std::ptr::null_mut(), 0, std::ptr::null_mut()) }
    }

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
        let duration = match user_sound_path(&sounds_dir(), TWILIGHT_FILE) {
            Some((path, is_mp3)) => {
                prepare_user_sound(&path, is_mp3).unwrap_or_else(|| Duration::from_secs(2))
            }
            None => Duration::from_secs_f32(TWILIGHT_SECS),
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
        // Сумерки играют на своём MCI-канале, а ход/взятие — через PlaySound:
        // так короткий звук не обрывает длинную музыку перехода.
        if !play_twilight_channel() {
            play(TWILIGHT_FILE, &TWILIGHT_WAV, twilight_samples);
        }
    }

    /// Пробует проиграть сумерки отдельным каналом. Возвращает false, если
    /// MCI не смог открыть файл — тогда вызывающий сыграет встроенный звук.
    fn play_twilight_channel() -> bool {
        let Some((path, is_mp3)) = user_sound_path(&sounds_dir(), TWILIGHT_FILE) else {
            return false;
        };
        let play_path = if is_mp3 {
            // MP3 MCI играет напрямую устройством mpegvideo.
            path
        } else {
            // WAV: убираем loop-метаданные (smpl/inst); если файл уже чистый,
            // играем его как есть.
            let Ok(raw) = std::fs::read(&path) else {
                return false;
            };
            let Some(clean) = sanitize_wav(&raw) else {
                return false;
            };
            if clean.len() == raw.len() && clean == raw {
                path
            } else {
                let temp = std::env::temp_dir().join("shatmat_twilight_play.wav");
                if std::fs::write(&temp, &clean).is_err() {
                    return false;
                }
                temp
            }
        };
        let kind = if is_mp3 { "mpegvideo" } else { "waveaudio" };
        let _ = mci(&format!("close {TWILIGHT_ALIAS}"));
        let cmd = format!(
            "open \"{}\" type {} alias {}",
            play_path.to_string_lossy().replace('"', ""),
            kind,
            TWILIGHT_ALIAS
        );
        if mci(&cmd) != 0 {
            return false;
        }
        if mci(&format!("play {TWILIGHT_ALIAS} from 0")) != 0 {
            let _ = mci(&format!("close {TWILIGHT_ALIAS}"));
            return false;
        }
        true
    }

    /// Декодирует пользовательский звук один раз и кладёт в кэш.
    /// Возвращает длительность готового WAV.
    fn prepare_user_sound(path: &Path, is_mp3: bool) -> Option<Duration> {
        let key = path.to_string_lossy().to_string();
        let files = FILE_WAVS.get_or_init(|| Mutex::new(HashMap::new()));
        {
            let guard = files.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(bytes) = guard.get(&key) {
                return wav_duration(bytes);
            }
        }
        let raw = std::fs::read(path).ok()?;
        let clean = if is_mp3 {
            mp3_to_wav(&raw)
        } else {
            sanitize_wav(&raw)
        }?;
        let duration = wav_duration(&clean);
        let mut guard = files.lock().unwrap_or_else(|e| e.into_inner());
        guard.insert(key, clean);
        duration
    }

    fn play(file: &str, cache: &'static OnceLock<Vec<u8>>, samples: fn() -> Vec<i16>) {
        if !super::enabled() {
            return;
        }
        if super::use_files() {
            if let Some((path, is_mp3)) = user_sound_path(&sounds_dir(), file) {
                let key = path.to_string_lossy().to_string();
                let files = FILE_WAVS.get_or_init(|| Mutex::new(HashMap::new()));
                {
                    let guard = files.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(bytes) = guard.get(&key) {
                        play_memory(bytes);
                        return;
                    }
                }
                // Играем пользовательский файл из памяти, а не по имени: так
                // PlaySound не может зациклить файл по loop-метаданным WAV.
                if let Ok(raw) = std::fs::read(&path) {
                    let wav = if is_mp3 {
                        // MP3 декодируется в WAV в памяти: PlaySound играет
                        // только PCM, зато формат файла не важен пользователю.
                        mp3_to_wav(&raw)
                    } else {
                        sanitize_wav(&raw)
                    };
                    if let Some(clean) = wav {
                        let mut guard = files.lock().unwrap_or_else(|e| e.into_inner());
                        guard.insert(key.clone(), clean);
                        if let Some(bytes) = guard.get(&key) {
                            play_memory(bytes);
                        }
                        return;
                    }
                }
                if !is_mp3 {
                    // Если это не RIFF/WAV, всё равно пробуем как файл — один раз.
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
                // MP3 не декодировался — ниже сыграем встроенный звук.
            }
        }
        let wav = cache.get_or_init(|| wav_bytes(&samples()));
        play_memory(wav);
    }

    fn play_memory(bytes: &[u8]) {
        unsafe {
            PlaySoundW(
                bytes.as_ptr() as *const u16,
                core::ptr::null_mut(),
                SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
            );
        }
    }

    /// Убирает из WAV chunk'и smpl/inst с точками цикла, чтобы файл
    /// гарантированно играл один раз, как встроенный звук.
    fn sanitize_wav(bytes: &[u8]) -> Option<Vec<u8>> {
        if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
            return None;
        }
        let mut out = Vec::with_capacity(bytes.len());
        out.extend_from_slice(&bytes[0..12]);
        let mut pos = 12;
        while pos + 8 <= bytes.len() {
            let id = &bytes[pos..pos + 4];
            let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().ok()?) as usize;
            let end = pos + 8 + size;
            if end > bytes.len() {
                break;
            }
            if id != b"smpl" && id != b"inst" {
                out.extend_from_slice(&bytes[pos..end]);
                if size & 1 == 1 && end < bytes.len() {
                    out.push(bytes[end]);
                }
            }
            pos = end + (size & 1);
        }
        if out.len() < 12 {
            return None;
        }
        let riff_size = (out.len() - 8) as u32;
        out[4..8].copy_from_slice(&riff_size.to_le_bytes());
        Some(out)
    }

    /// Ищет пользовательский звук: сначала `.mp3`, затем `.wav` с тем же именем.
    /// MP3 в приоритете, иначе стандартный `move.wav` всегда перекрывал бы
    /// пользовательский `move.mp3`.
    /// Возвращает путь и признак «это MP3».
    fn user_sound_path(dir: &Path, name: &str) -> Option<(PathBuf, bool)> {
        let mp3 = dir.join(format!("{name}.mp3"));
        if mp3.is_file() {
            return Some((mp3, true));
        }
        let wav = dir.join(format!("{name}.wav"));
        if wav.is_file() {
            return Some((wav, false));
        }
        None
    }

    /// Отрезает ID3v2-тег в начале и ID3v1-тег в конце файла.
    /// puremp3 ищет кадры с первого байта, а в тегах встречаются байты 0xFF,
    /// похожие на синхрослово, — из-за них декодер может «сломаться» на теге.
    fn strip_id3(raw: &[u8]) -> &[u8] {
        let mut start = 0usize;
        if raw.len() >= 10 && &raw[0..3] == b"ID3" {
            let size = ((raw[6] as usize & 0x7f) << 21)
                | ((raw[7] as usize & 0x7f) << 14)
                | ((raw[8] as usize & 0x7f) << 7)
                | (raw[9] as usize & 0x7f);
            let mut skip = 10 + size;
            if raw[5] & 0x10 != 0 {
                skip += 10; // футер ID3v2.4
            }
            start = skip.min(raw.len());
        }
        let mut end = raw.len();
        if end >= 128 && &raw[end - 128..end - 125] == b"TAG" {
            end -= 128;
        }
        if start < end {
            &raw[start..end]
        } else {
            raw
        }
    }

    /// Декодирует MP3 в WAV 16-бит/моно/22,05 кГц — тот же формат, что у
    /// встроенных звуков и у пользовательских WAV. Так PlaySound может играть
    /// любой пользовательский файл, независимо от исходного формата.
    fn mp3_to_wav(raw: &[u8]) -> Option<Vec<u8>> {
        let raw = strip_id3(raw);
        let (header, frames) = puremp3::read_mp3(raw).ok()?;
        let src_rate = header.sample_rate.hz().max(1) as f32;
        let step = src_rate / RATE as f32;
        let mut mono: Vec<f32> = Vec::new();
        for (l, r) in frames {
            mono.push((l + r) * 0.5);
        }
        if mono.is_empty() {
            return None;
        }
        let out_len = ((mono.len() as f32) / step).ceil() as usize;
        let mut out: Vec<i16> = Vec::with_capacity(out_len);
        for i in 0..out_len {
            let pos = i as f32 * step;
            let idx = pos.floor() as usize;
            let frac = pos - idx as f32;
            let a = mono.get(idx).copied().unwrap_or(0.0);
            let b = mono.get(idx + 1).copied().unwrap_or(a);
            let s = a + (b - a) * frac;
            out.push((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
        }
        Some(wav_bytes(&out))
    }

    /// Останавливает текущий звук (при выключении эффектов).
    pub fn stop() {
        unsafe {
            PlaySoundW(core::ptr::null(), core::ptr::null_mut(), 0);
        }
        let _ = mci(&format!("close {TWILIGHT_ALIAS}"));
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
        let _ = std::fs::write(dir.join("move.wav"), wav_bytes(&move_samples()));
        let _ = std::fs::write(dir.join("capture.wav"), wav_bytes(&capture_samples()));
        let _ = std::fs::write(dir.join("twilight.wav"), wav_bytes(&twilight_samples()));
    }

    fn write_missing(dir: &Path, name: &str, samples: &[i16]) {
        // Если пользователь положил MP3 с тем же именем, стандартный WAV
        // не создаём: иначе он перекрыл бы пользовательский звук.
        if dir.join(format!("{name}.mp3")).exists() {
            return;
        }
        let path = dir.join(format!("{name}.wav"));
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

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn mp3_decodes_to_playable_wav() {
            let Ok(raw) = std::fs::read("tests/vectors/mono_cbr192.mp3") else {
                return; // тестовый файл мог не попасть в архив исходников
            };
            let wav = mp3_to_wav(&raw).expect("MP3 декодируется");
            assert_eq!(&wav[0..4], b"RIFF", "на выходе WAV");
            assert_eq!(&wav[8..12], b"WAVE", "на выходе WAV");
            assert!(
                sanitize_wav(&wav).is_some(),
                "декодированный WAV проходит очистку"
            );
            let duration = wav_duration(&wav).expect("длительность читается");
            assert!(
                duration.as_secs_f32() > 1.0,
                "звук не пустой: {duration:?}"
            );
        }

        #[test]
        fn id3_tag_is_skipped_before_decoding() {
            let Ok(mp3) = std::fs::read("tests/vectors/mono_cbr192.mp3") else {
                return;
            };
            // Синтетический ID3v2.3 с UTF-16 BOM: байты 0xFF в теге
            // выглядят как синхрослово и раньше ломали декодер.
            let mut tagged = Vec::new();
            tagged.extend_from_slice(b"ID3");
            tagged.extend_from_slice(&[3, 0, 0]);
            let size = 64usize;
            tagged.extend_from_slice(&[
                ((size >> 21) & 0x7f) as u8,
                ((size >> 14) & 0x7f) as u8,
                ((size >> 7) & 0x7f) as u8,
                (size & 0x7f) as u8,
            ]);
            tagged.extend_from_slice(b"TIT2");
            tagged.extend_from_slice(&[0, 0, 0, 6]);
            tagged.extend_from_slice(&[0xFF, 0xFE, b'T', 0, b'x', 0]);
            tagged.resize(10 + size, 0);
            tagged.extend_from_slice(&mp3);
            let wav = mp3_to_wav(&tagged).expect("MP3 с ID3-тегом декодируется");
            assert_eq!(&wav[0..4], b"RIFF");
            assert!(wav_duration(&wav).is_some());
        }

        #[test]
        fn user_sound_path_prefers_mp3() {
            let dir = std::env::temp_dir().join("shatmat_sound_test");
            let _ = std::fs::create_dir_all(&dir);
            let wav = dir.join("move.wav");
            let mp3 = dir.join("move.mp3");
            let _ = std::fs::write(&wav, b"RIFF");
            let _ = std::fs::write(&mp3, b"ID3");
            let found = user_sound_path(&dir, "move").expect("файл найден");
            assert!(found.1, "MP3 в приоритете");
            assert_eq!(found.0.file_name().unwrap(), "move.mp3");
            let _ = std::fs::remove_file(&wav);
            let _ = std::fs::remove_file(&mp3);
        }

        #[test]
        fn sanitize_removes_loop_chunks() {
            let mut wav = wav_bytes(&[0i16; 100]);
            wav.extend_from_slice(b"smpl");
            wav.extend_from_slice(&4u32.to_le_bytes());
            wav.extend_from_slice(&[0u8; 4]);
            let size = (wav.len() - 8) as u32;
            wav[4..8].copy_from_slice(&size.to_le_bytes());
            assert!(wav.windows(4).any(|w| w == b"smpl"));
            let clean = sanitize_wav(&wav).expect("valid wav");
            assert!(!clean.windows(4).any(|w| w == b"smpl"));
            assert_eq!(
                u32::from_le_bytes(clean[4..8].try_into().unwrap()) as usize,
                clean.len() - 8
            );
        }
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
