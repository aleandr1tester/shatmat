//! Турнир ИИ против ИИ без окна: сравнение соотношений длин фаз.
//!
//! Запуск: `shatmat --arena --games 100 --out arena.csv`.
//! Сетка по умолчанию: шахматы 4/6/8 × шашки 2/4/6/8.
//! Каждая партия играется до конца (или до 400 полуходов), метрики пишутся
//! в CSV, а сводка печатается в stdout.

use std::io::Write;
use std::path::PathBuf;

use crate::ai::{self, Difficulty};
use crate::game::{Game, Kind, Phase, Side, Status};

/// Настройки турнира.
pub struct Config {
    /// Сколько партий на каждую комбинацию.
    pub games: usize,
    /// Длины шахматных фаз в полных ходах.
    pub chess: Vec<u8>,
    /// Длины шашечных фаз в полных ходах.
    pub checkers: Vec<u8>,
    pub difficulty: Difficulty,
    pub out: PathBuf,
}

struct Row {
    chess: u8,
    checkers: u8,
    seed: u64,
    result: String,
    winner: Option<Side>,
    plies: usize,
    caps_chess: u32,
    caps_checkers: u32,
    flips: u32,
    /// Сколько полуходов реально прошло в шахматной и шашечной фазах.
    plies_chess: usize,
    plies_checkers: usize,
    last_flip_material: i32,
    winner_behind: bool,
    flip_decided: bool,
}

/// Одна партия ИИ против ИИ с заданными длинами фаз.
fn play(chess: u8, checkers: u8, difficulty: Difficulty, seed0: u64) -> Row {
    let mut g = Game::new(60.0);
    g.set_twilight_by_moves(true);
    g.set_phase_len(Phase::Chess, chess);
    g.set_phase_len(Phase::Checkers, checkers);
    g.set_move_time_limit(180.0);
    g.mandatory_capture = true;
    g.crown_non_pawns = true;
    g.max_chain = true;
    g.king_escape = true;
    g.promotion_choice = false;

    let mut seed = seed0 | 1;
    let mut plies = 0usize;
    let mut plies_chess = 0usize;
    let mut plies_checkers = 0usize;
    let mut caps_chess = 0u32;
    let mut caps_checkers = 0u32;
    let mut flips = 0u32;
    let mut last_flip_ply = 0usize;
    let mut last_flip_material = 0i32;
    let mut prev_phase = g.phase;

    while !g.is_over() && plies < 400 {
        // Докручиваем паузу сумерек и сам переход, чтобы следующий ход
        // не «перескочил» смену природы.
        let mut guard = 0;
        while !g.is_over() && guard < 600 {
            if g.pending_promotion.is_some() {
                g.choose_promotion(Kind::Queen);
            }
            if g.trans == 0.0 && !g.move_twilight_pending && g.pending_promotion.is_none() {
                break;
            }
            g.tick(0.1);
            guard += 1;
        }
        if g.is_over() {
            break;
        }
        let Some(mv) = ai::best_move(&g, difficulty, &mut seed) else {
            break;
        };
        let phase = g.phase;
        let captured = mv.captured.is_some();
        g.apply(mv);
        plies += 1;
        if phase == Phase::Chess {
            plies_chess += 1;
        } else {
            plies_checkers += 1;
        }
        if captured {
            if phase == Phase::Chess {
                caps_chess += 1;
            } else {
                caps_checkers += 1;
            }
        }
        if g.phase != prev_phase {
            flips += 1;
            prev_phase = g.phase;
            last_flip_ply = plies;
            last_flip_material = g.material_diff();
        }
    }

    let (result, winner) = match &g.status {
        Status::Win { side, reason } => (format!("win {:?} {}", side, reason), Some(*side)),
        Status::Draw { reason } => (format!("draw {}", reason), None),
        _ => ("unfinished".to_string(), None),
    };
    let winner_behind = match winner {
        Some(Side::White) => last_flip_material < 0,
        Some(Side::Black) => last_flip_material > 0,
        None => false,
    };
    let flip_decided = winner.is_some() && plies.saturating_sub(last_flip_ply) <= 2;
    Row {
        chess,
        checkers,
        seed: seed0,
        result,
        winner,
        plies,
        caps_chess,
        caps_checkers,
        flips,
        plies_chess,
        plies_checkers,
        last_flip_material,
        winner_behind,
        flip_decided,
    }
}

/// Сид зависит только от номера партии: все комбинации играют одни и те же
/// «открытия», поэтому сравнение получается парным.
fn seed_for(index: usize) -> u64 {
    const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;
    GOLDEN.wrapping_mul(index as u64 + 1).rotate_left(17) | 1
}

/// Играет сетку партий, пишет CSV и печатает сводку.
pub fn run(cfg: &Config) {
    let mut rows: Vec<Row> = Vec::new();
    for &chess in &cfg.chess {
        for &checkers in &cfg.checkers {
            for index in 0..cfg.games {
                rows.push(play(chess, checkers, cfg.difficulty, seed_for(index)));
            }
            eprint!(
                "\rсыграно: шахматы {} / шашки {} — {} партий",
                chess, checkers, cfg.games
            );
        }
    }
    eprintln!();

    let mut csv = String::from(
        "chess_moves,checkers_moves,seed,result,winner,plies,caps_chess,caps_checkers,flips,last_flip_material,winner_behind,flip_decided,plies_chess,plies_checkers\n",
    );
    for r in &rows {
        let winner = match r.winner {
            Some(Side::White) => "white",
            Some(Side::Black) => "black",
            None => "",
        };
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            r.chess,
            r.checkers,
            r.seed,
            r.result.replace(',', ";"),
            winner,
            r.plies,
            r.caps_chess,
            r.caps_checkers,
            r.flips,
            r.last_flip_material,
            r.winner_behind as u8,
            r.flip_decided as u8,
            r.plies_chess,
            r.plies_checkers,
        ));
    }
    if let Ok(mut f) = std::fs::File::create(&cfg.out) {
        let _ = f.write_all(csv.as_bytes());
        println!("CSV: {}", cfg.out.display());
    } else {
        println!("не удалось записать {}", cfg.out.display());
    }

    println!();
    println!(
        "{:>6} {:>7} | {:>4} {:>4} {:>4} {:>4} | {:>6} {:>6} {:>7} {:>7}",
        "шахм.", "шашки", "бел.", "чёрн.", "ничьи", "незав.", "длина", "смены", "лидер↓", "флип"
    );
    for &chess in &cfg.chess {
        for &checkers in &cfg.checkers {
            let set: Vec<&Row> = rows
                .iter()
                .filter(|r| r.chess == chess && r.checkers == checkers)
                .collect();
            let n = set.len().max(1) as f32;
            let white = set.iter().filter(|r| r.winner == Some(Side::White)).count();
            let black = set.iter().filter(|r| r.winner == Some(Side::Black)).count();
            let draws = set
                .iter()
                .filter(|r| r.result.starts_with("draw"))
                .count();
            let unfinished = set
                .iter()
                .filter(|r| r.result.starts_with("unfinished"))
                .count();
            let plies: f32 = set.iter().map(|r| r.plies as f32).sum::<f32>() / n;
            let flips: f32 = set.iter().map(|r| r.flips as f32).sum::<f32>() / n;
            let behind = set.iter().filter(|r| r.winner_behind).count() as f32 / n;
            let decided = set.iter().filter(|r| r.flip_decided).count() as f32 / n;
            println!(
                "{:>6} {:>7} | {:>4} {:>4} {:>4} {:>4} | {:>6.1} {:>6.1} {:>6.0}% {:>6.0}%",
                chess,
                checkers,
                white,
                black,
                draws,
                unfinished,
                plies,
                flips,
                behind * 100.0,
                decided * 100.0
            );
        }
    }
    println!();
    println!("«лидер↓» — доля партий, где победитель на последней смене фаз был позади по материалу.");
    println!("«флип» — доля партий, решённых в первые два полухода после смены фаз.");
}
