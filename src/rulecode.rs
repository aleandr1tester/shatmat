//! Код правил — компактное буквенно-цифровое представление игровых настроек.
//!
//! Правила пакуются в 31 бит и защищаются контрольной суммой CRC8, затем
//! кодируются в 8 символов base32 (алфавит без похожих букв I, L, O, U).
//! Код отображается как `XXXX-XXXX`: его удобно продиктовать или скопировать.
//! Ввод нормализуется: регистр любой, дефисы и пробелы игнорируются,
//! O→0, I→1, L→1. Ошибка в одном символе отсеивается контрольной суммой.

use crate::game::{Game, Phase};

/// Версия формата кода: меняется, если состав правил изменится.
const CODE_VERSION: u8 = 1;

/// Base32 без похожих символов (Crockford-алфавит).
const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Параметры правил, достаточные для воспроизведения партии.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rules {
    /// Смена фаз: true — по ходам, false — по времени.
    pub twilight_by_moves: bool,
    /// Случайная длительность фаз (режим «по времени»).
    pub auto_phase: bool,
    /// Длительность фазы шахмат, 5–60 с.
    pub chess_len: f32,
    /// Длительность фазы шашек, 5–60 с.
    pub checkers_len: f32,
    /// Максимум случайной длительности фазы, 10–60 с.
    pub auto_max: f32,
    /// Время на один ход в режиме «по ходам», 10–180 с.
    pub move_time_limit: f32,
    /// Превращение пешки: true — выбор фигуры, false — всегда ферзь.
    pub promotion_choice: bool,
    /// Обязательное взятие в шашках.
    pub mandatory_capture: bool,
    /// Дамки: true — превращаются все фигуры, false — только пешки.
    pub crown_non_pawns: bool,
}

impl Rules {
    /// Текущие правила партии.
    pub fn from_game(game: &Game) -> Self {
        Self {
            twilight_by_moves: game.twilight_by_moves,
            auto_phase: game.auto_phase,
            chess_len: game.chess_len,
            checkers_len: game.checkers_len,
            auto_max: game.auto_max,
            move_time_limit: game.move_time_limit,
            promotion_choice: game.promotion_choice,
            mandatory_capture: game.mandatory_capture,
            crown_non_pawns: game.crown_non_pawns,
        }
    }

    /// Применяет правила к партии: сначала лимит хода, затем режим смены фаз,
    /// чтобы новые значения вступили в силу сразу.
    pub fn apply(&self, game: &mut Game) {
        game.set_move_time_limit(self.move_time_limit);
        game.set_twilight_by_moves(self.twilight_by_moves);
        game.set_len(Phase::Chess, self.chess_len);
        game.set_len(Phase::Checkers, self.checkers_len);
        game.set_auto_max(self.auto_max);
        game.set_auto(self.auto_phase);
        game.promotion_choice = self.promotion_choice;
        game.mandatory_capture = self.mandatory_capture;
        game.crown_non_pawns = self.crown_non_pawns;
    }

    /// Краткое описание правил для сообщения «применено».
    pub fn summary(&self) -> String {
        let mode = if self.twilight_by_moves {
            crate::i18n::trf("по ходам, {secs} с", &[("secs", &fmt(self.move_time_limit))])
        } else if self.auto_phase {
            crate::i18n::trf(
                "по времени, случайно 10–{max} с",
                &[("max", &fmt(self.auto_max))],
            )
        } else {
            crate::i18n::trf(
                "по времени, шахматы {chess} с, шашки {checkers} с",
                &[("chess", &fmt(self.chess_len)), ("checkers", &fmt(self.checkers_len))],
            )
        };
        crate::i18n::trf("Смена фаз: {mode}", &[("mode", &mode)])
    }
}

fn fmt(secs: f32) -> String {
    format!("{:.0}", secs)
}

/// Упаковывает правила в 31 бит payload.
fn pack(r: &Rules) -> u32 {
    let chess = (r.chess_len.round().clamp(5.0, 60.0) - 5.0) as u32;
    let checkers = (r.checkers_len.round().clamp(5.0, 60.0) - 5.0) as u32;
    let auto_max = (r.auto_max.round().clamp(10.0, 60.0) - 10.0) as u32;
    let mtl = (r.move_time_limit.round().clamp(10.0, 180.0) - 10.0) as u32;
    let mut v: u32 = 0;
    v |= r.twilight_by_moves as u32;
    v |= (r.auto_phase as u32) << 1;
    v |= (r.promotion_choice as u32) << 2;
    v |= (r.mandatory_capture as u32) << 3;
    v |= (r.crown_non_pawns as u32) << 4;
    v |= chess << 5;
    v |= checkers << 11;
    v |= auto_max << 17;
    v |= mtl << 23;
    v
}

/// Распаковывает payload в правила.
fn unpack(v: u32) -> Rules {
    Rules {
        twilight_by_moves: v & 1 != 0,
        auto_phase: (v >> 1) & 1 != 0,
        promotion_choice: (v >> 2) & 1 != 0,
        mandatory_capture: (v >> 3) & 1 != 0,
        crown_non_pawns: (v >> 4) & 1 != 0,
        chess_len: (((v >> 5) & 0x3F) as f32) + 5.0,
        checkers_len: (((v >> 11) & 0x3F) as f32) + 5.0,
        auto_max: (((v >> 17) & 0x3F) as f32) + 10.0,
        move_time_limit: (((v >> 23) & 0xFF) as f32) + 10.0,
    }
}

/// CRC8-полином 0x07, начальное значение 0x29.
fn crc8(data: &[u8]) -> u8 {
    let mut crc: u8 = 0x29;
    for &b in data {
        crc ^= b;
        for _ in 0..8 {
            crc = if crc & 0x80 != 0 { (crc << 1) ^ 0x07 } else { crc << 1 };
        }
    }
    crc
}

/// Кодирует правила в строку вида `XXXX-XXXX`.
pub fn encode(r: &Rules) -> String {
    let payload = pack(r);
    let bytes = [CODE_VERSION, payload as u8, (payload >> 8) as u8, (payload >> 16) as u8, (payload >> 24) as u8];
    let crc = crc8(&bytes);
    let bits = (payload as u64) | ((crc as u64) << 31);
    let mut chars = [0u8; 8];
    for (i, c) in chars.iter_mut().enumerate() {
        *c = ALPHABET[((bits >> (i * 5)) & 0x1F) as usize];
    }
    let s = String::from_utf8_lossy(&chars).into_owned();
    format!("{}-{}", &s[..4], &s[4..])
}

/// Нормализует введённый код: верхний регистр, O→0, I→1, L→1,
/// дефисы и пробелы удаляются. Возвращает None, если остались посторонние
/// символы или длина не 8.
fn normalize(input: &str) -> Option<[u8; 8]> {
    let mut out = [0u8; 8];
    let mut n = 0;
    for ch in input.chars() {
        let up = ch.to_ascii_uppercase();
        let up = match up {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        };
        match up {
            '-' | ' ' => continue,
            '0'..='9' | 'A'..='Z' => {
                if n >= 8 {
                    return None;
                }
                out[n] = up as u8;
                n += 1;
            }
            _ => return None,
        }
    }
    if n == 8 {
        Some(out)
    } else {
        None
    }
}

/// Декодирует код в правила. None — код повреждён или неверной длины.
pub fn decode(input: &str) -> Option<Rules> {
    let chars = normalize(input)?;
    let mut bits: u64 = 0;
    for (i, &c) in chars.iter().enumerate() {
        let pos = ALPHABET.iter().position(|&a| a == c)?;
        bits |= (pos as u64) << (i * 5);
    }
    // Старший бит 39 должен быть нулём.
    if bits >= 1 << 39 {
        return None;
    }
    let payload = (bits & 0x7FFF_FFFF) as u32;
    let crc = ((bits >> 31) & 0xFF) as u8;
    let bytes = [CODE_VERSION, payload as u8, (payload >> 8) as u8, (payload >> 16) as u8, (payload >> 24) as u8];
    if crc8(&bytes) != crc {
        return None;
    }
    Some(unpack(payload))
}

/// Кодирует произвольные байты в base32 (Crockford-алфавит, без паддинга).
pub fn base32(bytes: &[u8]) -> String {
    let mut bits: u64 = 0;
    let mut nbits: u32 = 0;
    let mut out = String::new();
    for &b in bytes {
        bits = (bits << 8) | b as u64;
        nbits += 8;
        while nbits >= 5 {
            nbits -= 5;
            out.push(ALPHABET[((bits >> nbits) & 0x1F) as usize] as char);
        }
    }
    if nbits > 0 {
        out.push(ALPHABET[((bits << (5 - nbits)) & 0x1F) as usize] as char);
    }
    out
}

/// Декодирует 8-символьный идентификатор игрока в 5 байт.
/// None — посторонние символы или неверная длина.
pub fn decode_id(input: &str) -> Option<[u8; 5]> {
    let chars = normalize(input)?;
    let mut bits: u64 = 0;
    for &c in &chars {
        let pos = ALPHABET.iter().position(|&a| a == c)?;
        bits = (bits << 5) | pos as u64;
    }
    let mut out = [0u8; 5];
    for (i, b) in out.iter_mut().enumerate() {
        *b = ((bits >> (32 - i * 8)) & 0xFF) as u8;
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// Компактный код настроек для кода приглашения: 4 символа, 20 бит
// ---------------------------------------------------------------------------

/// Упаковывает правила в 18 бит (контейнер 20 бит = 4 символа base32).
///
/// Плотнее, чем `pack`: длины фаз — с шагом 5 с, время хода — с шагом 10 с.
/// «Максимум случайности» едет только в режиме «по времени», а в режиме
/// «по ходам» на его место — время хода. Неиспользуемые поля при разборе
/// получают значения по умолчанию. Точные правила всё равно приходят в
/// рукопожатии — компактный код нужен лишь для предпросмотра в приглашении.
fn pack20(r: &Rules) -> u32 {
    let chess = (((r.chess_len.round().clamp(5.0, 60.0) - 5.0) / 5.0).round().clamp(0.0, 11.0)) as u32;
    let checkers =
        (((r.checkers_len.round().clamp(5.0, 60.0) - 5.0) / 5.0).round().clamp(0.0, 11.0)) as u32;
    let mtl =
        (((r.move_time_limit.round().clamp(10.0, 180.0) - 10.0) / 10.0).round().clamp(0.0, 17.0))
            as u32;
    let auto = (((r.auto_max.round().clamp(10.0, 60.0) - 10.0) / 10.0).round().clamp(0.0, 5.0))
        as u32;
    let mut v: u32 = 0;
    v |= r.twilight_by_moves as u32;
    v |= (r.auto_phase as u32) << 1;
    v |= (r.promotion_choice as u32) << 2;
    v |= (r.mandatory_capture as u32) << 3;
    v |= (r.crown_non_pawns as u32) << 4;
    v |= chess << 5; // 4 бита: 5..9
    v |= checkers << 9; // 4 бита: 9..13
    if r.twilight_by_moves {
        v |= mtl << 13; // 5 бит: 13..18
    } else {
        v |= auto << 13; // 3 бита: 13..16
    }
    v
}

/// Разбирает компактный код в правила (см. `pack20`).
fn unpack20(v: u32) -> Rules {
    let by_moves = v & 1 != 0;
    Rules {
        twilight_by_moves: by_moves,
        auto_phase: (v >> 1) & 1 != 0,
        promotion_choice: (v >> 2) & 1 != 0,
        mandatory_capture: (v >> 3) & 1 != 0,
        crown_non_pawns: (v >> 4) & 1 != 0,
        chess_len: ((((v >> 5) & 0xF) as f32) * 5.0) + 5.0,
        checkers_len: ((((v >> 9) & 0xF) as f32) * 5.0) + 5.0,
        auto_max: if by_moves {
            60.0
        } else {
            ((((v >> 13) & 0x7) as f32) * 10.0) + 10.0
        },
        move_time_limit: if by_moves {
            ((((v >> 13) & 0x1F) as f32) * 10.0) + 10.0
        } else {
            60.0
        },
    }
}

/// Компактный код настроек для кода приглашения: ровно 4 символа.
///
/// Без контрольной суммы: ошибка в приглашении не страшна — точные правила
/// приходят в рукопожатии, а блок настроек здесь служит предпросмотром.
pub fn encode_compact(r: &Rules) -> String {
    let v = pack20(r);
    let mut out = [0u8; 4];
    for (i, c) in out.iter_mut().enumerate() {
        *c = ALPHABET[((v >> (i * 5)) & 0x1F) as usize];
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Разбирает компактный код настроек. None — посторонние символы или длина.
pub fn decode_compact(input: &str) -> Option<Rules> {
    let mut vals = [0u8; 4];
    let mut n = 0;
    for ch in input.chars() {
        let up = ch.to_ascii_uppercase();
        let up = match up {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        };
        match up {
            '-' | ' ' => continue,
            '0'..='9' | 'A'..='Z' => {
                if n >= 4 {
                    return None;
                }
                let pos = ALPHABET.iter().position(|&a| a == up as u8)?;
                vals[n] = pos as u8;
                n += 1;
            }
            _ => return None,
        }
    }
    if n != 4 {
        return None;
    }
    let mut v: u32 = 0;
    for (i, &p) in vals.iter().enumerate() {
        v |= (p as u32) << (i * 5);
    }
    Some(unpack20(v))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_rules() -> Rules {
        let mut g = Game::new(20.0);
        g.checkers_len = 10.0;
        Rules::from_game(&g)
    }

    #[test]
    fn code_roundtrip_defaults() {
        let r = default_rules();
        let code = encode(&r);
        assert_eq!(code.len(), 9, "код XXXX-XXXX");
        let back = decode(&code).expect("код по умолчанию декодируется");
        assert_eq!(back, r);
    }

    #[test]
    fn compact_code_roundtrip() {
        // «По ходам»: время хода и длины фаз восстанавливаются точно.
        let r = Rules {
            twilight_by_moves: true,
            auto_phase: false,
            chess_len: 25.0,
            checkers_len: 10.0,
            auto_max: 60.0,
            move_time_limit: 60.0,
            promotion_choice: false,
            mandatory_capture: true,
            crown_non_pawns: true,
        };
        let code = encode_compact(&r);
        assert_eq!(code.chars().count(), 4, "компактный код — 4 символа");
        let back = decode_compact(&code).expect("компактный код декодируется");
        assert_eq!(back.move_time_limit, 60.0);
        assert_eq!(back.chess_len, 25.0);
        assert_eq!(back.checkers_len, 10.0);
        assert!(back.mandatory_capture && back.crown_non_pawns);
        // «По времени, случайно»: восстанавливается максимум случайности.
        let r2 = Rules {
            twilight_by_moves: false,
            auto_phase: true,
            chess_len: 60.0,
            checkers_len: 45.0,
            auto_max: 30.0,
            move_time_limit: 60.0,
            promotion_choice: true,
            mandatory_capture: false,
            crown_non_pawns: false,
        };
        let back2 = decode_compact(&encode_compact(&r2)).unwrap();
        assert_eq!(back2.auto_max, 30.0);
        assert_eq!(back2.chess_len, 60.0);
        assert_eq!(back2.checkers_len, 45.0);
        assert!(back2.promotion_choice);
        // Неиспользуемое в этом режиме поле — значение по умолчанию.
        assert_eq!(back2.move_time_limit, 60.0);
    }

    #[test]
    fn code_roundtrip_all_variants() {
        let variants = [
            Rules {
                twilight_by_moves: false,
                auto_phase: false,
                chess_len: 5.0,
                checkers_len: 60.0,
                auto_max: 10.0,
                move_time_limit: 10.0,
                promotion_choice: true,
                mandatory_capture: false,
                crown_non_pawns: false,
            },
            Rules {
                twilight_by_moves: true,
                auto_phase: true,
                chess_len: 60.0,
                checkers_len: 5.0,
                auto_max: 60.0,
                move_time_limit: 180.0,
                promotion_choice: false,
                mandatory_capture: true,
                crown_non_pawns: true,
            },
            Rules {
                twilight_by_moves: false,
                auto_phase: true,
                chess_len: 37.0,
                checkers_len: 23.0,
                auto_max: 45.0,
                move_time_limit: 95.0,
                promotion_choice: true,
                mandatory_capture: false,
                crown_non_pawns: true,
            },
        ];
        for r in variants {
            let code = encode(&r);
            let back = decode(&code).expect("декодируется");
            assert_eq!(back, r, "код {}", code);
        }
    }

    #[test]
    fn code_rejects_corrupted_char() {
        let r = default_rules();
        let code = encode(&r);
        // Портим второй символ: меняем на соседний по алфавиту.
        let mut chars: Vec<char> = code.chars().collect();
        let c = chars[1];
        let pos = ALPHABET.iter().position(|&a| a == c as u8).unwrap();
        let next = ALPHABET[(pos + 1) % ALPHABET.len()] as char;
        chars[1] = next;
        let broken: String = chars.into_iter().collect();
        if broken != code {
            assert!(decode(&broken).is_none(), "испорченный код {broken} не декодируется");
        }
    }

    #[test]
    fn code_accepts_lowercase_and_hyphens() {
        let r = default_rules();
        let code = encode(&r);
        let messy: String = code
            .chars()
            .map(|c| if c == '-' { '-' } else { c.to_ascii_lowercase() })
            .collect();
        let back = decode(&messy).expect("строчный код с дефисом декодируется");
        assert_eq!(back, r);
    }

    #[test]
    fn code_accepts_lookalike_letters() {
        let r = default_rules();
        let code = encode(&r);
        // Заменяем 0→O и 1→I: нормализация должна вернуть тот же код.
        let o1: String = code
            .chars()
            .map(|c| match c {
                '0' => 'O',
                '1' => 'I',
                other => other,
            })
            .collect();
        let back = decode(&o1).expect("O/I/L нормализуются");
        assert_eq!(back, r);
    }

    #[test]
    fn code_apply_matches_game() {
        let mut g = Game::new(20.0);
        g.checkers_len = 10.0;
        let r = Rules::from_game(&g);
        let code = encode(&r);
        let mut other = Game::new(60.0);
        other.checkers_len = 60.0;
        other.twilight_by_moves = false;
        other.auto_phase = true;
        other.promotion_choice = true;
        other.mandatory_capture = false;
        other.crown_non_pawns = false;
        other.move_time_limit = 180.0;
        let back = decode(&code).unwrap();
        back.apply(&mut other);
        assert_eq!(Rules::from_game(&other), r, "после применения код совпадает");
    }

    #[test]
    fn code_rejects_wrong_length() {
        let r = default_rules();
        let code = encode(&r);
        let short = &code[..7];
        assert!(decode(short).is_none());
        let extra = format!("{}X", code);
        assert!(decode(&extra).is_none());
        let bad = format!("{}#X", code);
        assert!(decode(&bad).is_none());
    }

    #[test]
    fn base32_roundtrip_for_id() {
        let key = [0x12, 0x34, 0x56, 0x78, 0x9A];
        let id = base32(&key);
        assert_eq!(id.len(), 8, "ID из 5 байт — 8 символов");
        assert_eq!(decode_id(&id).expect("ID декодируется"), key);
        let messy = format!("{}-{}", &id[..4], &id[4..]).to_lowercase();
        assert_eq!(decode_id(&messy).expect("ID с дефисом"), key);
        assert_eq!(decode_id("00O0-0O00").expect("O нормализуется"), [0; 5]);
        assert!(decode_id("12345").is_none(), "короткий ID отклоняется");
        assert!(decode_id("1234567#").is_none(), "посторонние символы отклоняются");
    }
}
