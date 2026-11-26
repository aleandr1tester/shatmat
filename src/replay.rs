//! Проигрывание партии из лога текущими правилами.
//!
//! Регрессия правил: если после правок движка записанная партия перестала
//! воспроизводиться (ход стал нелегальным или результат другим) — здесь это
//! всплывёт. Запуск: `shatmat --replay <файл|папка>`.

use std::path::{Path, PathBuf};

use crate::game::{Game, Kind, Phase, Piece, Side, Square, Status};

/// Один ход из таблицы лога.
struct MoveRow {
    side: Side,
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

/// Загружает партию из лога для просмотра: возвращает доску с историей
/// (снимки на каждый полуход), чтобы листать её как обычный разбор.
pub fn load_game(path: &Path) -> Option<Game> {
    let text = std::fs::read_to_string(path).ok()?;
    let logged = parse(&text).ok()?;
    let (chess_moves, checkers_moves) = logged.lengths?;
    let mut g = Game::new(60.0);
    g.set_twilight_by_moves(true);
    g.set_phase_len(Phase::Chess, chess_moves);
    g.set_phase_len(Phase::Checkers, checkers_moves);
    g.set_move_time_limit(logged.move_time);
    g.mandatory_capture = logged.mandatory_capture;
    g.crown_non_pawns = logged.crown_non_pawns;
    g.king_escape = logged.king_escape;
    // Доска ведётся строго по логу: правила не перепроверяем, поэтому
    // откроется любая партия, даже если движок уже играет иначе.
    for row in &logged.moves {
        raw_apply(&mut g, row);
    }
    g.status = Status::Playing;
    g.selected = None;
    g.legal.clear();
    g.chain = None;
    g.trans = 0.0;
    g.paused = false;
    g.pending_promotion = None;
    g.move_twilight_pending = false;
    Some(g)
}

/// Провести ход из лога по доске без проверки правил.
fn raw_apply(g: &mut Game, row: &MoveRow) {
    g.turn = row.side;
    g.phase = row.phase;
    let Some(mut piece) = g.board[row.from.1][row.from.0].take() else {
        return;
    };
    let mut captured: Option<(Square, Piece)> = None;
    if row.capture {
        let victim_sq = if row.phase == Phase::Chess {
            Some(row.to)
        } else {
            checker_victim(g, row.from, row.to, piece.side)
        };
        if let Some(v) = victim_sq {
            if let Some(vp) = g.board[v.1][v.0].take() {
                captured = Some((v, vp));
            }
        }
    }
    if row.phase == Phase::Checkers {
        let last = if piece.side == Side::White { 7 } else { 0 };
        if row.to.1 == last {
            piece.crowned = true;
        }
    } else if piece.kind == Kind::Pawn {
        // Превращение в шахматах: пешка на последнем ряду — ферзь.
        let last = if piece.side == Side::White { 7 } else { 0 };
        if row.to.1 == last {
            piece.kind = Kind::Queen;
        }
    }
    let kind_before = piece.kind;
    let damka = piece.crowned;
    g.board[row.to.1][row.to.0] = Some(piece);
    g.last_move = Some((row.from, row.to));
    g.last_capture = captured.map(|(sq, p)| (sq, p));
    g.history.push(crate::game::MoveRecord {
        side: row.side,
        kind: kind_before,
        damka,
        from: row.from,
        to: row.to,
        captured: captured.map(|(_, p)| (p.side, p.kind)),
        phase: row.phase,
    });
    g.turn = row.side.other();
    g.snapshots.push(crate::game::Snapshot {
        board: g.board,
        turn: g.turn,
        phase: row.phase,
        last_move: Some((row.from, row.to)),
        last_capture: g.last_capture,
    });
}

/// Кого бьёт шашечный ход: первая чужая фигура между «откуда» и «куда»
/// (или прямо на клетке приземления, если короля берут наступлением).
fn checker_victim(g: &Game, from: Square, to: Square, side: Side) -> Option<Square> {
    let (df, dr) = (
        to.0 as i32 - from.0 as i32,
        to.1 as i32 - from.1 as i32,
    );
    let steps = df.abs().max(dr.abs());
    if steps <= 1 {
        return None;
    }
    let (sx, sy) = (df.signum(), dr.signum());
    for k in 1..steps {
        let f = from.0 as i32 + sx * k;
        let r = from.1 as i32 + sy * k;
        if !(0..8).contains(&f) || !(0..8).contains(&r) {
            return None;
        }
        if let Some(p) = g.board[r as usize][f as usize] {
            if p.side != side {
                return Some((f as usize, r as usize));
            }
            return None;
        }
    }
    None
}

/// Список логов из папки `logs`: (путь, подпись). Свежие сверху.
pub fn list_logs() -> Vec<(PathBuf, String)> {
    let dir = crate::logging::logs_dir();
    let mut items: Vec<(PathBuf, String)> = Vec::new();
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return items;
    };
    for entry in rd.flatten() {
        let p = entry.path();
        if !p.extension().map(|e| e == "txt").unwrap_or(false) {
            continue;
        }
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.to_uppercase().starts_with("README") {
            continue;
        }
        items.push((p.clone(), log_caption(&p)));
    }
    items.sort_by(|a, b| b.0.cmp(&a.0));
    items
}

/// Короткая строка для списка логов: результат партии из заголовка.
pub fn log_caption(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .trim_end_matches(".txt")
        .to_string();
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut result = String::new();
    let mut mode = String::new();
    for line in text.lines().take(20) {
        if let Some(r) = line.strip_prefix("Результат: ") {
            result = r.trim().to_string();
        } else if let Some(m) = line.strip_prefix("Режим: ") {
            mode = m.trim().to_string();
        }
    }
    if result.is_empty() {
        name
    } else {
        format!("{name} · {mode} · {result}")
    }
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
            let side = if parts[1].trim() == "Белые" {
                Side::White
            } else {
                Side::Black
            };
            logged.moves.push(MoveRow {
                side,
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
