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
            "Король"
        } else if damka {
            "Дамка"
        } else {
            "Шашка"
        }
    } else {
        match kind {
            Kind::Pawn => "Пешка",
            Kind::Knight => "Конь",
            Kind::Bishop => "Слон",
            Kind::Rook => "Ладья",
            Kind::Queen => "Ферзь",
            Kind::King => "Король",
        }
    }
}

fn phase_name(phase: Phase) -> &'static str {
    match phase {
        Phase::Chess => "шахматы",
        Phase::Checkers => "шашки",
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
        .map(|(side, kind)| format!("{} {}", side.name_ru(), kind_name(kind, m.phase, false)))
        .unwrap_or_else(|| "—".to_string());
    format!(
        "{index}\t{}\t{piece}\t{coords}\t{captured}\t{}",
        m.side.name_ru(),
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
    s.push_str("Партия «ModeShift: Bipolar»\n");
    s.push_str(&format!("Версия: {}\n", env!("CARGO_PKG_VERSION")));
    s.push_str(&format!("Начало: {start}\n"));
    s.push_str(&format!("Окончание: {end}\n"));
    s.push_str(&format!("Режим: {mode}\n"));
    if game.twilight_by_moves {
        s.push_str(&format!(
            "Смена фаз: по ходам (случайно после каждого хода), время хода {:.0} с\n",
            game.move_time_limit
        ));
    } else if game.auto_phase {
        s.push_str(&format!(
            "Автосмена сумерек: случайно 10–{:.0} с\n",
            game.auto_max
        ));
    } else {
        s.push_str(&format!(
            "Время фаз: шахматы {:.0} с, шашки {:.0} с\n",
            game.chess_len, game.checkers_len
        ));
    }
    s.push_str(&format!(
        "Обязательное взятие: {}\n",
        if game.mandatory_capture { "вкл" } else { "выкл" }
    ));
    s.push_str(&format!(
        "Коронация не-пешек: {}\n",
        if game.crown_non_pawns { "вкл" } else { "выкл" }
    ));
    s.push_str(&format!("Результат: {result}\n"));
    s.push_str(&format!(
        "Всего ходов: {} (белые: {white}, чёрные: {black})\n",
        game.history.len()
    ));
    s.push_str(&format!(
        "Полных ходов: {}\n",
        (game.history.len() + 1) / 2
    ));
    s.push_str("\n№\tСторона\tФигура\tХод\tВзятие\tФаза\n");
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
