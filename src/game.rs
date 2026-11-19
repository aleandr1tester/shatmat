//! «Сумерки» — правила гибрида шахмат и шашек.
//!
//! Партия идёт на доске 8×8. Раз в N секунд начинается «сумеречный переход»,
//! после которого все фигуры меняют природу: шахматные фигуры начинают ходить
//! по правилам шашек и наоборот. Ходы в момент перехода замирают.

pub const TRANS_SECS: f32 = 1.7;
pub const MIN_PHASE: f32 = 10.0;
pub const MAX_PHASE: f32 = 45.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    White,
    Black,
}

impl Side {
    pub fn other(self) -> Side {
        match self {
            Side::White => Side::Black,
            Side::Black => Side::White,
        }
    }

    /// Направление движения пешек и простых шашек по рангам (0 — низ доски).
    pub fn forward(self) -> i32 {
        match self {
            Side::White => 1,
            Side::Black => -1,
        }
    }

    pub fn name_ru(self) -> &'static str {
        match self {
            Side::White => "Белые",
            Side::Black => "Чёрные",
        }
    }

    pub fn gen_ru(self) -> &'static str {
        match self {
            Side::White => "белых",
            Side::Black => "чёрных",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

impl Kind {
    pub fn letter(self) -> &'static str {
        match self {
            Kind::Pawn => "П",
            Kind::Knight => "К",
            Kind::Bishop => "С",
            Kind::Rook => "Л",
            Kind::Queen => "Ф",
            Kind::King => "Кр",
        }
    }

    /// В шашечной фазе пешки и кони становятся простыми шашками,
    /// остальные фигуры — дамками.
    pub fn becomes_man(self) -> bool {
        matches!(self, Kind::Pawn | Kind::Knight)
    }

    pub fn material(self) -> i32 {
        match self {
            Kind::Pawn => 1,
            Kind::Knight | Kind::Bishop => 3,
            Kind::Rook => 5,
            Kind::Queen => 9,
            Kind::King => 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Piece {
    pub side: Side,
    pub kind: Kind,
    /// Пешка/конь, дошедшие до последнего ряда в шашечной фазе, становятся дамкой.
    pub crowned: bool,
}

impl Piece {
    pub fn new(side: Side, kind: Kind) -> Self {
        Piece {
            side,
            kind,
            crowned: false,
        }
    }

    pub fn is_damka(&self) -> bool {
        self.crowned || !self.kind.becomes_man()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Chess,
    Checkers,
}

impl Phase {
    pub fn other(self) -> Phase {
        match self {
            Phase::Chess => Phase::Checkers,
            Phase::Checkers => Phase::Chess,
        }
    }

    pub fn label_ru(self) -> &'static str {
        match self {
            Phase::Chess => "ШАХМАТЫ",
            Phase::Checkers => "ШАШКИ",
        }
    }

    pub fn one_ru(self) -> &'static str {
        match self {
            Phase::Chess => "шахматы",
            Phase::Checkers => "шашки",
        }
    }
}

pub type Square = (usize, usize);
pub type Board = [[Option<Piece>; 8]; 8];

#[derive(Clone, Copy, Debug)]
pub struct Move {
    pub from: Square,
    pub to: Square,
    pub captured: Option<Square>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Status {
    Playing,
    Check(Side),
    Win { side: Side, reason: String },
    Draw(String),
}

#[derive(Clone)]
pub struct Game {
    pub board: Board,
    pub turn: Side,
    pub phase: Phase,
    pub phase_len: f32,
    pub phase_left: f32,
    /// 0..1 — прогресс сумеречного перехода (0 — обычная фаза).
    pub trans: f32,
    pub paused: bool,
    pub status: Status,
    pub selected: Option<Square>,
    pub legal: Vec<Move>,
    pub chain: Option<Square>,
    pub last_move: Option<(Square, Square)>,
    pub log: Vec<String>,
    pub ply: u32,
    pub captured_by_white: u32,
    pub captured_by_black: u32,
    pub anim: f32,
    pub crowned_flash: Option<(Square, f32)>,
    /// Сколько ходов подряд пропущено из-за отсутствия ходов в шашках.
    pub passes: u8,
    /// Оба застряли в шашках: сумерки больше не включаются, играем только шахматы.
    pub chess_only: bool,
    /// Пешка дошла до последнего ряда и ждёт, какую фигуру выбрать.
    pub pending_promotion: Option<Square>,
    royal_captured: Option<Side>,
    pending: String,
}

impl Game {
    pub fn new(phase_len: f32) -> Self {
        let mut board: Board = [[None; 8]; 8];
        let back = [
            Kind::Rook,
            Kind::Knight,
            Kind::Bishop,
            Kind::Queen,
            Kind::King,
            Kind::Bishop,
            Kind::Knight,
            Kind::Rook,
        ];
        for (f, k) in back.iter().enumerate() {
            board[0][f] = Some(Piece::new(Side::White, *k));
            board[7][f] = Some(Piece::new(Side::Black, *k));
            board[1][f] = Some(Piece::new(Side::White, Kind::Pawn));
            board[6][f] = Some(Piece::new(Side::Black, Kind::Pawn));
        }
        Game {
            board,
            turn: Side::White,
            phase: Phase::Chess,
            phase_len: phase_len.clamp(MIN_PHASE, MAX_PHASE),
            phase_left: phase_len.clamp(MIN_PHASE, MAX_PHASE),
            trans: 0.0,
            paused: false,
            status: Status::Playing,
            selected: None,
            legal: Vec::new(),
            chain: None,
            last_move: None,
            log: Vec::new(),
            ply: 0,
            captured_by_white: 0,
            captured_by_black: 0,
            anim: 0.0,
            crowned_flash: None,
            passes: 0,
            chess_only: false,
            pending_promotion: None,
            royal_captured: None,
            pending: String::new(),
        }
    }

    // ---------- состояние ----------

    pub fn is_over(&self) -> bool {
        matches!(self.status, Status::Win { .. } | Status::Draw(_))
    }

    pub fn is_locked(&self) -> bool {
        self.is_over() || self.trans > 0.0 || self.paused || self.pending_promotion.is_some()
    }

    pub fn twilight_pending(&self) -> bool {
        !self.chess_only && self.trans == 0.0 && self.phase_left <= 0.0 && !self.is_over()
    }

    pub fn count(&self, side: Side) -> usize {
        let mut n = 0;
        for row in self.board.iter() {
            for cell in row.iter() {
                if let Some(p) = cell {
                    if p.side == side {
                        n += 1;
                    }
                }
            }
        }
        n
    }

    pub fn king_sq(&self, side: Side) -> Option<Square> {
        for (r, row) in self.board.iter().enumerate() {
            for (f, cell) in row.iter().enumerate() {
                if let Some(p) = cell {
                    if p.side == side && p.kind == Kind::King {
                        return Some((f, r));
                    }
                }
            }
        }
        None
    }

    pub fn in_check(&self, side: Side) -> bool {
        match self.king_sq(side) {
            Some(k) => attacked(&self.board, k, side.other()),
            None => false,
        }
    }

    pub fn check_square(&self) -> Option<Square> {
        match self.status {
            Status::Check(side) => self.king_sq(side),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn winner(&self) -> Option<Side> {
        match self.status {
            Status::Win { side, .. } => Some(side),
            _ => None,
        }
    }

    pub fn piece_at(&self, sq: Square) -> Option<Piece> {
        self.board[sq.1][sq.0]
    }

    // ---------- выбор и ходы ----------

    pub fn click(&mut self, sq: Square) {
        if self.is_locked() {
            return;
        }
        if self.chain.is_some() {
            if let Some(mv) = self.legal.iter().find(|m| m.to == sq).copied() {
                self.apply(mv);
            }
            return;
        }
        if let Some(sel) = self.selected {
            if let Some(mv) = self.legal.iter().find(|m| m.to == sq && m.from == sel).copied() {
                self.apply(mv);
                return;
            }
        }
        self.select(sq);
    }

    pub fn select(&mut self, sq: Square) {
        let mv = self.moves_for(sq);
        if mv.is_empty() {
            self.selected = None;
            self.legal.clear();
        } else {
            self.selected = Some(sq);
            self.legal = mv;
        }
    }

    pub fn deselect(&mut self) {
        if self.chain.is_none() {
            self.selected = None;
            self.legal.clear();
        }
    }

    pub fn moves_for(&self, sq: Square) -> Vec<Move> {
        if self.is_locked() {
            return Vec::new();
        }
        match self.piece_at(sq) {
            Some(p) if p.side == self.turn => {}
            _ => return Vec::new(),
        }
        if let Some(c) = self.chain {
            if c != sq {
                return Vec::new();
            }
            return self.checker_captures_from(sq);
        }
        match self.phase {
            Phase::Chess => self.chess_from(sq),
            Phase::Checkers => {
                let caps = self.checker_all_captures();
                if !caps.is_empty() {
                    caps.into_iter().filter(|m| m.from == sq).collect()
                } else {
                    self.checker_quiet_from(sq)
                }
            }
        }
    }

    pub fn all_moves(&self) -> Vec<Move> {
        match self.phase {
            Phase::Chess => self.chess_all(),
            Phase::Checkers => {
                if let Some(c) = self.chain {
                    return self.checker_captures_from(c);
                }
                let caps = self.checker_all_captures();
                if caps.is_empty() {
                    self.checker_all_quiet()
                } else {
                    caps
                }
            }
        }
    }

    pub fn apply(&mut self, mv: Move) {
        if self.is_locked() {
            return;
        }
        let from = mv.from;
        let to = mv.to;
        let piece = match self.board[from.1][from.0].take() {
            Some(p) => p,
            None => return,
        };
        let orig_kind = piece.kind;

        if let Some(cap) = mv.captured {
            if let Some(victim) = self.board[cap.1][cap.0].take() {
                match piece.side {
                    Side::White => self.captured_by_white += 1,
                    Side::Black => self.captured_by_black += 1,
                }
                if victim.kind == Kind::King {
                    self.royal_captured = Some(victim.side);
                }
            }
        }

        self.board[to.1][to.0] = Some(piece);
        self.last_move = Some((from, to));
        self.selected = None;
        self.legal.clear();

        // запись хода
        if self.phase == Phase::Checkers {
            if self.pending.is_empty() {
                self.pending = sq_name(from);
            }
            if mv.captured.is_some() {
                self.pending.push('×');
            } else {
                self.pending.push('–');
            }
            self.pending.push_str(&sq_name(to));
        } else {
            let mut s = String::new();
            if self.turn == Side::White {
                self.ply += 1;
                s.push_str(&format!("{}. ", self.ply));
            } else {
                s.push_str(&format!("{}… ", self.ply.max(1)));
            }
            if orig_kind != Kind::Pawn {
                s.push_str(orig_kind.letter());
                s.push(' ');
            }
            s.push_str(&sq_name(from));
            if mv.captured.is_some() {
                s.push('×');
            } else {
                s.push('–');
            }
            s.push_str(&sq_name(to));
            self.pending = s;
        }

        if self.royal_captured.is_some() {
            if self.count(Side::White) == 0 || self.count(Side::Black) == 0 {
                // всё равно победа по королю
            }
            self.refresh_status();
            return;
        }

        // Пешка на последнем ряду: превращение выбирает игрок.
        if self.phase == Phase::Chess && piece.kind == Kind::Pawn {
            let last = if piece.side == Side::White { 7 } else { 0 };
            if to.1 == last {
                self.pending_promotion = Some(to);
                self.chain = None;
                self.passes = 0;
                return;
            }
        }

        // шашечная фаза: продолжение цепочки взятий
        if self.phase == Phase::Checkers && mv.captured.is_some() {
            let more = self.checker_captures_from(to);
            if !more.is_empty() {
                self.chain = Some(to);
                self.selected = Some(to);
                self.legal = more;
                return;
            }
        }

        if self.phase == Phase::Checkers {
            if let Some(mut p) = self.board[to.1][to.0] {
                let last = if p.side == Side::White { 7 } else { 0 };
                if p.kind.becomes_man() && to.1 == last {
                    p.crowned = true;
                    self.board[to.1][to.0] = Some(p);
                    self.crowned_flash = Some((to, 1.0));
                    self.pending.push_str(" (дамка)");
                }
            }
        }

        self.chain = None;
        self.passes = 0;
        self.turn = self.turn.other();
        self.refresh_status();
    }

    /// Игрок выбрал фигуру для превращения пешки.
    pub fn choose_promotion(&mut self, kind: Kind) {
        let Some(sq) = self.pending_promotion else {
            return;
        };
        if !matches!(kind, Kind::Queen | Kind::Rook | Kind::Bishop | Kind::Knight) {
            return;
        }
        self.pending_promotion = None;
        if let Some(mut p) = self.board[sq.1][sq.0] {
            p.kind = kind;
            self.board[sq.1][sq.0] = Some(p);
        }
        self.pending.push('=');
        self.pending.push_str(kind.letter());
        self.chain = None;
        self.passes = 0;
        self.turn = self.turn.other();
        self.refresh_status();
    }

    pub fn tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        self.anim += dt;
        if let Some((_, t)) = self.crowned_flash {
            let t = t - dt * 1.6;
            self.crowned_flash = if t > 0.0 {
                self.crowned_flash.map(|(s, _)| (s, t))
            } else {
                None
            };
        }
        if self.paused || self.is_over() || self.pending_promotion.is_some() {
            return;
        }
        if self.chess_only {
            return;
        }
        // Сначала пропуски: если оба застряли, сумерки не запускаем.
        self.auto_pass();
        if self.phase == Phase::Checkers && self.passes >= 2 {
            self.enter_chess_only();
            return;
        }
        if self.trans > 0.0 {
            self.trans += dt / TRANS_SECS;
            if self.trans >= 1.0 {
                self.trans = 0.0;
                self.flip_phase();
            }
        } else {
            self.phase_left -= dt;
            if self.phase_left <= 0.0 {
                self.phase_left = 0.0;
                // переход ждёт, пока игрок завершит цепочку взятий
                if self.chain.is_none() {
                    self.trans = 0.0001;
                    self.selected = None;
                    self.legal.clear();
                }
            }
        }
    }

    /// В шашках «нечем ходить» — это пропуск хода, а не поражение.
    /// Если застряли оба, переходим в шахматы без сумерек.
    fn auto_pass(&mut self) {
        if self.phase != Phase::Checkers
            || self.trans > 0.0
            || self.chain.is_some()
            || self.is_over()
            || self.passes >= 2
        {
            return;
        }
        if !self.all_moves().is_empty() {
            return;
        }
        let side = self.turn;
        self.turn = self.turn.other();
        self.passes = self.passes.saturating_add(1);
        self.selected = None;
        self.legal.clear();
        let name = match side {
            Side::White => "белым",
            Side::Black => "чёрным",
        };
        self.log.push(format!("— {} нечем ходить: пропуск", name));
        if self.log.len() > 240 {
            self.log.remove(0);
        }
        self.refresh_status();
    }

    /// Оба соперника не могут ходить в шашках: сумерки не запускаем,
    /// партия остаётся в шахматном режиме.
    fn enter_chess_only(&mut self) {
        self.chess_only = true;
        self.phase = Phase::Chess;
        self.phase_left = self.phase_len;
        self.trans = 0.0;
        self.selected = None;
        self.legal.clear();
        self.chain = None;
        self.pending_promotion = None;
        self.passes = 0;
        self.refresh_status();
        self.log.push("— в шашках застряли оба: остаёмся в шахматах".into());
        if self.log.len() > 240 {
            self.log.remove(0);
        }
    }

    fn flip_phase(&mut self) {
        if self.chess_only {
            return;
        }
        self.phase = self.phase.other();
        self.phase_left = self.phase_len;
        self.trans = 0.0;
        self.selected = None;
        self.legal.clear();
        self.chain = None;
        self.pending.clear();
        self.pending_promotion = None;
        self.passes = 0;
        if !self.is_over() {
            self.refresh_status();
        }
    }

    fn refresh_status(&mut self) {
        if let Some(loser) = self.royal_captured {
            self.status = Status::Win {
                side: loser.other(),
                reason: "король взят".into(),
            };
            self.end_turn();
            return;
        }
        if self.count(self.turn.other()) == 0 {
            self.status = Status::Win {
                side: self.turn,
                reason: "у соперника не осталось фигур".into(),
            };
            self.end_turn();
            return;
        }
        let moves = self.all_moves();
        match self.phase {
            Phase::Chess => {
                let checked = self.in_check(self.turn);
                if moves.is_empty() {
                    if checked {
                        self.status = Status::Win {
                            side: self.turn.other(),
                            reason: "мат".into(),
                        };
                    } else {
                        self.status = Status::Draw("пат — ничья".into());
                    }
                } else if checked {
                    self.status = Status::Check(self.turn);
                } else {
                    self.status = Status::Playing;
                }
            }
            Phase::Checkers => {
                // Нет ходов — не поражение: ход просто переходит сопернику.
                // Этим занимается tick().
                self.status = Status::Playing;
            }
        }
        let checked = matches!(self.status, Status::Check(_));
        self.end_turn();
        if checked {
            if let Some(last) = self.log.last_mut() {
                if !last.ends_with('+') {
                    last.push('+');
                }
            }
        }
    }

    fn end_turn(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let line = std::mem::take(&mut self.pending);
        self.log.push(line);
        if self.log.len() > 240 {
            self.log.remove(0);
        }
    }

    // ---------- шахматы ----------

    fn chess_all(&self) -> Vec<Move> {
        let mut out = Vec::new();
        for r in 0..8 {
            for f in 0..8 {
                if let Some(p) = self.board[r][f] {
                    if p.side == self.turn {
                        out.extend(self.chess_from((f, r)));
                    }
                }
            }
        }
        out
    }

    fn chess_from(&self, sq: Square) -> Vec<Move> {
        let p = match self.piece_at(sq) {
            Some(p) if p.side == self.turn => p,
            _ => return Vec::new(),
        };
        let (f, r) = (sq.0 as i32, sq.1 as i32);
        let mut pseudo: Vec<Move> = Vec::new();

        match p.kind {
            Kind::Pawn => {
                let dir = p.side.forward();
                let start_rank = if p.side == Side::White { 1 } else { 6 };
                if self.empty_i(f, r + dir) {
                    push_move(&mut pseudo, sq, (f, r + dir), None);
                    if r == start_rank && self.empty_i(f, r + 2 * dir) {
                        push_move(&mut pseudo, sq, (f, r + 2 * dir), None);
                    }
                }
                for dx in [-1, 1] {
                    if let Some(v) = self.at_i(f + dx, r + dir) {
                        if v.side != p.side {
                            push_move(&mut pseudo, sq, (f + dx, r + dir), Some((f + dx, r + dir)));
                        }
                    }
                }
            }
            Kind::Knight => {
                for (dx, dy) in KNIGHT {
                    if self.inb(f + dx, r + dy) {
                        match self.at_i(f + dx, r + dy) {
                            None => push_move(&mut pseudo, sq, (f + dx, r + dy), None),
                            Some(v) if v.side != p.side => {
                                push_move(&mut pseudo, sq, (f + dx, r + dy), Some((f + dx, r + dy)))
                            }
                            _ => {}
                        }
                    }
                }
            }
            Kind::King => {
                for (dx, dy) in KING {
                    if self.inb(f + dx, r + dy) {
                        match self.at_i(f + dx, r + dy) {
                            None => push_move(&mut pseudo, sq, (f + dx, r + dy), None),
                            Some(v) if v.side != p.side => {
                                push_move(&mut pseudo, sq, (f + dx, r + dy), Some((f + dx, r + dy)))
                            }
                            _ => {}
                        }
                    }
                }
            }
            Kind::Bishop | Kind::Rook | Kind::Queen => {
                let dirs: &[(i32, i32)] = match p.kind {
                    Kind::Bishop => &DIAG,
                    Kind::Rook => &ORTHO,
                    _ => &ALL8,
                };
                for (dx, dy) in dirs {
                    let (mut x, mut y) = (f + dx, r + dy);
                    while self.inb(x, y) {
                        match self.at_i(x, y) {
                            None => {
                                push_move(&mut pseudo, sq, (x, y), None);
                                x += dx;
                                y += dy;
                            }
                            Some(v) => {
                                if v.side != p.side {
                                    push_move(&mut pseudo, sq, (x, y), Some((x, y)));
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }

        pseudo
            .into_iter()
            .filter(|m| self.king_safe(self.turn, m))
            .collect()
    }

    fn king_safe(&self, side: Side, mv: &Move) -> bool {
        let mut b = self.board;
        let piece = match b[mv.from.1][mv.from.0].take() {
            Some(p) => p,
            None => return false,
        };
        if let Some(cap) = mv.captured {
            b[cap.1][cap.0] = None;
        }
        b[mv.to.1][mv.to.0] = Some(piece);
        match find_king(&b, side) {
            Some(k) => !attacked(&b, k, side.other()),
            None => true,
        }
    }

    // ---------- шашки ----------

    fn checker_all_captures(&self) -> Vec<Move> {
        let mut out = Vec::new();
        for r in 0..8 {
            for f in 0..8 {
                if let Some(p) = self.board[r][f] {
                    if p.side == self.turn {
                        out.extend(self.checker_captures_from((f, r)));
                    }
                }
            }
        }
        out
    }

    fn checker_captures_from(&self, sq: Square) -> Vec<Move> {
        let p = match self.piece_at(sq) {
            Some(p) if p.side == self.turn => p,
            _ => return Vec::new(),
        };
        let (f, r) = (sq.0 as i32, sq.1 as i32);
        let mut out = Vec::new();
        if p.is_damka() {
            for (dx, dy) in DIAG {
                let (mut x, mut y) = (f + dx, r + dy);
                while self.inb(x, y) && self.empty_i(x, y) {
                    x += dx;
                    y += dy;
                }
                if !self.inb(x, y) {
                    continue;
                }
                let victim = match self.at_i(x, y) {
                    Some(v) => v,
                    None => continue,
                };
                if victim.side == p.side {
                    continue;
                }
                let (mut lx, mut ly) = (x + dx, y + dy);
                while self.inb(lx, ly) && self.empty_i(lx, ly) {
                    push_move(&mut out, sq, (lx, ly), Some((x, y)));
                    lx += dx;
                    ly += dy;
                }
            }
        } else {
            for (dx, dy) in DIAG {
                let (vx, vy) = (f + dx, r + dy);
                let (lx, ly) = (f + 2 * dx, r + 2 * dy);
                if !self.inb(lx, ly) {
                    continue;
                }
                if let Some(victim) = self.at_i(vx, vy) {
                    if victim.side != p.side && self.empty_i(lx, ly) {
                        push_move(&mut out, sq, (lx, ly), Some((vx, vy)));
                    }
                }
            }
        }
        out
    }

    fn checker_all_quiet(&self) -> Vec<Move> {
        let mut out = Vec::new();
        for r in 0..8 {
            for f in 0..8 {
                if let Some(p) = self.board[r][f] {
                    if p.side == self.turn {
                        out.extend(self.checker_quiet_from((f, r)));
                    }
                }
            }
        }
        out
    }

    fn checker_quiet_from(&self, sq: Square) -> Vec<Move> {
        let p = match self.piece_at(sq) {
            Some(p) if p.side == self.turn => p,
            _ => return Vec::new(),
        };
        let (f, r) = (sq.0 as i32, sq.1 as i32);
        let mut out = Vec::new();
        if p.is_damka() {
            for (dx, dy) in DIAG {
                let (mut x, mut y) = (f + dx, r + dy);
                while self.inb(x, y) && self.empty_i(x, y) {
                    push_move(&mut out, sq, (x, y), None);
                    x += dx;
                    y += dy;
                }
            }
        } else {
            // Простая шашка ходит по диагонали и вперёд, и назад.
            for dy in [-1, 1] {
                for dx in [-1, 1] {
                    if self.empty_i(f + dx, r + dy) {
                        push_move(&mut out, sq, (f + dx, r + dy), None);
                    }
                }
            }
        }
        out
    }

    // ---------- вспомогательное ----------

    fn inb(&self, f: i32, r: i32) -> bool {
        (0..8).contains(&f) && (0..8).contains(&r)
    }

    fn at_i(&self, f: i32, r: i32) -> Option<Piece> {
        if self.inb(f, r) {
            self.board[r as usize][f as usize]
        } else {
            None
        }
    }

    fn empty_i(&self, f: i32, r: i32) -> bool {
        self.inb(f, r) && self.board[r as usize][f as usize].is_none()
    }

    pub fn material_diff(&self) -> i32 {
        let mut white = 0;
        let mut black = 0;
        for row in self.board.iter() {
            for cell in row.iter() {
                if let Some(p) = cell {
                    let v = p.kind.material();
                    match p.side {
                        Side::White => white += v,
                        Side::Black => black += v,
                    }
                }
            }
        }
        white - black
    }
}

const KNIGHT: [(i32, i32); 8] = [
    (1, 2),
    (2, 1),
    (2, -1),
    (1, -2),
    (-1, -2),
    (-2, -1),
    (-2, 1),
    (-1, 2),
];

const KING: [(i32, i32); 8] = [
    (1, 1),
    (1, 0),
    (1, -1),
    (0, 1),
    (0, -1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

const DIAG: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
const ORTHO: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const ALL8: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

fn push_move(out: &mut Vec<Move>, from: Square, to: (i32, i32), cap: Option<(i32, i32)>) {
    out.push(Move {
        from,
        to: (to.0 as usize, to.1 as usize),
        captured: cap.map(|(f, r)| (f as usize, r as usize)),
    });
}

pub fn sq_name(sq: Square) -> String {
    let file = (b'a' + sq.0 as u8) as char;
    format!("{}{}", file, sq.1 + 1)
}

fn find_king(b: &Board, side: Side) -> Option<Square> {
    for (r, row) in b.iter().enumerate() {
        for (f, cell) in row.iter().enumerate() {
            if let Some(p) = cell {
                if p.side == side && p.kind == Kind::King {
                    return Some((f, r));
                }
            }
        }
    }
    None
}

fn attacked(b: &Board, sq: Square, by: Side) -> bool {
    let (f, r) = (sq.0 as i32, sq.1 as i32);
    let pd = by.forward();

    // пешки
    for dx in [-1, 1] {
        if let Some(p) = at_b(b, f + dx, r - pd) {
            if p.side == by && p.kind == Kind::Pawn {
                return true;
            }
        }
    }
    // кони
    for (dx, dy) in KNIGHT {
        if let Some(p) = at_b(b, f + dx, r + dy) {
            if p.side == by && p.kind == Kind::Knight {
                return true;
            }
        }
    }
    // король
    for (dx, dy) in KING {
        if let Some(p) = at_b(b, f + dx, r + dy) {
            if p.side == by && p.kind == Kind::King {
                return true;
            }
        }
    }
    // дальнобойные
    for (dx, dy) in ALL8 {
        let (mut x, mut y) = (f + dx, r + dy);
        loop {
            if !(0..8).contains(&x) || !(0..8).contains(&y) {
                break;
            }
            if let Some(p) = b[y as usize][x as usize] {
                if p.side == by {
                    let diag = dx != 0 && dy != 0;
                    let ok = match p.kind {
                        Kind::Queen => true,
                        Kind::Bishop => diag,
                        Kind::Rook => !diag,
                        _ => false,
                    };
                    if ok {
                        return true;
                    }
                }
                break;
            }
            x += dx;
            y += dy;
        }
    }
    false
}

fn at_b(b: &Board, f: i32, r: i32) -> Option<Piece> {
    if (0..8).contains(&f) && (0..8).contains(&r) {
        b[r as usize][f as usize]
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_game() -> Game {
        Game::new(25.0)
    }

    #[test]
    fn initial_chess_moves() {
        let g = test_game();
        assert_eq!(g.all_moves().len(), 20, "у белых 20 ходов в начале");
        assert!(g.king_sq(Side::White).is_some());
        assert!(g.king_sq(Side::Black).is_some());
    }

    #[test]
    fn pawn_cannot_jump_over_piece() {
        let mut g = test_game();
        g.board[1][4] = None; // e2 убрана
        g.board[2][4] = Some(Piece::new(Side::White, Kind::Pawn)); // e3
        let moves = g.chess_from((4, 2));
        assert!(moves.iter().all(|m| m.to.1 != 4), "e4 занята своей пешкой");
    }

    #[test]
    fn checkers_mandatory_capture() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[3][2] = Some(Piece::new(Side::White, Kind::Pawn)); // c4
        g.board[4][3] = Some(Piece::new(Side::Black, Kind::Pawn)); // d5
        g.turn = Side::White;
        let moves = g.all_moves();
        assert_eq!(moves.len(), 1);
        assert_eq!(moves[0].to, (4, 5)); // e6
        assert_eq!(moves[0].captured, Some((3, 4)));
    }

    #[test]
    fn damka_flies_and_captures_at_distance() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[0][0] = Some(Piece::new(Side::White, Kind::Rook)); // a1 — дамка
        g.board[3][3] = Some(Piece::new(Side::Black, Kind::Pawn)); // d4
        g.turn = Side::White;
        let moves = g.all_moves();
        assert!(moves.iter().any(|m| m.to == (5, 5) && m.captured == Some((3, 3))));
        assert!(moves.iter().any(|m| m.to == (6, 6)));
    }

    #[test]
    fn royal_capture_ends_game_in_checkers() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[3][2] = Some(Piece::new(Side::White, Kind::Pawn)); // c4
        g.board[4][3] = Some(Piece::new(Side::Black, Kind::King)); // d5
        g.turn = Side::White;
        let moves = g.all_moves();
        let mv = moves[0];
        g.apply(mv);
        assert_eq!(g.winner(), Some(Side::White));
    }

    #[test]
    fn phase_flip_changes_rules() {
        let mut g = test_game();
        g.phase_left = 0.05;
        g.tick(0.05);
        assert!(g.trans > 0.0, "переход начался");
        for _ in 0..40 {
            g.tick(0.1);
        }
        assert_eq!(g.phase, Phase::Checkers);
        // в шашках пешка e2 не может пойти на e4
        assert!(g.moves_for((4, 1)).iter().all(|m| m.to != (4, 3)));
        // а по диагонали — может
        assert!(g.moves_for((4, 1)).iter().any(|m| m.to == (3, 2) || m.to == (5, 2)));
    }

    #[test]
    fn check_and_mate() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.board[0][4] = Some(Piece::new(Side::White, Kind::King)); // e1
        g.board[7][7] = Some(Piece::new(Side::Black, Kind::King)); // h8
        g.board[7][0] = Some(Piece::new(Side::White, Kind::Rook)); // a8
        g.board[6][6] = Some(Piece::new(Side::Black, Kind::Pawn)); // g7
        g.board[6][7] = Some(Piece::new(Side::Black, Kind::Pawn)); // h7
        g.turn = Side::Black;
        g.refresh_status();
        assert_eq!(g.winner(), Some(Side::White));
    }

    #[test]
    fn check_is_reported() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.board[0][4] = Some(Piece::new(Side::White, Kind::King)); // e1
        g.board[7][4] = Some(Piece::new(Side::Black, Kind::Rook)); // e8
        g.board[7][7] = Some(Piece::new(Side::Black, Kind::King)); // h8
        g.turn = Side::White;
        assert!(g.in_check(Side::White));
        let moves = g.all_moves();
        assert!(moves.iter().all(|m| m.from == (4, 0)), "из-под шаха только король");
    }

    #[test]
    fn stalemate_is_a_draw() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.board[7][0] = Some(Piece::new(Side::Black, Kind::King)); // a8
        g.board[5][2] = Some(Piece::new(Side::White, Kind::King)); // c6
        g.board[5][1] = Some(Piece::new(Side::White, Kind::Queen)); // b6
        g.turn = Side::Black;
        g.refresh_status();
        assert!(matches!(g.status, Status::Draw(_)), "пат: {:?}", g.status);
    }

    #[test]
    fn pawn_promotes_to_queen() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.board[0][4] = Some(Piece::new(Side::White, Kind::King)); // e1
        g.board[7][7] = Some(Piece::new(Side::Black, Kind::King)); // h8
        g.board[6][0] = Some(Piece::new(Side::White, Kind::Pawn)); // a7
        g.turn = Side::White;
        let mv = g
            .moves_for((0, 6))
            .into_iter()
            .find(|m| m.to == (0, 7))
            .unwrap();
        g.apply(mv);
        assert_eq!(g.pending_promotion, Some((0, 7)), "пешка ждёт выбора");
        assert_eq!(g.turn, Side::White, "ход ещё не передан");
        g.choose_promotion(Kind::Queen);
        assert_eq!(g.piece_at((0, 7)).unwrap().kind, Kind::Queen);
        assert_eq!(g.turn, Side::Black);
        assert!(g.log.last().unwrap().contains("=Ф"));
    }

    #[test]
    fn pawn_can_promote_to_any_piece() {
        for kind in [Kind::Queen, Kind::Rook, Kind::Bishop, Kind::Knight] {
            let mut g = test_game();
            g.board = [[None; 8]; 8];
            g.board[0][4] = Some(Piece::new(Side::White, Kind::King)); // e1
            g.board[7][7] = Some(Piece::new(Side::Black, Kind::King)); // h8
            g.board[6][0] = Some(Piece::new(Side::White, Kind::Pawn)); // a7
            g.turn = Side::White;
            let mv = Move {
                from: (0, 6),
                to: (0, 7),
                captured: None,
            };
            g.apply(mv);
            g.choose_promotion(kind);
            assert_eq!(g.piece_at((0, 7)).unwrap().kind, kind);
            assert_eq!(g.turn, Side::Black);
        }
    }

    #[test]
    fn checkers_can_move_backward() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[3][3] = Some(Piece::new(Side::White, Kind::Pawn)); // d4
        g.turn = Side::White;
        let moves = g.all_moves();
        assert!(moves.iter().any(|m| m.to == (2, 2)), "назад-влево");
        assert!(moves.iter().any(|m| m.to == (4, 2)), "назад-вправо");
        assert!(moves.iter().any(|m| m.to == (2, 4)), "вперёд-влево");
        assert!(moves.iter().any(|m| m.to == (4, 4)), "вперёд-вправо");
    }

    #[test]
    fn checkers_keep_their_square_color() {
        let mut g = test_game();
        g.phase = Phase::Checkers;
        for _ in 0..12 {
            if g.is_over() {
                break;
            }
            for mv in g.all_moves() {
                assert_eq!(
                    (mv.from.0 + mv.from.1) % 2,
                    (mv.to.0 + mv.to.1) % 2,
                    "шашка осталась на своём цвете: {mv:?}"
                );
            }
            let moves = g.all_moves();
            if moves.is_empty() {
                break;
            }
            g.apply(moves[0]);
            g.tick(0.0);
        }
    }

    #[test]
    fn checker_may_move_into_attack() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[3][3] = Some(Piece::new(Side::White, Kind::Pawn)); // d4
        g.board[5][5] = Some(Piece::new(Side::Black, Kind::Pawn)); // f6 — бьёт d4
        g.turn = Side::White;
        // белая шашка может пойти на e5 под удар чёрной с f6
        assert!(
            g.all_moves().iter().any(|m| m.to == (4, 4)),
            "ход под удар разрешён"
        );
    }

    #[test]
    fn checkers_multi_capture_keeps_turn() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[3][2] = Some(Piece::new(Side::White, Kind::Pawn)); // c4
        g.board[4][3] = Some(Piece::new(Side::Black, Kind::Pawn)); // d5
        g.board[6][5] = Some(Piece::new(Side::Black, Kind::Pawn)); // f7
        g.board[7][7] = Some(Piece::new(Side::Black, Kind::King)); // h8
        g.turn = Side::White;
        let first = g
            .all_moves()
            .into_iter()
            .find(|m| m.captured == Some((3, 4)))
            .unwrap();
        g.apply(first);
        assert_eq!(g.turn, Side::White, "серия взятий продолжается");
        assert_eq!(g.chain, Some((4, 5)));
        let second = g.all_moves();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].to, (6, 7)); // g8
        g.apply(second[0]);
        assert_eq!(g.turn, Side::Black);
        let crowned = g.piece_at((6, 7)).unwrap();
        assert_eq!(crowned.kind, Kind::Pawn);
        assert!(crowned.crowned, "дошла до последнего ряда — стала дамкой");
    }

    #[test]
    fn random_games_do_not_break() {
        let mut seed = 0x9E3779B97F4A7C15u64;
        let mut next = move || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as usize
        };
        for _ in 0..4 {
            let mut g = Game::new(15.0);
            for _ in 0..500 {
                if g.is_over() {
                    break;
                }
                let dt = (next() % 4) as f32 * 0.1;
                g.tick(dt);
                if g.trans > 0.0 || g.paused {
                    continue;
                }
                let moves = g.all_moves();
                if moves.is_empty() {
                    break;
                }
                let mv = moves[next() % moves.len()];
                g.apply(mv);
                if g.pending_promotion.is_some() {
                    g.choose_promotion(Kind::Queen);
                }
                assert!(g.count(Side::White) <= 16 && g.count(Side::Black) <= 16);
            }
        }
    }

    #[test]
    fn checkers_no_moves_is_a_pass() {
        let mut g = Game::new(25.0);
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[0][7] = Some(Piece::new(Side::White, Kind::King)); // h1 — дамка
        g.board[0][0] = Some(Piece::new(Side::Black, Kind::Pawn)); // a1
        g.board[1][1] = Some(Piece::new(Side::White, Kind::Pawn)); // b2 — закрывает ход
        g.board[2][2] = Some(Piece::new(Side::White, Kind::Pawn)); // c3 — закрывает взятие
        g.turn = Side::Black;
        g.tick(0.05);
        assert_eq!(g.turn, Side::White, "ход перешёл белым");
        assert_eq!(g.passes, 1);
        assert!(g.log.iter().any(|l| l.contains("пропуск")), "пропуск в журнале");
        assert!(!g.is_over(), "это не поражение");
    }

    #[test]
    fn auto_pass_stops_after_two() {
        let mut g = Game::new(25.0);
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[0][0] = Some(Piece::new(Side::Black, Kind::Pawn)); // a1
        g.board[1][1] = Some(Piece::new(Side::White, Kind::Pawn)); // b2
        g.board[2][2] = Some(Piece::new(Side::White, Kind::Pawn)); // c3
        g.turn = Side::Black;
        g.passes = 2;
        g.auto_pass();
        assert_eq!(g.turn, Side::Black, "после двух пропусков ждём сумерки");
        assert_eq!(g.passes, 2);
    }

    #[test]
    fn both_stuck_switches_to_chess_only() {
        let mut g = Game::new(25.0);
        g.phase = Phase::Checkers;
        for r in 0..8 {
            for f in 0..8 {
                let side = if (r + f) % 2 == 0 {
                    Side::White
                } else {
                    Side::Black
                };
                g.board[r][f] = Some(Piece::new(side, Kind::Pawn));
            }
        }
        g.turn = Side::White;
        for _ in 0..4 {
            g.tick(0.05);
        }
        assert!(g.chess_only, "оба застряли — только шахматы");
        assert_eq!(g.phase, Phase::Chess);
        assert_eq!(g.trans, 0.0, "сумерки не включаются");
        assert_eq!(g.passes, 0);
        assert!(g.log.iter().any(|l| l.contains("остаёмся в шахматах")));
    }

    #[test]
    fn chess_only_does_not_flip_back() {
        let mut g = Game::new(25.0);
        g.chess_only = true;
        g.phase = Phase::Chess;
        g.phase_left = 0.0;
        for _ in 0..200 {
            g.tick(0.1);
        }
        assert_eq!(g.phase, Phase::Chess);
        assert_eq!(g.trans, 0.0);
        assert_eq!(g.phase_left, 0.0, "таймер сумерек выключен");
    }

    fn dump(g: &Game) -> String {
        let mut s = String::new();
        for r in (0..8).rev() {
            for f in 0..8 {
                let c = match g.board[r][f] {
                    None => '.',
                    Some(p) => {
                        let base = match p.kind {
                            Kind::Pawn => 'p',
                            Kind::Knight => 'n',
                            Kind::Bishop => 'b',
                            Kind::Rook => 'r',
                            Kind::Queen => 'q',
                            Kind::King => 'k',
                        };
                        if p.side == Side::White {
                            base.to_ascii_uppercase()
                        } else {
                            base
                        }
                    }
                };
                s.push(c);
                s.push(' ');
            }
            s.push('\n');
        }
        s.push_str(&format!(
            "phase={:?} turn={:?} status={:?} chain={:?} trans={} left={}\n",
            g.phase, g.turn, g.status, g.chain, g.trans, g.phase_left
        ));
        s
    }

    #[test]
    fn checkers_moves_are_reachable() {
        let mut seed = 0x1234_5678_9ABC_DEF0u64;
        let mut next = move || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed >> 33) as usize
        };
        for game_i in 0..200 {
            let mut g = Game::new(25.0);
            g.phase = Phase::Checkers;
            for ply in 0..200 {
                if g.is_over() {
                    break;
                }
                let moves = g.all_moves();
                if moves.is_empty() {
                    assert!(
                        g.passes > 0,
                        "пусто без пропуска: партия {game_i}, полуход {ply}\n{}",
                        dump(&g)
                    );
                    break;
                }
                for mv in &moves {
                    let reachable = g
                        .moves_for(mv.from)
                        .iter()
                        .any(|m| m.to == mv.to && m.captured == mv.captured);
                    if !reachable {
                        panic!(
                            "ход не выбирается кликом: партия {game_i}, полуход {ply}, {mv:?}\n{}",
                            dump(&g)
                        );
                    }
                }
                let mv = moves[next() % moves.len()];
                g.apply(mv);
                g.tick(0.0);
            }
        }
    }
}
