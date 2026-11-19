//! Короткие звуки хода и взятия.
//!
//! На Windows звук синтезируется в WAV прямо в памяти и играется через winmm —
//! без внешних файлов и аудио-библиотек. На других платформах функция тихая,
//! чтобы прототип собирался везде.

#[cfg(windows)]
mod imp {
    use std::sync::OnceLock;

    #[link(name = "winmm")]
    extern "system" {
        fn PlaySoundW(sound: *const u16, module: *mut core::ffi::c_void, flags: u32) -> i32;
    }

    const SND_ASYNC: u32 = 0x0001;
    const SND_NODEFAULT: u32 = 0x0002;
    const SND_MEMORY: u32 = 0x0004;

    const RATE: u32 = 22_050;

    static MOVE_WAV: OnceLock<Vec<u8>> = OnceLock::new();
    static CAPTURE_WAV: OnceLock<Vec<u8>> = OnceLock::new();

    /// Лёгкий щелчок на ходу.
    pub fn piece_move() {
        let wav = MOVE_WAV.get_or_init(|| wav_bytes(&move_samples()));
        play(wav);
    }

    /// Низкий удар при взятии фигуры.
    pub fn capture() {
        let wav = CAPTURE_WAV.get_or_init(|| wav_bytes(&capture_samples()));
        play(wav);
    }

    fn play(wav: &[u8]) {
        // SND_MEMORY + SND_ASYNC: буфер живёт в OnceLock, поэтому не
        // освобождается, пока звук ещё играет.
        unsafe {
            PlaySoundW(
                wav.as_ptr() as *const u16,
                core::ptr::null_mut(),
                SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
            );
        }
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
        out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out
    }

    fn move_samples() -> Vec<i16> {
        let dur = 0.065f32;
        let n = (RATE as f32 * dur) as usize;
        let mut v = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let p = t / dur;
            // короткий «клик» с чуть падающим тоном
            let freq = 720.0 + 260.0 * (1.0 - p);
            let env = (1.0 - p).powf(2.0);
            let mut s = (t * freq * std::f32::consts::TAU).sin() * env * 0.28;
            s += (t * freq * 2.0 * std::f32::consts::TAU).sin() * env * 0.10;
            v.push((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
        }
        v
    }

    fn capture_samples() -> Vec<i16> {
        let dur = 0.19f32;
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
}

#[cfg(windows)]
pub use imp::{capture, piece_move};

#[cfg(not(windows))]
pub fn piece_move() {}

#[cfg(not(windows))]
pub fn capture() {}
