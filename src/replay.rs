//! Проигрывание партии из лога текущими правилами.
//!
//! Регрессия правил: если после правок движка записанная партия перестала
//! воспроизводиться (ход стал нелегальным или результат другим) — здесь это
//! всплывёт. Запуск: `shatmat --replay <файл|папка>`.

use std::path::{Path, PathBuf};

use crate::game::{Game, Kind, Phase, Side, Status};

/// Один ход из таблицы лога.
struct MoveRow {
    from: (usize, usize),
    to: (usize, usize),
    capture: bool,
    phase: Phase,
}

struct Logged {
    /// Длины фаз в полных ходах (если режим «по ходам» и он детерминирован).
    lengths: Option<(u8, u8)>,
    auto_phase: bool,
    time_mode: bool,
    move_time: f32,
    mandatory_capture: bool,
    crown_non_pawns: bool,
    king_escape: bool,
    result: String,
    moves: Vec<MoveRow>,
}

pub fn run(path: &Path) {
    let files = collect(path);
    if files.is_empty() {
        println!("логов не найдено: {}", path.display());
        return;
    }
    let mut ok = 0usize;
    let mut bad = 0usize;
    let mut skipped = 0usize;
    let mut shifted = 0usize;
    for f in &files {
        match check(f) {
            Ok(Outcome::Ok) => ok += 1,
            Ok(Outcome::Skip(why)) => {
                skipped += 1;
                println!("{}: пропущено — {why}", f.display());
            }
            Ok(Outcome::PhaseShift(why)) => {
                shifted += 1;
                println!("{}: сдвиг фазы — {why}", f.display());
            }
            Err(e) => {
                bad += 1;
                println!("{}: {e}", f.display());
            }
        }
    }
    println!(
        "\nлогов: {}, воспроизведено: {ok}, сдвиг фазы (ход во время паузы): {shifted}, расхождений: {bad}, пропущено: {skipped}",
        files.len()
    );
}

enum Outcome {
    Ok,
    /// Партия не воспроизводится один в один: игрок успел сходить во время
    /// паузы сумерек, и этот ход остался в старой фазе.
    PhaseShift(String),
    Skip(String),
}

fn collect(path: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if path.is_dir() {
        collect_dir(path, &mut out);
        out.sort();
    } else if path.is_file() {
        out.push(path.to_path_buf());
    }
    out
}

fn collect_dir(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_dir(&p, out);
        } else if p.extension().map(|e| e == "txt").unwrap_or(false) {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !name.to_uppercase().starts_with("README") {
                out.push(p);
            }
        }
    }
}

fn check(path: &Path) -> Result<Outcome, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("чтение: {e}"))?;
    let logged = parse(&text)?;
    if logged.time_mode || logged.auto_phase {
        return Ok(Outcome::Skip("смена фаз не детерминирована".to_string()));
    }
    let Some((chess_moves, checkers_moves)) = logged.lengths else {
        return Ok(Outcome::Skip("нет длин фаз в заголовке".to_string()));
    };
    let mut g = Game::new(60.0);
    g.set_twilight_by_moves(true);
    g.set_phase_len(Phase::Chess, chess_moves);
    g.set_phase_len(Phase::Checkers, checkers_moves);
    g.set_move_time_limit(logged.move_time);
    g.mandatory_capture = logged.mandatory_capture;
    g.crown_non_pawns = logged.crown_non_pawns;
    g.king_escape = logged.king_escape;

    for (i, mv) in logged.moves.iter().enumerate() {
        settle(&mut g);
        if g.is_over() {
            return Err(format!("ход {}: партия уже окончена", i + 1));
        }
        if g.pending_promotion.is_some() {
            g.choose_promotion(Kind::Queen);
        }
        if g.phase != mv.phase {
            // Ход во время паузы сумерек остаётся в старой фазе: движок
            // к этому моменту уже перевернул фазу. Это не ошибка правил,
            // а особенность ввода — помечаем и прекращаем разбор.
            settle(&mut g);
            if g.phase != mv.phase {
                return Ok(Outcome::PhaseShift(format!(
                    "ход {}: в логе {:?}, у движка {:?}",
                    i + 1,
                    mv.phase,
                    g.phase
                )));
            }
        }
        let legal = g
            .moves_for(mv.from)
            .into_iter()
            .find(|m| m.to == mv.to && m.captured.is_some() == mv.capture);
        let Some(found) = legal else {
            return Err(format!(
                "ход {}: {}->{} ({}) нелегален",
                i + 1,
                name(mv.from),
                name(mv.to),
                if mv.capture { "взятие" } else { "тихо" }
            ));
        };
        g.apply(found);
    }
    settle(&mut g);

    let want = logged.result.as_str();
    if want.starts_with("прервана") || want.is_empty() {
        return Ok(Outcome::Ok);
    }
    let got = match &g.status {
        Status::Win { side, .. } => format!("победа {}", side_ru(*side)),
        Status::Draw { .. } => "ничья".to_string(),
        _ => "не окончена".to_string(),
    };
    let want_kind = if want.starts_with("победа") {
        want.split('—').next().unwrap_or("").trim().to_string()
    } else if want.starts_with("ничья") {
        "ничья".to_string()
    } else {
        want.to_string()
    };
    if got != want_kind {
        return Err(format!("результат: в логе «{want_kind}», получилось «{got}»"));
    }
    Ok(Outcome::Ok)
}

/// Прокрутить переходы сумерек, чтобы следующий ход делался в новой фазе.
fn settle(g: &mut Game) {
    let mut guard = 0;
    while !g.is_over() && guard < 2000 {
        if g.pending_promotion.is_some() {
            g.choose_promotion(Kind::Queen);
        }
        if g.trans == 0.0 && !g.move_twilight_pending && g.pending_promotion.is_none() {
            break;
        }
        g.tick(0.1);
        guard += 1;
    }
}

fn side_ru(side: Side) -> &'static str {
    match side {
        Side::White => "белые",
        Side::Black => "чёрные",
    }
}

fn name(sq: (usize, usize)) -> String {
    format!("{}{}", (b'a' + sq.0 as u8) as char, sq.1 + 1)
}

fn parse(text: &str) -> Result<Logged, String> {
    let mut logged = Logged {
        lengths: None,
        auto_phase: false,
        time_mode: false,
        move_time: 60.0,
        mandatory_capture: true,
        crown_non_pawns: true,
        king_escape: true,
        result: String::new(),
        moves: Vec::new(),
    };
    for line in text.lines() {
        let line = line.trim_end();
        if let Some(rest) = line.strip_prefix("Смена фаз: ") {
            if let Some(pos) = rest.find("время хода ") {
                let tail = &rest[pos + "время хода ".len()..];
                let num: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(v) = num.parse::<f32>() {
                    logged.move_time = v;
                }
            }
            if rest.starts_with("по времени") {
                logged.time_mode = true;
            } else if rest.contains("случайно") {
                logged.auto_phase = true;
            } else if let Some(pos) = rest.find("шахматы ") {
                // новый формат: шахматы C, шашки K
                let after = &rest[pos + "шахматы ".len()..];
                let c: String = after.chars().take_while(|ch| ch.is_ascii_digit()).collect();
                let k = after
                    .find("шашки ")
                    .map(|i| {
                        after[i + "шашки ".len()..]
                            .chars()
                            .take_while(|ch| ch.is_ascii_digit())
                            .collect::<String>()
                    })
                    .unwrap_or_default();
                if let (Ok(c), Ok(k)) = (c.parse::<u8>(), k.parse::<u8>()) {
                    logged.lengths = Some((c, k));
                }
            } else if let Some(pos) = rest.find("полных хода") {
                // старый формат: N полных хода для обеих фаз
                let before = rest[..pos].trim_end();
                let num: String = before
                    .chars()
                    .rev()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
                if let Ok(n) = num.parse::<u8>() {
                    logged.lengths = Some((n, n));
                }
            }
        } else if line.starts_with("Обязательное взятие:") {
            logged.mandatory_capture = line.ends_with("вкл");
        } else if line.starts_with("Коронация не-пешек:") {
            logged.crown_non_pawns = line.ends_with("вкл");
        } else if line.starts_with("Король уходит от боя:") {
            logged.king_escape = line.ends_with("вкл");
        } else if let Some(rest) = line.strip_prefix("Результат: ") {
            logged.result = rest.trim().to_string();
        }
        // ход: «1\tБелые\tПешка\th2-h4\t—\tшахматы»
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() == 6 && parts[0].chars().all(|c| c.is_ascii_digit()) {
            let mv = parts[3].trim();
            let phase = match parts[5].trim() {
                "шашки" => Phase::Checkers,
                _ => Phase::Chess,
            };
            let from = parse_sq(&mv[..2]).ok_or_else(|| format!("клетка «{mv}»"))?;
            let to = parse_sq(&mv[3..5]).ok_or_else(|| format!("клетка «{mv}»"))?;
            logged.moves.push(MoveRow {
                from,
                to,
                capture: mv.contains('x'),
                phase,
            });
        }
    }
    Ok(logged)
}

fn parse_sq(s: &str) -> Option<(usize, usize)> {
    let b = s.as_bytes();
    if b.len() != 2 {
        return None;
    }
    let f = (b[0] as char).to_digit(36)? as usize;
    let r = (b[1] as char).to_digit(10)? as usize;
    let file = f.checked_sub(10)?; // 'a'..'h' -> 0..7
    let rank = r.checked_sub(1)?;
    if file < 8 && rank < 8 {
        Some((file, rank))
    } else {
        None
    }
}
