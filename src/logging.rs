//! Файловые логи партий: время, режим, ходы и итоги.

use std::path::PathBuf;

use crate::game::{sq_name, Game, Kind, MoveRecord, Phase, Side};

pub fn now_string() -> String {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct SystemTime {
            year: u16,
            month: u16,
            day_of_week: u16,
            day: u16,
            hour: u16,
            minute: u16,
            second: u16,
            milliseconds: u16,
        }
        extern "system" {
            fn GetLocalTime(st: *mut SystemTime);
        }
        let mut st = SystemTime {
            year: 0,
            month: 0,
            day_of_week: 0,
            day: 0,
            hour: 0,
            minute: 0,
            second: 0,
            milliseconds: 0,
        };
        unsafe {
            GetLocalTime(&mut st);
        }
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            st.year, st.month, st.day, st.hour, st.minute, st.second
        )
    }
    #[cfg(not(windows))]
    {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        format!("unix-{secs}")
    }
}

pub fn logs_dir() -> PathBuf {
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
    {
        return dir.join("logs");
    }
    PathBuf::from("logs")
}

fn kind_name(kind: Kind, phase: Phase, damka: bool) -> &'static str {
    if phase == Phase::Checkers {
        if kind == Kind::King {
            crate::i18n::log("Король")
        } else if damka {
            crate::i18n::log("Дамка")
        } else {
            crate::i18n::log("Шашка")
        }
    } else {
        match kind {
            Kind::Pawn => crate::i18n::log("Пешка"),
            Kind::Knight => crate::i18n::log("Конь"),
            Kind::Bishop => crate::i18n::log("Слон"),
            Kind::Rook => crate::i18n::log("Ладья"),
            Kind::Queen => crate::i18n::log("Ферзь"),
            Kind::King => crate::i18n::log("Король"),
        }
    }
}

fn phase_name(phase: Phase) -> &'static str {
    match phase {
        Phase::Chess => crate::i18n::log("шахматы"),
        Phase::Checkers => crate::i18n::log("шашки"),
    }
}

fn side_log(side: Side) -> &'static str {
    match side {
        Side::White => crate::i18n::log("Белые"),
        Side::Black => crate::i18n::log("Чёрные"),
    }
}

fn move_record_line(index: usize, m: &MoveRecord) -> String {
    let piece = kind_name(m.kind, m.phase, m.damka);
    let coords = if m.captured.is_some() {
        format!("{}x{}", sq_name(m.from), sq_name(m.to))
    } else {
        format!("{}-{}", sq_name(m.from), sq_name(m.to))
    };
    let captured = m
        .captured
        .map(|(side, kind)| format!("{} {}", side_log(side), kind_name(kind, m.phase, false)))
        .unwrap_or_else(|| "—".to_string());
    format!(
        "{index}\t{}\t{piece}\t{coords}\t{captured}\t{}",
        side_log(m.side),
        phase_name(m.phase)
    )
}

pub fn format_game_log(
    game: &Game,
    start: &str,
    end: &str,
    mode: &str,
    result: &str,
) -> String {
    let white = game
        .history
        .iter()
        .filter(|m| m.side == Side::White)
        .count();
    let black = game
        .history
        .iter()
        .filter(|m| m.side == Side::Black)
        .count();
    let mut s = String::new();
    s.push_str(&crate::i18n::logf("Партия «ModeShift: Bipolar»\n", &[]));
    s.push_str(&crate::i18n::logf(
        "Версия: {version}\n",
        &[("version", env!("CARGO_PKG_VERSION"))],
    ));
    s.push_str(&crate::i18n::logf("Начало: {start}\n", &[("start", start)]));
    s.push_str(&crate::i18n::logf("Окончание: {end}\n", &[("end", end)]));
    s.push_str(&crate::i18n::logf("Режим: {mode}\n", &[("mode", mode)]));
    if game.twilight_by_moves {
        s.push_str(&crate::i18n::logf(
            "Смена фаз: по ходам (случайно после каждого хода), время хода {secs} с\n",
            &[("secs", &format!("{:.0}", game.move_time_limit))],
        ));
    } else if game.auto_phase {
        s.push_str(&crate::i18n::logf(
            "Автосмена сумерек: случайно 10–{secs} с\n",
            &[("secs", &format!("{:.0}", game.auto_max))],
        ));
    } else {
        s.push_str(&crate::i18n::logf(
            "Время фаз: шахматы {chess} с, шашки {checkers} с\n",
            &[
                ("chess", &format!("{:.0}", game.chess_len)),
                ("checkers", &format!("{:.0}", game.checkers_len)),
            ],
        ));
    }
    s.push_str(&crate::i18n::logf(
        "Обязательное взятие: {value}\n",
        &[(
            "value",
            if game.mandatory_capture {
                crate::i18n::log("вкл")
            } else {
                crate::i18n::log("выкл")
            },
        )],
    ));
    s.push_str(&crate::i18n::logf(
        "Коронация не-пешек: {value}\n",
        &[(
            "value",
            if game.crown_non_pawns {
                crate::i18n::log("вкл")
            } else {
                crate::i18n::log("выкл")
            },
        )],
    ));
    s.push_str(&crate::i18n::logf("Результат: {result}\n", &[("result", result)]));
    s.push_str(&crate::i18n::logf(
        "Всего ходов: {total} (белые: {white}, чёрные: {black})\n",
        &[
            ("total", &game.history.len().to_string()),
            ("white", &white.to_string()),
            ("black", &black.to_string()),
        ],
    ));
    s.push_str(&crate::i18n::logf(
        "Полных ходов: {value}\n",
        &[("value", &((game.history.len() + 1) / 2).to_string())],
    ));
    s.push_str(&crate::i18n::logf(
        "\n№\tСторона\tФигура\tХод\tВзятие\tФаза\n",
        &[],
    ));
    for (i, m) in game.history.iter().enumerate() {
        s.push_str(&move_record_line(i + 1, m));
        s.push('\n');
    }
    s
}

pub fn write_game_log(
    game: &Game,
    start: &str,
    mode: &str,
    result: &str,
) -> Option<PathBuf> {
    let dir = logs_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return None;
    }
    let end = now_string();
    let text = format_game_log(game, start, &end, mode, result);
    let stamp = end.replace(' ', "-").replace(':', "-");
    let mut path = dir.join(format!("game-{stamp}.txt"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("game-{stamp}-{n}.txt"));
        n += 1;
    }
    std::fs::write(&path, text).ok()?;
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Move, Phase};

    #[test]
    fn log_has_times_mode_counts_and_coordinates() {
        let mut g = Game::new(25.0);
        g.apply(Move {
            from: (4, 1),
            to: (4, 3),
            captured: None,
        });
        let text = format_game_log(
            &g,
            "2026-01-01 10:00:00",
            "2026-01-01 10:01:00",
            "вдвоём",
            "не завершена",
        );
        assert!(text.contains("Начало: 2026-01-01 10:00:00"));
        assert!(text.contains("Окончание: 2026-01-01 10:01:00"));
        assert!(text.contains("Режим: вдвоём"));
        assert!(text.contains("Всего ходов: 1 (белые: 1, чёрные: 0)"));
        assert!(text.contains("e2-e4"));
        assert!(text.contains("Пешка"));
        assert!(text.contains("шахматы"));
    }

    #[test]
    fn log_marks_capture_and_checkers_phase() {
        let mut g = Game::new(25.0);
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[3][2] = Some(crate::game::Piece::new(Side::White, Kind::Pawn)); // c4
        g.board[4][3] = Some(crate::game::Piece::new(Side::Black, Kind::Pawn)); // d5
        g.turn = Side::White;
        g.apply(Move {
            from: (2, 3),
            to: (4, 5),
            captured: Some((3, 4)),
        });
        let text = format_game_log(&g, "s", "e", "вдвоём", "не завершена");
        assert!(text.contains("c4xe6"));
        assert!(text.contains("Шашка"));
        assert!(text.contains("шашки"));
    }

    #[test]
    fn write_game_log_creates_file() {
        let g = Game::new(25.0);
        let path = write_game_log(&g, "start", "тест", "не завершена").expect("лог записан");
        assert!(path.exists());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("Начало: start"));
        assert!(text.contains("Всего ходов: 0"));
        let _ = std::fs::remove_file(path);
    }
}
