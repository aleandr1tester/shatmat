//! Компьютерный соперник: перебор вариантов с оценкой позиции.
//!
//! Сложность задаёт глубину поиска: легко — случайный ход, средне — 2 полухода,
//! сложно — 3 полухода с отсечением. Оценка учитывает и шахматную, и шашечную
//! силу фигур, потому что через несколько секунд природа фигур меняется.

use crate::game::{Game, Kind, Move, Phase, Side, Status};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

impl Difficulty {
    pub fn label_ru(self) -> &'static str {
        match self {
            Difficulty::Easy => "легко",
            Difficulty::Medium => "средне",
            Difficulty::Hard => "сложно",
        }
    }

    fn depth(self) -> u8 {
        match self {
            Difficulty::Easy => 1,
            Difficulty::Medium => 2,
            Difficulty::Hard => 3,
        }
    }
}

pub const ALL_DIFFICULTIES: [Difficulty; 3] = [
    Difficulty::Easy,
    Difficulty::Medium,
    Difficulty::Hard,
];

const MATE: f32 = 100_000.0;
const INF: f32 = 1_000_000.0;

/// Лучший ход для стороны, которая сейчас ходит.
pub fn best_move(g: &Game, difficulty: Difficulty, seed: &mut u64) -> Option<Move> {
    let moves = g.all_moves();
    if moves.is_empty() {
        return None;
    }
    if difficulty == Difficulty::Easy && moves.len() > 1 {
        let i = (next_rand(seed) as usize) % moves.len();
        return Some(moves[i]);
    }

    let depth = difficulty.depth();
    let mut best = moves[0];
    let mut best_score = -INF;
    let mut alpha = -INF;
    for mv in order(g, moves) {
        let mut child = g.clone();
        apply_auto(&mut child, mv);
        let score = -negamax(&child, depth.saturating_sub(1), -INF, -alpha, 1);
        if score > best_score + 0.001 {
            best_score = score;
            best = mv;
            alpha = alpha.max(score);
        } else if (score - best_score).abs() <= 0.001 && next_rand(seed) % 3 == 0 {
            best = mv;
        }
    }
    Some(best)
}

/// Оценка со стороны того, кто ходит: чем больше, тем лучше для него.
fn negamax(g: &Game, depth: u8, mut alpha: f32, beta: f32, ply: u8) -> f32 {
    match &g.status {
        Status::Win { side, .. } => {
            return if *side == g.turn {
                MATE - ply as f32
            } else {
                -MATE + ply as f32
            };
        }
        Status::Draw(_) => return 0.0,
        _ => {}
    }
    if depth == 0 {
        return eval_for_turn(g);
    }
    let moves = g.all_moves();
    if moves.is_empty() {
        return eval_for_turn(g);
    }

    let mut best = -INF;
    for mv in order(g, moves) {
        let mut child = g.clone();
        apply_auto(&mut child, mv);
        let score = -negamax(&child, depth - 1, -beta, -alpha, ply + 1);
        if score > best {
            best = score;
        }
        if best > alpha {
            alpha = best;
        }
        if alpha >= beta {
            break;
        }
    }
    best
}

/// Применяет ход и, если пешка дошла до края, сразу выбирает ферзя.
/// Компьютер не спрашивает игрока — он всегда превращает в ферзя.
fn apply_auto(g: &mut Game, mv: Move) {
    g.apply(mv);
    if g.pending_promotion.is_some() {
        g.choose_promotion(Kind::Queen);
    }
}

fn eval_for_turn(g: &Game) -> f32 {
    let white = eval_white(g);
    if g.turn == Side::White {
        white
    } else {
        -white
    }
}

fn eval_white(g: &Game) -> f32 {
    let mut chess = 0.0;
    let mut checkers = 0.0;
    for (r, row) in g.board.iter().enumerate() {
        for cell in row.iter() {
            let Some(p) = cell else { continue };
            let s = if p.side == Side::White { 1.0 } else { -1.0 };
            chess += s * chess_value(p.kind);
            checkers += s * if p.is_damka() { 3.2 } else { 1.0 };
            if p.kind.becomes_man() {
                let adv = if p.side == Side::White {
                    r as f32
                } else {
                    (7 - r) as f32
                } / 7.0;
                checkers += s * 0.35 * adv;
            }
        }
    }
    match g.phase {
        Phase::Chess => chess + checkers * 0.45,
        Phase::Checkers => checkers * 1.6 + chess * 0.35,
    }
}

fn chess_value(kind: Kind) -> f32 {
    match kind {
        Kind::Pawn => 1.0,
        Kind::Knight => 3.0,
        Kind::Bishop => 3.15,
        Kind::Rook => 5.0,
        Kind::Queen => 9.0,
        Kind::King => 0.0,
    }
}

/// Сначала взятия пожирнее — так отсечение работает заметно лучше.
fn order(g: &Game, mut moves: Vec<Move>) -> Vec<Move> {
    moves.sort_by(|a, b| {
        let va = a
            .captured
            .map(|sq| capture_value(g, sq))
            .unwrap_or(0.0);
        let vb = b
            .captured
            .map(|sq| capture_value(g, sq))
            .unwrap_or(0.0);
        vb.partial_cmp(&va).unwrap_or(std::cmp::Ordering::Equal)
    });
    moves
}

fn capture_value(g: &Game, sq: (usize, usize)) -> f32 {
    match g.board[sq.1][sq.0] {
        Some(p) if p.kind == Kind::King => 50.0,
        Some(p) if p.is_damka() => 5.0,
        Some(_) => 1.0,
        None => 0.0,
    }
}

fn next_rand(seed: &mut u64) -> u64 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *seed >> 33
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Piece, Side};

    fn seed() -> u64 {
        0xDEAD_BEEF_1234_5678
    }

    #[test]
    fn ai_returns_legal_moves() {
        for d in ALL_DIFFICULTIES {
            let g = Game::new(25.0);
            let mv = best_move(&g, d, &mut seed()).expect("есть ходы");
            assert!(g
                .all_moves()
                .iter()
                .any(|m| m.from == mv.from && m.to == mv.to));
        }
    }

    #[test]
    fn ai_takes_the_king() {
        let mut g = Game::new(25.0);
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[0][0] = Some(Piece::new(Side::White, Kind::King)); // a1
        g.board[4][4] = Some(Piece::new(Side::Black, Kind::King)); // e5 — под боем
        g.board[3][3] = Some(Piece::new(Side::White, Kind::Rook)); // d4 — дамка
        g.turn = Side::White;
        let mv = best_move(&g, Difficulty::Hard, &mut seed()).unwrap();
        assert_eq!(mv.captured, Some((4, 4)), "нужно съесть короля");
    }

    #[test]
    fn ai_plays_a_short_game() {
        let mut g = Game::new(20.0);
        let mut seed = seed();
        for _ in 0..60 {
            if g.is_over() {
                break;
            }
            g.tick(0.1);
            if g.is_locked() {
                continue;
            }
            if let Some(mv) = best_move(&g, Difficulty::Medium, &mut seed) {
                apply_auto(&mut g, mv);
                g.tick(0.0);
            } else {
                g.tick(0.01);
            }
        }
    }
}
