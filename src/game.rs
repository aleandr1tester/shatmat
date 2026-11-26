//! «Сумерки» — правила гибрида шахмат и шашек.
//!
//! Партия идёт на доске 8×8. Раз в N секунд начинается «сумеречный переход»,
//! после которого все фигуры меняют природу: шахматные фигуры начинают ходить
//! по правилам шашек и наоборот. Ходы в момент перехода замирают.

pub const TRANS_SECS: f32 = 1.7;
pub const MOVE_ANIM_SECS: f32 = 0.22;
pub const MIN_PHASE: f32 = 5.0;
pub const MAX_PHASE: f32 = 60.0;
pub const MIN_AUTO_SECS: f32 = 10.0;
/// Время на один ход в режиме «сумерки по ходам».
pub const MIN_MOVE_SECS: f32 = 10.0;
pub const MAX_MOVE_SECS: f32 = 180.0;
pub const DEFAULT_MOVE_SECS: f32 = 60.0;
/// Последние секунды перед сменой: если игрок взял фигуру, таймер ждёт,
/// чтобы ход успел завершиться и его результат был виден до смены игры.
pub const GRACE_SECS: f32 = 5.0;
/// Пауза после хода перед появлением сумерек в режиме «по ходам»:
/// игрок успевает увидеть результат хода.
pub const MOVE_TWILIGHT_PAUSE: f32 = 2.0;
/// Сколько секунд сумерки могут ждать решающую ситуацию за один раз:
/// две отложки по паузе хода — и фаза меняется принудительно.
pub const TWILIGHT_HOLD_MAX: f32 = 4.0;

/// Ничья в мёртвом эндшпиле, если столько полуходов подряд сыграно
/// без единого взятия: одинокого короля с одной фигурой не добить.
pub const NO_CAPTURE_DRAW_PLIES: usize = 20;

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
            Side::White => crate::i18n::tr("Белые"),
            Side::Black => crate::i18n::tr("Чёрные"),
        }
    }

    pub fn gen_ru(self) -> &'static str {
        match self {
            Side::White => crate::i18n::tr("белых"),
            Side::Black => crate::i18n::tr("чёрных"),
        }
    }

    /// Имя стороны для журнала: ru для русской локализации, en для остальных.
    pub fn name_log(self) -> &'static str {
        match self {
            Side::White => crate::i18n::log("Белые"),
            Side::Black => crate::i18n::log("Чёрные"),
        }
    }

    pub fn name_log_lower(self) -> &'static str {
        match self {
            Side::White => crate::i18n::log("белые"),
            Side::Black => crate::i18n::log("чёрные"),
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

    /// Буква фигуры для журнала: русская для русской локализации,
    /// латинская для всех остальных.
    pub fn letter_log(self) -> &'static str {
        if crate::i18n::log_lang() == crate::i18n::Lang::Ru {
            return self.letter();
        }
        match self {
            Kind::Pawn => "P",
            Kind::Knight => "N",
            Kind::Bishop => "B",
            Kind::Rook => "R",
            Kind::Queen => "Q",
            Kind::King => "K",
        }
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
        self.crowned
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
            Phase::Chess => crate::i18n::tr("ШАХМАТЫ"),
            Phase::Checkers => crate::i18n::tr("ШАШКИ"),
        }
    }

    pub fn one_ru(self) -> &'static str {
        match self {
            Phase::Chess => crate::i18n::tr("шахматы"),
            Phase::Checkers => crate::i18n::tr("шашки"),
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

/// Позиция после полухода — для пошагового просмотра партии.
#[derive(Clone, Copy)]
pub struct Snapshot {
    pub board: Board,
    pub turn: Side,
    pub phase: Phase,
    pub last_move: Option<(Square, Square)>,
    pub last_capture: Option<(Square, Piece)>,
}

/// Один полуход для файла лога партии.
#[derive(Clone, Debug)]
pub struct MoveRecord {
    pub side: Side,
    pub kind: Kind,
    /// Был ли ходящий уже дамкой (для шашечной фазы).
    pub damka: bool,
    pub from: Square,
    pub to: Square,
    pub captured: Option<(Side, Kind)>,
    pub phase: Phase,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Status {
    Playing,
    Check(Side),
    Win { side: Side, reason: String },
    /// Ничья: обе стороны остались с королями и не могут зажать друг друга.
    Draw { reason: String },
}

#[derive(Clone)]
pub struct Game {
    pub board: Board,
    pub turn: Side,
    pub phase: Phase,
    /// Длительность шахматной фазы, секунды.
    pub chess_len: f32,
    /// Длительность шашечной фазы, секунды.
    pub checkers_len: f32,
    pub phase_left: f32,
    /// Длительность текущей фазы на момент её начала.
    pub phase_total: f32,
    /// Случайная автосмена сумерек.
    pub auto_phase: bool,
    pub auto_max: f32,
    pub rng_state: u64,
    pub last_phase_len: f32,
    /// 0..1 — прогресс сумеречного перехода (0 — обычная фаза).
    pub trans: f32,
    pub paused: bool,
    pub status: Status,
    pub selected: Option<Square>,
    pub legal: Vec<Move>,
    pub chain: Option<Square>,
    pub last_move: Option<(Square, Square)>,
    /// Последнее взятие: клетка и съеденная фигура (для анимации и повтора).
    pub last_capture: Option<(Square, Piece)>,
    /// Прогресс анимации последнего хода: 0 — начало, 1 — конец.
    pub move_anim: f32,
    /// Длительность анимации хода в секундах (настраивается в меню).
    pub move_anim_secs: f32,
    pub log: Vec<String>,
    /// Структурированная история полуходов для файла лога.
    pub history: Vec<MoveRecord>,
    /// Позиции: начальная и после каждого полухода.
    pub snapshots: Vec<Snapshot>,
    pub ply: u32,
    pub captured_by_white: u32,
    pub captured_by_black: u32,
    pub anim: f32,
    pub crowned_flash: Option<(Square, f32)>,
    /// Сколько ходов подряд пропущено из-за отсутствия ходов в шашках.
    pub passes: u8,
    /// Сколько подряд пустых (без взятия) полуходов сделано, когда у сторон
    /// осталось только по королю — счётчик правила «король против короля».
    pub kings_only_plies: u8,
    /// Пешка дошла до последнего ряда и ждёт, какую фигуру выбрать.
    pub pending_promotion: Option<Square>,
    /// Превращение после смены фазы: это не ход, очередь не передаётся.
    pub promotion_free: bool,
    /// true — пешка на последнем ряду ждёт выбора игрока; false — сразу ферзь.
    pub promotion_choice: bool,
    /// true — бить обязательно; false — можно ходить без взятия.
    pub mandatory_capture: bool,
    /// true — не-пешки тоже становятся дамками; false — только пешки.
    pub crown_non_pawns: bool,
    /// Максимальная цепочка: брать надо самую длинную серию взятий.
    pub max_chain: bool,
    /// Король под боем может уйти вместо обязательного взятия.
    pub king_escape: bool,
    /// Длина шахматной фазы в полных ходах (1–8), режим «по ходам».
    pub chess_moves: u8,
    /// Длина шашечной фазы в полных ходах (1–8), режим «по ходам».
    pub checkers_moves: u8,
    /// Сколько полуходов сыграно в текущей фазе.
    pub plies_in_phase: u16,
    /// true — фаза меняется после хода случайным образом, а не по времени.
    pub twilight_by_moves: bool,
    /// Ход завершён: на следующем тике можно бросить «остаться/смениться».
    pub move_twilight_pending: bool,
    /// Сколько ещё ждать перед решением о сумерках после хода.
    pub move_twilight_wait: f32,
    /// Сколько уже копится удержание сумерек (сбрасывается при смене фазы).
    twilight_hold_secs: f32,
    /// Время на один ход в режиме «по ходам» (10–180 с).
    pub move_time_limit: f32,
    /// Остаток времени на текущий ход.
    pub move_left: f32,
    /// true — фаза ждёт: не тикает таймер (например, ход компьютера).
    pub phase_hold: bool,
    royal_captured: Option<Side>,
    /// true только на время проверки статуса сразу после смены фазы:
    /// мат, найденный в этот момент, — следствие сумерек, а не хода.
    twilight_flip: bool,
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
            chess_len: phase_len.clamp(MIN_PHASE, MAX_PHASE),
            checkers_len: phase_len.clamp(MIN_PHASE, MAX_PHASE),
            phase_left: phase_len.clamp(MIN_PHASE, MAX_PHASE),
            phase_total: phase_len.clamp(MIN_PHASE, MAX_PHASE),
            auto_phase: false,
            auto_max: MAX_PHASE,
            rng_state: 0x9E37_79B9_7F4A_7C15,
            last_phase_len: phase_len.clamp(MIN_PHASE, MAX_PHASE),
            trans: 0.0,
            paused: false,
            status: Status::Playing,
            selected: None,
            legal: Vec::new(),
            chain: None,
            last_move: None,
            last_capture: None,
            move_anim: 1.0,
            move_anim_secs: MOVE_ANIM_SECS,
            log: Vec::new(),
            history: Vec::new(),
            snapshots: vec![Snapshot {
                board,
                turn: Side::White,
                phase: Phase::Chess,
                last_move: None,
                last_capture: None,
            }],
            ply: 0,
            captured_by_white: 0,
            captured_by_black: 0,
            anim: 0.0,
            crowned_flash: None,
            passes: 0,
            kings_only_plies: 0,
            pending_promotion: None,
            promotion_free: false,
            promotion_choice: false,
            mandatory_capture: true,
            crown_non_pawns: true,
            max_chain: true,
            king_escape: true,
            chess_moves: 6,
            checkers_moves: 6,
            plies_in_phase: 0,
            twilight_by_moves: true,
            move_twilight_pending: false,
            move_twilight_wait: 0.0,
            twilight_hold_secs: 0.0,
            move_time_limit: DEFAULT_MOVE_SECS,
            move_left: DEFAULT_MOVE_SECS,
            phase_hold: false,
            royal_captured: None,
            twilight_flip: false,
            pending: String::new(),
        }
    }

    // ---------- состояние ----------

    fn push_snapshot(&mut self) {
        self.snapshots.push(Snapshot {
            board: self.board,
            turn: self.turn,
            phase: self.phase,
            last_move: self.last_move,
            last_capture: self.last_capture,
        });
    }

    /// На доске ровно по королю у каждой стороны — и больше ничего.
    fn kings_only(&self) -> bool {
        let mut n = 0;
        let mut white = false;
        let mut black = false;
        for row in &self.board {
            for cell in row {
                if let Some(p) = cell {
                    if p.kind != Kind::King {
                        return false;
                    }
                    n += 1;
                    match p.side {
                        Side::White => white = true,
                        Side::Black => black = true,
                    }
                }
            }
        }
        n == 2 && white && black
    }

    /// Может ли кто-то прямо сейчас взять короля: в шашечной фазе король
    /// бьётся как дамка, и тогда позиция не ничейная, а выигрышная.
    fn king_capture_possible(&self) -> bool {
        if self.phase != Phase::Checkers {
            return false;
        }
        for r in 0..8 {
            for f in 0..8 {
                if let Some(p) = self.board[r][f] {
                    if p.kind == Kind::King && !self.checker_captures_from((f, r)).is_empty() {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Учёт полухода для правила «король против короля».
    fn note_kings_only_ply(&mut self, captured: bool) {
        if captured || !self.kings_only() {
            self.kings_only_plies = 0;
        } else {
            self.kings_only_plies = self.kings_only_plies.saturating_add(1);
        }
    }

    /// Может ли сторона, чей сейчас ход, съесть чужого короля (в шашках).
    pub fn king_capture_available(&self) -> bool {
        if self.phase != Phase::Checkers {
            return false;
        }
        let Some(enemy) = self.king_sq(self.turn.other()) else {
            return false;
        };
        self.all_moves()
            .iter()
            .any(|m| m.captured == Some(enemy))
    }

    /// Два короля стоят вплотную (по вертикали, горизонтали или диагонали):
    /// в шахматах такая позиция невозможна — они «бьют» друг друга.
    fn kings_adjacent(&self) -> bool {
        let (Some(a), Some(b)) = (
            self.king_sq(Side::White),
            self.king_sq(Side::Black),
        ) else {
            return false;
        };
        (a.0 as i32 - b.0 as i32).abs() <= 1 && (a.1 as i32 - b.1 as i32).abs() <= 1
    }

    /// Есть ли в текущем режиме решающая ситуация, которую нельзя обрывать
    /// сменой фазы: в шашках — король под боем (взятие отменилось бы),
    /// в шахматах — шах, который надо отыграть.
    fn twilight_should_wait(&self) -> bool {
        if self.is_over() {
            return false;
        }
        match self.phase {
            // Короля можно съесть или короли стоят вплотную: смена фазы
            // отменила бы взятие либо дала бы невозможную шахматную позицию.
            Phase::Checkers => self.king_capture_available() || self.kings_adjacent(),
            Phase::Chess => self.in_check(self.turn),
        }
    }

    /// Мёртвый эндшпиль: у одной стороны только король, у другой —
    /// король и ровно одна сильная фигура (ферзь, ладья, слон, конь,
    /// дамка — но не простая пешка/шашка): такую позицию не доиграть.
    fn lone_king_endgame(&self) -> bool {
        let mut sides: [Vec<Piece>; 2] = [Vec::new(), Vec::new()];
        for row in self.board.iter() {
            for cell in row.iter() {
                let Some(p) = cell else {
                    continue;
                };
                sides[usize::from(p.side == Side::Black)].push(*p);
            }
        }
        let is_king = |p: &Piece| p.kind == Kind::King;
        let is_strong = |p: &Piece| {
            !is_king(p) && (p.crowned || !matches!(p.kind, Kind::Pawn))
        };
        let lone = |v: &Vec<Piece>| v.len() == 1 && is_king(&v[0]);
        let one_strong = |v: &Vec<Piece>| {
            v.iter().filter(|p| is_king(p)).count() == 1
                && v.iter().filter(|p| is_strong(p)).count() == 1
        };
        (lone(&sides[0]) && one_strong(&sides[1]))
            || (lone(&sides[1]) && one_strong(&sides[0]))
    }

    /// Сколько полуходов подряд сыграно без взятий.
    pub fn no_capture_plies(&self) -> usize {
        self.history
            .iter()
            .rev()
            .take_while(|m| m.captured.is_none())
            .count()
    }

    /// Король стороны под боем: соперник может взять его следующим ходом.
    pub fn king_under_attack(&self, side: Side) -> bool {
        if self.phase != Phase::Checkers {
            return false;
        }
        let Some(king) = self.king_sq(side) else {
            return false;
        };
        for r in 0..8 {
            for f in 0..8 {
                let Some(p) = self.board[r][f] else {
                    continue;
                };
                if p.side != side.other() {
                    continue;
                }
                if can_capture_at(&self.board, (f as i32, r as i32), p, king) {
                    return true;
                }
            }
        }
        false
    }

    /// Шахматная фаза и смена уже на следующем ходу: окажется ли король
    /// под боем сразу после переворота — по шашечной геометрии текущей доски.
    pub fn king_danger_after_flip(&self, side: Side) -> bool {
        if self.phase != Phase::Chess || self.is_over() {
            return false;
        }
        let imminent = if self.twilight_by_moves {
            self.full_moves_to_switch() <= 1
        } else {
            self.phase_left <= 12.0
        };
        if !imminent {
            return false;
        }
        let Some(king) = self.king_sq(side) else {
            return false;
        };
        for r in 0..8 {
            for f in 0..8 {
                let Some(p) = self.board[r][f] else {
                    continue;
                };
                if p.side != side.other() {
                    continue;
                }
                if can_capture_at(&self.board, (f as i32, r as i32), p, king) {
                    return true;
                }
            }
        }
        false
    }

    pub fn is_over(&self) -> bool {
        matches!(self.status, Status::Win { .. } | Status::Draw { .. })
    }

    pub fn is_locked(&self) -> bool {
        self.is_over() || self.trans > 0.0 || self.paused || self.pending_promotion.is_some()
    }

    pub fn twilight_pending(&self) -> bool {
        self.trans == 0.0 && self.phase_left <= 0.0 && !self.is_over()
    }

    /// Остаток таймера текущего режима: фаза по времени или часы хода.
    pub fn active_left(&self) -> f32 {
        if self.twilight_by_moves {
            self.move_left
        } else {
            self.phase_left
        }
    }

    /// Игрок держит фигуру в последние 5 секунд до смены — таймер замирает,
    /// чтобы ход успел завершиться и его результат был виден до смены игры.
    pub fn grace_hold(&self) -> bool {
        !self.is_over()
            && !self.paused
            && self.trans == 0.0
            && self.selected.is_some()
            && self.active_left() <= GRACE_SECS
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

    /// Клетки шашек, которые обязаны бить сейчас (для красной пульсации).
    pub fn forced_capture_squares(&self) -> Vec<Square> {
        if self.phase != Phase::Checkers
            || !self.mandatory_capture
            || self.is_over()
            || self.trans > 0.0
            || self.pending_promotion.is_some()
        {
            return Vec::new();
        }
        let mut out = Vec::new();
        for mv in self.all_moves() {
            if mv.captured.is_some()
                && !out.contains(&mv.from)
                && !matches!(board_at(&self.board, mv.from).map(|p| p.kind), Some(Kind::King))
            {
                out.push(mv.from);
            }
        }
        out
    }

    pub fn king_sq(&self, side: Side) -> Option<Square> {
        find_king(&self.board, side)
    }

    /// Клетки фигур стороны `by`, которые атакуют клетку `target` по
    /// шахматным правилам (для линий мата).
    pub fn attackers_to(&self, target: Square, by: Side) -> Vec<(Square, Kind)> {
        let mut out = Vec::new();
        for r in 0..8 {
            for f in 0..8 {
                if let Some(p) = self.board[r][f] {
                    if p.side == by && piece_attacks(&self.board, (f, r), p, target) {
                        out.push(((f, r), p.kind));
                    }
                }
            }
        }
        out
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

    pub fn len_for(&self, phase: Phase) -> f32 {
        match phase {
            Phase::Chess => self.chess_len,
            Phase::Checkers => self.checkers_len,
        }
    }

    /// Меняет длительность фазы; если она идёт сейчас — перезапускает отсчёт.
    pub fn set_len(&mut self, phase: Phase, secs: f32) {
        let secs = secs.clamp(MIN_PHASE, MAX_PHASE);
        match phase {
            Phase::Chess => self.chess_len = secs,
            Phase::Checkers => self.checkers_len = secs,
        }
        if self.phase == phase && self.trans == 0.0 {
            self.phase_left = secs;
            self.phase_total = secs;
        }
    }

    fn next_rand(&mut self) -> f32 {
        let mut x = self.rng_state.max(1);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng_state = x;
        (x % 10_000) as f32 / 10_000.0
    }

    /// Случайная длительность следующей фазы: 10..auto_max секунд,
    /// причём не совпадает с предыдущей, если диапазон это позволяет.
    pub fn random_phase_len(&mut self) -> f32 {
        let max = self.auto_max.clamp(MIN_AUTO_SECS, MAX_PHASE);
        let min = MIN_AUTO_SECS;
        if max <= min {
            self.last_phase_len = min;
            return min;
        }
        let mut value = min;
        for _ in 0..10 {
            value = (min + (max - min) * self.next_rand()).round();
            if (value - self.last_phase_len).abs() >= 1.0 {
                break;
            }
        }
        if (value - self.last_phase_len).abs() < 1.0 {
            value = if value + 1.0 <= max {
                value + 1.0
            } else {
                (value - 1.0).max(min)
            };
        }
        self.last_phase_len = value;
        value
    }

    /// Запускает отсчёт текущей фазы: фиксированный или случайный.
    pub fn refresh_phase_timer(&mut self) {
        self.phase_left = if self.auto_phase {
            self.random_phase_len()
        } else {
            self.len_for(self.phase)
        };
        self.phase_total = self.phase_left;
    }

    pub fn set_auto(&mut self, on: bool) {
        self.auto_phase = on;
        self.refresh_phase_timer();
    }

    pub fn set_twilight_by_moves(&mut self, on: bool) {
        self.twilight_by_moves = on;
        self.move_twilight_pending = false;
        self.move_twilight_wait = 0.0;
        self.plies_in_phase = 0;
        if on {
            self.move_left = self.move_time_limit;
        } else {
            self.refresh_phase_timer();
        }
    }

    /// Длина фазы в полных ходах: 4, 6 или 8. Смена идёт строго после хода каждой
    /// стороны, поэтому чётность полуходов не даёт перевеса.
    /// Завершить ожидающие сумерки сразу, без анимации: нужно перебору,
    /// чтобы видеть позицию уже в новой природе.
    /// Сбросить тяжёлые для клонирования поля: журнал и снимки в переборе
    /// не нужны, а копируются они на каждом узле.
    pub fn trim_for_search(&mut self) {
        self.log.clear();
        self.snapshots.clear();
        self.pending.clear();
    }

    pub fn settle_twilight(&mut self) {
        if self.move_twilight_pending {
            self.move_twilight_pending = false;
            self.move_twilight_wait = 0.0;
            if !self.is_over() {
                self.trans = 1.0;
            }
        }
        if self.trans > 0.0 {
            self.trans = 0.0;
            self.flip_phase();
        }
    }

    /// Длина фазы в полных ходах для указанной природы.
    pub fn phase_len(&self, phase: Phase) -> u8 {
        match phase {
            Phase::Chess => self.chess_moves,
            Phase::Checkers => self.checkers_moves,
        }
        .max(1)
    }

    /// Задаёт длину фазы в полных ходах отдельно для шахмат и шашек (1–8).
    pub fn set_phase_len(&mut self, phase: Phase, n: u8) {
        let n = n.clamp(1, 8);
        match phase {
            Phase::Chess => self.chess_moves = n,
            Phase::Checkers => self.checkers_moves = n,
        }
        self.plies_in_phase = 0;
        self.move_twilight_pending = false;
    }

    /// Сколько полных ходов осталось до смены фаз (для интерфейса).
    pub fn full_moves_to_switch(&self) -> u8 {
        if !self.twilight_by_moves {
            return 0;
        }
        let total = (self.phase_len(self.phase) as u16) * 2;
        let left = total.saturating_sub(self.plies_in_phase);
        ((left + 1) / 2) as u8
    }

    /// Время на один ход: 10–180 с. Текущий отсчёт подрезается под новый лимит.
    pub fn set_move_time_limit(&mut self, secs: f32) {
        self.move_time_limit = secs.clamp(MIN_MOVE_SECS, MAX_MOVE_SECS);
        self.move_left = self.move_left.min(self.move_time_limit);
    }

    fn arm_move_twilight(&mut self) {
        if self.twilight_by_moves {
            self.move_left = self.move_time_limit;
        }
        if !self.twilight_by_moves
            || self.is_over()
            || self.trans > 0.0
            || self.chain.is_some()
            || self.pending_promotion.is_some()
        {
            return;
        }
        self.plies_in_phase += 1;
        if self.plies_in_phase >= (self.phase_len(self.phase) as u16) * 2 {
            self.plies_in_phase = 0;
            self.move_twilight_pending = true;
            self.move_twilight_wait = MOVE_TWILIGHT_PAUSE;
        }
    }

    pub fn set_auto_max(&mut self, secs: f32) {
        self.auto_max = secs.clamp(MIN_AUTO_SECS, MAX_PHASE);
    }

    pub fn seed_rng(&mut self, seed: u64) {
        self.rng_state = seed.max(1);
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
            Phase::Checkers => self
                .checkers_all()
                .into_iter()
                .filter(|m| m.from == sq)
                .collect(),
        }
    }

    pub fn all_moves(&self) -> Vec<Move> {
        match self.phase {
            Phase::Chess => self.chess_all(),
            Phase::Checkers => self.checkers_all(),
        }
    }

    /// Все допустимые ходы в шашечной фазе: одно место правил, чтобы клик
    /// и перебор ходов не расходились.
    fn checkers_all(&self) -> Vec<Move> {
        {
            {
                if let Some(c) = self.chain {
                    let caps = self.checker_captures_from(c);
                    return if self.max_chain {
                        self.only_longest(caps, c)
                    } else {
                        caps
                    };
                }
                if !self.mandatory_capture {
                    let mut moves = self.checker_all_quiet();
                    moves.extend(self.checker_all_captures());
                    return moves;
                }
                // Обязательный бой — для всех шашек, кроме короля: он бьёт
                // только по желанию. Но если король бьёт, это тоже взятие,
                // поэтому его ходы не «прикалываются», пока бьёт шашка.
                let all = self.checker_all_captures();
                let is_king = |m: &Move| {
                    matches!(board_at(&self.board, m.from).map(|p| p.kind), Some(Kind::King))
                };
                let mut forced: Vec<Move> = all.iter().copied().filter(|m| !is_king(m)).collect();
                let kings: Vec<Move> = all.into_iter().filter(is_king).collect();
                if forced.is_empty() {
                    let mut moves = self.checker_all_quiet();
                    // Взятия короля необязательны, но доступны.
                    for mv in kings {
                        if !moves.iter().any(|m| m.from == mv.from && m.to == mv.to) {
                            moves.push(mv);
                        }
                    }
                    return moves;
                }
                // Король под боем: правило «король дороже боя» разрешает
                // увести его вместо обязательного взятия. Иначе игрок
                // обязан бить и теряет короля без выбора.
                if self.king_escape && self.king_under_attack(self.turn) {
                    if let Some(king) = self.king_sq(self.turn) {
                        for mv in self.checker_quiet_from(king) {
                            if !forced.iter().any(|m| m.from == mv.from && m.to == mv.to) {
                                forced.push(mv);
                            }
                        }
                    }
                }
                // «Максимальная цепочка»: длинную серию обходить нельзя.
                let need = if self.max_chain {
                    self.longest_run_from_any().max(1)
                } else {
                    1
                };
                if self.max_chain && need > 1 {
                    forced.retain(|m| 1 + run_after(&self.board, *m) == need);
                    for mv in kings {
                        // Взять короля можно всегда: это победа, а не серия.
                        let takes_royal = mv
                            .captured
                            .and_then(|c| board_at(&self.board, c))
                            .map(|p| p.kind == Kind::King)
                            .unwrap_or(false);
                        if takes_royal || 1 + run_after(&self.board, mv) >= need {
                            forced.push(mv);
                        }
                    }
                } else {
                    forced.extend(kings);
                }
                forced
            }
        }
    }

    pub fn apply(&mut self, mv: Move) {
        if self.is_locked() {
            return;
        }
        if self.phase == Phase::Chess {
            if let Some(cap) = mv.captured {
                if self
                    .board
                    .get(cap.1)
                    .and_then(|row| row.get(cap.0))
                    .and_then(|p| *p)
                    .map(|p| p.kind == Kind::King)
                    .unwrap_or(false)
                {
                    // В шахматах короля не берут: это шах или мат.
                    return;
                }
            }
        }
        self.move_anim = 0.0;
        self.last_capture = None;
        let from = mv.from;
        let to = mv.to;
        let piece = match self.board[from.1][from.0].take() {
            Some(p) => p,
            None => return,
        };
        let orig_kind = piece.kind;
        let mut captured_info = None;

        if let Some(cap) = mv.captured {
            if let Some(victim) = self.board[cap.1][cap.0].take() {
                match piece.side {
                    Side::White => self.captured_by_white += 1,
                    Side::Black => self.captured_by_black += 1,
                }
                if victim.kind == Kind::King {
                    self.royal_captured = Some(victim.side);
                }
                captured_info = Some((victim.side, victim.kind));
                self.last_capture = Some((cap, victim));
            }
        }

        self.board[to.1][to.0] = Some(piece);
        self.last_move = Some((from, to));
        // Ничья «король против короля»: считаем пустые полуходы.
        self.note_kings_only_ply(mv.captured.is_some());
        self.selected = None;
        self.legal.clear();
        self.history.push(MoveRecord {
            side: piece.side,
            kind: orig_kind,
            damka: piece.is_damka(),
            from,
            to,
            captured: captured_info,
            phase: self.phase,
        });

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
            self.push_snapshot();
            self.passes = 0;
            // Ход всё равно переходит сопернику: на этом держится оценка
            // «победа/поражение» у ИИ (после победы ход у проигравшего).
            self.turn = self.turn.other();
            self.refresh_status();
            return;
        }

        // Пешка на последнем ряду: по умолчанию сразу ферзь,
        // выбор фигуры включается в настройках.
        if self.phase == Phase::Chess && piece.kind == Kind::Pawn {
            let last = if piece.side == Side::White { 7 } else { 0 };
            if to.1 == last {
                if self.promotion_choice {
                    self.pending_promotion = Some(to);
                    self.promotion_free = false;
                    self.chain = None;
                    self.passes = 0;
                    self.push_snapshot();
                    return;
                }
                let mut queen = piece;
                queen.kind = Kind::Queen;
                self.board[to.1][to.0] = Some(queen);
                self.pending.push_str("=Ф");
            }
        }

        // Шашечная фаза: сначала коронация, затем продолжение серии.
        // Шашка, дошедшая до последнего ряда, становится дамкой и
        // продолжает серию уже как дамка.
        if self.phase == Phase::Checkers {
            if let Some(mut p) = self.board[to.1][to.0] {
                let last = if p.side == Side::White { 7 } else { 0 };
                if to.1 == last && (self.crown_non_pawns || p.kind == Kind::Pawn) {
                    p.crowned = true;
                    self.board[to.1][to.0] = Some(p);
                    self.crowned_flash = Some((to, 1.0));
                    self.pending.push_str(" (дамка)");
                }
            }
            if mv.captured.is_some() {
                let more = self.checker_captures_from(to);
                if !more.is_empty() {
                    self.chain = Some(to);
                    self.selected = Some(to);
                    self.legal = more;
                    self.push_snapshot();
                    return;
                }
            }
        }

        self.push_snapshot();
        self.chain = None;
        self.passes = 0;
        self.turn = self.turn.other();
        self.refresh_status();
        self.arm_move_twilight();
    }

    /// Завершить серию взятий, не продолжая её (когда взятие необязательно).
    pub fn end_chain(&mut self) {
        if self.chain.is_none() {
            return;
        }
        self.chain = None;
        self.selected = None;
        self.legal.clear();
        self.passes = 0;
        self.turn = self.turn.other();
        self.refresh_status();
        self.arm_move_twilight();
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
        let free = self.promotion_free;
        self.promotion_free = false;
        self.move_anim = 1.0;
        if let Some(mut p) = self.board[sq.1][sq.0] {
            p.kind = kind;
            self.board[sq.1][sq.0] = Some(p);
        }
        if let Some(last) = self.snapshots.last_mut() {
            last.board = self.board;
        }
        if free {
            self.log.push(crate::i18n::logf(
                "{side} — пешка {sq} стала {piece}",
                &[
                    ("side", self.turn.name_log()),
                    ("sq", &sq_name(sq)),
                    ("piece", kind.letter_log()),
                ],
            ));
        } else {
            self.pending.push('=');
            self.pending.push_str(kind.letter());
        }
        self.chain = None;
        self.passes = 0;
        if !free {
            self.turn = self.turn.other();
        }
        self.refresh_status();
        if !free {
            self.arm_move_twilight();
        }
    }

    pub fn tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        self.anim += dt;
        if self.move_anim < 1.0 {
            self.move_anim =
                (self.move_anim + dt / self.move_anim_secs.max(0.03)).min(1.0);
        }
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
        // Сначала пропуски: если оба застряли, сумерки выводят из тупика.
        self.auto_pass();
        if self.phase == Phase::Checkers && self.passes >= 2 {
            self.start_rescue_twilight();
            return;
        }
        if self.trans > 0.0 {
            self.trans += dt / TRANS_SECS;
            if self.trans >= 1.0 {
                self.trans = 0.0;
                self.flip_phase();
            }
        } else if self.twilight_by_moves {
            if self.move_twilight_pending
                && self.move_anim >= 1.0
                && self.chain.is_none()
                && self.selected.is_none()
            {
                // Пауза после хода: игрок видит результат до смены игры.
                self.move_twilight_wait -= dt;
                if self.move_twilight_wait <= 0.0 {
                    let may_hold = self.twilight_hold_secs < TWILIGHT_HOLD_MAX;
                    if self.twilight_should_wait() && may_hold {
                        // Решающий момент: король под боем или шах. Смена
                        // фазы сейчас отменила бы сам смысл хода — ждём,
                        // пока ситуация не разрешится следующим ходом.
                        // Дольше двух отложек подряд держать нельзя.
                        self.twilight_hold_secs += MOVE_TWILIGHT_PAUSE;
                        self.move_twilight_pending = false;
                        self.move_twilight_wait = 0.0;
                        let threshold = (self.phase_len(self.phase) as u16) * 2;
                        self.plies_in_phase = threshold.saturating_sub(1);
                    } else {
                        // Фаза меняется по счёту ходов: обе стороны знают,
                        // сколько полных ходов осталось до смены природы.
                        self.move_twilight_pending = false;
                        self.trans = 0.0001;
                        self.selected = None;
                        self.legal.clear();
                    }
                }
            }
            // Часы хода: по истечении ход переходит противнику.
            // В последние 5 секунд игрок, взявший фигуру, задерживает часы:
            // ход должен завершиться и показать результат до смены игры.
            if self.trans == 0.0
                && !self.phase_hold
                && !self.grace_hold()
                && self.move_anim >= 1.0
                && self.chain.is_none()
            {
                self.move_left -= dt;
                if self.move_left <= 0.0 {
                    self.move_left = 0.0;
                    self.turn = self.turn.other();
                    self.selected = None;
                    self.legal.clear();
                    self.move_left = self.move_time_limit;
                    self.refresh_status();
                    self.log.push(
                        crate::i18n::log("— время хода истекло: ход переходит противнику").to_string(),
                    );
                    if self.log.len() > 240 {
                        self.log.remove(0);
                    }
                }
            }
        } else if !self.phase_hold
            && !self.grace_hold()
            && self.move_anim >= 1.0
            && self.chain.is_none()
        {
            self.phase_left -= dt;
            if self.phase_left <= 0.0 {
                self.phase_left = 0.0;
                // переход ждёт, пока игрок завершит цепочку взятий
                if self.chain.is_none() {
                    if self.twilight_should_wait()
                        && self.twilight_hold_secs < TWILIGHT_HOLD_MAX
                    {
                        // Решающая ситуация: ждём, но не дольше лимита.
                        self.twilight_hold_secs += dt;
                    } else {
                        self.trans = 0.0001;
                        self.selected = None;
                        self.legal.clear();
                    }
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
            Side::White => crate::i18n::log("белым"),
            Side::Black => crate::i18n::log("чёрным"),
        };
        self.log.push(crate::i18n::logf(
            "— {side} нечем ходить: пропуск",
            &[("side", name)],
        ));
        if self.log.len() > 240 {
            self.log.remove(0);
        }
        self.refresh_status();
    }

    /// Оба соперника не могут ходить в шашках: сумерки запускаются сразу
    /// и временно переводят игру в шахматы, чтобы выйти из тупика.
    fn start_rescue_twilight(&mut self) {
        if self.trans > 0.0 {
            return;
        }
        self.phase_left = 0.0;
        self.trans = 0.0001;
        self.selected = None;
        self.legal.clear();
        self.chain = None;
        self.passes = 0;
        self.log.push(
            crate::i18n::log("— в шашках застряли оба: сумерки выводят в шахматы").to_string(),
        );
        if self.log.len() > 240 {
            self.log.remove(0);
        }
    }

    fn start_stalemate_twilight(&mut self) {
        if self.trans > 0.0 {
            return;
        }
        self.trans = 0.0001;
        self.selected = None;
        self.legal.clear();
        self.chain = None;
        self.log.push(crate::i18n::log("— пат: сумерки продолжают бой").to_string());
        if self.log.len() > 240 {
            self.log.remove(0);
        }
    }

    fn flip_phase(&mut self) {
        self.phase = self.phase.other();
        self.twilight_hold_secs = 0.0;
        self.refresh_phase_timer();
        self.trans = 0.0;
        self.selected = None;
        self.legal.clear();
        self.chain = None;
        self.pending.clear();
        self.pending_promotion = None;
        self.promotion_free = false;
        self.last_capture = None;
        self.move_anim = 1.0;
        self.passes = 0;
        self.plies_in_phase = 0;
        // «Дамки: все»: при переходе в шашки дамками становятся все фигуры
        // на последнем ряду каждой стороны, а не только дошедшие ходом.
        let mut mass_crowned = false;
        if self.phase == Phase::Checkers && self.crown_non_pawns && !self.is_over() {
            for r in 0..8 {
                for f in 0..8 {
                    let Some(p) = self.board[r][f] else {
                        continue;
                    };
                    let last = if p.side == Side::White { 7 } else { 0 };
                    if r == last && !p.crowned {
                        let mut d = p;
                        d.crowned = true;
                        self.board[r][f] = Some(d);
                        mass_crowned = true;
                    }
                }
            }
        }
        if self.phase == Phase::Chess && !self.is_over() {
            self.promote_pawns_after_flip();
        }
        // Принудительная смена фазы, а короли стоят вплотную: позиция в
        // шахматах невозможна, поэтому взятие не отменяется — тот, чей ход,
        // забирает чужого короля.
        if self.phase == Phase::Chess && self.kings_adjacent() {
            self.royal_captured = Some(self.turn.other());
            self.pending_promotion = None;
        }
        if !self.is_over() && self.pending_promotion.is_none() {
            self.resolve_transition_check();
            self.twilight_flip = true;
            self.refresh_status();
            self.twilight_flip = false;
            if mass_crowned {
                self.log.push(
                    crate::i18n::log("— «Дамки: все»: фигуры на последнем ряду стали дамками")
                        .to_string(),
                );
                if self.log.len() > 240 {
                    self.log.remove(0);
                }
            }
        }
    }

    /// Если после перехода в шахматы под шахом оказался только что
    /// ходивший король, передаём ему ход, чтобы он мог уйти из-под удара,
    /// а не даём сопернику просто «съесть» короля.
    fn resolve_transition_check(&mut self) {
        if self.phase != Phase::Chess
            || self.is_over()
            || self.pending_promotion.is_some()
        {
            return;
        }
        let mover = self.turn;
        if self.in_check(mover.other()) && !self.in_check(mover) {
            self.turn = mover.other();
        }
    }

    /// После перехода в шахматы пешки на последнем ряду не должны оставаться
    /// пешками. Одну пешку ходящей стороны показываем в диалоге выбора,
    /// остальные автоматически становятся ферзями.
    fn promote_pawns_after_flip(&mut self) {
        let mut prompt = None;
        for r in 0..8 {
            for f in 0..8 {
                let Some(p) = self.board[r][f] else {
                    continue;
                };
                if p.kind != Kind::Pawn {
                    continue;
                }
                let last = if p.side == Side::White { 7 } else { 0 };
                if r != last {
                    continue;
                }
                let give_choice =
                    self.promotion_choice && p.side == self.turn && prompt.is_none();
                if give_choice {
                    prompt = Some((f, r));
                } else {
                    let mut q = p;
                    q.kind = Kind::Queen;
                    self.board[r][f] = Some(q);
                    self.log.push(crate::i18n::logf(
                        "{side} — пешка {sq} стала ферзём",
                        &[("side", p.side.name_log()), ("sq", &sq_name((f, r)))],
                    ));
                }
            }
        }
        if self.log.len() > 240 {
            self.log.remove(0);
        }
        if let Some(sq) = prompt {
            self.pending_promotion = Some(sq);
            self.promotion_free = true;
            self.selected = None;
            self.legal.clear();
        }
    }

    // ---------- перенос состояния (реконнект) ----------

    /// Полное состояние партии для продолжения после переподключения.
    /// Включает состояние генератора случайности: без него хеши позиций
    /// сторон разъедутся на первой же случайной смене фаз.
    pub fn export_state(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(300);
        b.extend_from_slice(b"SMST");
        b.push(1); // версия формата
        b.push(match self.phase {
            Phase::Chess => 0,
            Phase::Checkers => 1,
        });
        b.push(match self.turn {
            Side::White => 0,
            Side::Black => 1,
        });
        b.push(self.twilight_by_moves as u8);
        b.push(self.auto_phase as u8);
        b.push(self.mandatory_capture as u8);
        b.push(self.crown_non_pawns as u8);
        b.push(self.promotion_choice as u8);
        b.push(self.promotion_free as u8);
        b.push(self.passes);
        b.push(match self.royal_captured {
            None => 0xFF,
            Some(s) => match s {
                Side::White => 0,
                Side::Black => 1,
            },
        });
        for v in [
            self.phase_left,
            self.phase_total,
            self.chess_len,
            self.checkers_len,
            self.auto_max,
            self.move_time_limit,
            self.move_left,
            self.move_anim_secs,
            self.move_twilight_wait,
        ] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.push(self.move_twilight_pending as u8);
        // Сумеречный переход в момент обрыва: обе стороны завершат его
        // одинаково, потому что зерно генератора переносится вместе.
        b.extend_from_slice(&self.trans.to_le_bytes());
        b.extend_from_slice(&self.rng_state.to_le_bytes());
        b.extend_from_slice(&self.ply.to_le_bytes());
        b.extend_from_slice(&(self.captured_by_white as u32).to_le_bytes());
        b.extend_from_slice(&(self.captured_by_black as u32).to_le_bytes());
        for row in &self.board {
            for cell in row {
                match cell {
                    None => b.push(0xFF),
                    Some(p) => {
                        b.push(match p.side {
                            Side::White => 0,
                            Side::Black => 1,
                        });
                        b.push(match p.kind {
                            Kind::Pawn => 0,
                            Kind::King => 1,
                            Kind::Queen => 2,
                            Kind::Rook => 3,
                            Kind::Bishop => 4,
                            Kind::Knight => 5,
                        });
                        b.push(p.crowned as u8);
                    }
                }
            }
        }
        match self.chain {
            None => b.push(0xFF),
            Some(s) => b.push(pack_sq(s)),
        }
        match self.last_move {
            None => {
                b.push(0xFF);
                b.push(0xFF);
            }
            Some((from, to)) => {
                b.push(pack_sq(from));
                b.push(pack_sq(to));
            }
        }
        match self.pending_promotion {
            None => b.push(0xFF),
            Some(s) => b.push(pack_sq(s)),
        }
        b.push(self.max_chain as u8);
        b.push(self.chess_moves);
        b.push(self.checkers_moves);
        b.extend_from_slice(&self.plies_in_phase.to_le_bytes());
        // Добавлено в 0.21.1: старые состояния короче на один байт.
        b.push(self.kings_only_plies);
        b
    }

    /// Применяет переданное состояние. false — данные повреждены.
    pub fn import_state(&mut self, data: &[u8]) -> bool {
        let Some(st) = parse_game_state(data) else {
            return false;
        };
        self.phase = st.phase;
        self.turn = st.turn;
        self.board = st.board;
        self.twilight_by_moves = st.twilight_by_moves;
        self.auto_phase = st.auto_phase;
        self.mandatory_capture = st.mandatory_capture;
        self.crown_non_pawns = st.crown_non_pawns;
        self.promotion_choice = st.promotion_choice;
        self.promotion_free = st.promotion_free;
        self.max_chain = st.max_chain;
        self.chess_moves = st.chess_moves;
        self.checkers_moves = st.checkers_moves;
        self.plies_in_phase = st.plies_in_phase;
        self.kings_only_plies = st.kings_only_plies;
        self.passes = st.passes;
        self.royal_captured = st.royal_captured;
        self.phase_left = st.phase_left;
        self.phase_total = st.phase_total;
        self.chess_len = st.chess_len;
        self.checkers_len = st.checkers_len;
        self.auto_max = st.auto_max;
        self.move_time_limit = st.move_time_limit;
        self.move_left = st.move_left;
        self.move_anim_secs = st.move_anim_secs;
        self.move_twilight_wait = st.move_twilight_wait;
        self.move_twilight_pending = st.move_twilight_pending;
        self.rng_state = st.rng_state;
        self.ply = st.ply;
        self.captured_by_white = st.captured_by_white;
        self.captured_by_black = st.captured_by_black;
        self.chain = st.chain;
        self.last_move = st.last_move;
        self.pending_promotion = st.pending_promotion;
        self.trans = st.trans;
        self.selected = None;
        self.legal.clear();
        self.pending.clear();
        self.anim = 0.0;
        self.move_anim = 1.0;
        self.crowned_flash = None;
        self.last_capture = None;
        self.refresh_status();
        self.push_snapshot();
        true
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
        // Мат важнее любых ничьих: предохранитель не должен «съесть» победу.
        let mate = self.phase == Phase::Chess && moves.is_empty() && self.in_check(self.turn);
        if !mate {
            // Два короля не могут зажать друг друга: в шашках они к тому же
            // стоят на разных диагоналях, а в шахматах микрошагами не загнать
            // в угол. После пустого полухода каждого — ничья.
            if self.kings_only() && self.kings_only_plies >= 2 && !self.king_capture_possible() {
                self.status = Status::Draw {
                    reason: "король против короля".into(),
                };
                self.end_turn();
                return;
            }
            // Одинокий король против короля и одной фигуры: добить нельзя,
            // и через 10 полуходов без взятий объявляем ничью.
            if self.lone_king_endgame() && self.no_capture_plies() >= NO_CAPTURE_DRAW_PLIES {
                self.status = Status::Draw {
                    reason: "нет взятий".into(),
                };
                self.end_turn();
                return;
            }
        }
        match self.phase {
            Phase::Chess => {
                let checked = self.in_check(self.turn);
                if moves.is_empty() {
                    if checked {
                        // Мат, открывшийся сразу после смены фазы, — следствие
                        // сумерек: последний ход был в прежней игре.
                        let reason = if self.twilight_flip {
                            "мат после сумерек"
                        } else {
                            "мат"
                        };
                        self.status = Status::Win {
                            side: self.turn.other(),
                            reason: reason.into(),
                        };
                    } else {
                        // Пат — не ничья: сумерки переводят бой в шашки.
                        self.status = Status::Playing;
                        self.start_stalemate_twilight();
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
                        if v.side != p.side && v.kind != Kind::King {
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
                            Some(v) if v.side != p.side && v.kind != Kind::King => {
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
                            Some(v) if v.side != p.side && v.kind != Kind::King => {
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
                                if v.side != p.side && v.kind != Kind::King {
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
        // Король в шашках — сильная фигура: бьёт как дамка.
        if p.is_damka() || p.kind == Kind::King {
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
                // Короля дамка (и король) берёт наступлением на его клетку:
                // на краю доски за королём нет поля приземления, и без этого
                // правила край был бы для него неприступной крепостью.
                if victim.kind == Kind::King {
                    push_move(&mut out, sq, (x, y), Some((x, y)));
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

    /// Самая длинная серия взятий, доступная стороне, которая ходит.
    fn longest_run_from_any(&self) -> usize {
        let mut best = 0;
        for r in 0..8 {
            for f in 0..8 {
                if let Some(p) = self.board[r][f] {
                    if p.side == self.turn && p.kind != Kind::King {
                        let run = max_capture_run(&self.board, (f, r), 0);
                        if run > best {
                            best = run;
                        }
                    }
                }
            }
        }
        best
    }

    /// Оставить только продолжения, ведущие к самой длинной серии из `from`.
    fn only_longest(&self, caps: Vec<Move>, from: Square) -> Vec<Move> {
        let need = max_capture_run(&self.board, from, 0);
        if need <= 1 {
            return caps;
        }
        caps.into_iter()
            .filter(|m| 1 + run_after(&self.board, *m) == need)
            .collect()
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
        // Король в шашках ходит как дамка — у него есть мобильность и защита.
        if p.is_damka() || p.kind == Kind::King {
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

fn piece_attacks(b: &Board, from: Square, p: Piece, target: Square) -> bool {
    if from == target {
        return false;
    }
    let (f, r) = (from.0 as i32, from.1 as i32);
    let (tf, tr) = (target.0 as i32, target.1 as i32);
    let dx = tf - f;
    let dy = tr - r;
    match p.kind {
        Kind::Pawn => {
            dy == p.side.forward() && dx.abs() == 1
        }
        Kind::Knight => KNIGHT.contains(&(dx, dy)),
        Kind::King => dx.abs() <= 1 && dy.abs() <= 1,
        Kind::Bishop | Kind::Rook | Kind::Queen => {
            let diag = dx.abs() == dy.abs() && dx != 0;
            let ortho = (dx == 0) != (dy == 0);
            let ok = match p.kind {
                Kind::Bishop => diag,
                Kind::Rook => ortho,
                _ => diag || ortho,
            };
            if !ok {
                return false;
            }
            let sx = dx.signum();
            let sy = dy.signum();
            let mut x = f + sx;
            let mut y = r + sy;
            while (x, y) != (tf, tr) {
                if b[y as usize][x as usize].is_some() {
                    return false;
                }
                x += sx;
                y += sy;
            }
            true
        }
    }
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

/// Клетка на доске (любая очередь, без проверок).
/// Может ли фигура `p` с клетки `from` взять фигуру на `target` (шашки).
/// Дамка и король бьют с любого расстояния по свободной диагонали,
/// простая шашка — только через соседнюю клетку.
pub(crate) fn can_capture_at(
    b: &Board,
    from: (i32, i32),
    p: Piece,
    target: Square,
) -> bool {
    let (tf, tr) = (target.0 as i32, target.1 as i32);
    let (df, dr) = (tf - from.0, tr - from.1);
    if df == 0 || df.abs() != dr.abs() {
        return false;
    }
    let (sx, sy) = (df.signum(), dr.signum());
    let flyer = p.is_damka() || p.kind == Kind::King;
    // Дамка и король бьют короля и наступлением на его клетку: на краю
    // доски поля приземления за ним нет, а взять его всё равно нужно.
    let step_on_king = flyer
        && at_b(b, tf, tr)
            .map(|v| v.side != p.side && v.kind == Kind::King)
            .unwrap_or(false);
    // Поле приземления — сразу за целью, оно должно быть свободным.
    let (lx, ly) = (tf + sx, tr + sy);
    if !step_on_king && (!inb(lx, ly) || b[ly as usize][lx as usize].is_some()) {
        return false;
    }
    if flyer {
        let mut x = from.0 + sx;
        let mut y = from.1 + sy;
        while (x, y) != (tf, tr) {
            if !inb(x, y) || b[y as usize][x as usize].is_some() {
                return false;
            }
            x += sx;
            y += sy;
        }
        true
    } else {
        // Простая шашка бьёт только через соседнюю клетку: цель рядом,
        // поле приземления за ней уже проверено.
        df.abs() == 1
    }
}

fn inb(f: i32, r: i32) -> bool {
    (0..8).contains(&f) && (0..8).contains(&r)
}

fn board_at(b: &Board, sq: Square) -> Option<Piece> {
    at_b(b, sq.0 as i32, sq.1 as i32)
}

fn empty_b(b: &Board, f: i32, r: i32) -> bool {
    (0..8).contains(&f) && (0..8).contains(&r) && b[r as usize][f as usize].is_none()
}

fn inb_b(f: i32, r: i32) -> bool {
    (0..8).contains(&f) && (0..8).contains(&r)
}

/// Взятия фигуры с клетки по доске: всё, что может взять эта фигура.
fn captures_on(b: &Board, sq: Square) -> Vec<Move> {
    let Some(p) = board_at(b, sq) else {
        return Vec::new();
    };
    let (f, r) = (sq.0 as i32, sq.1 as i32);
    let mut out = Vec::new();
    if p.is_damka() || p.kind == Kind::King {
        for (dx, dy) in DIAG {
            let (mut x, mut y) = (f + dx, r + dy);
            while inb_b(x, y) && empty_b(b, x, y) {
                x += dx;
                y += dy;
            }
            if !inb_b(x, y) {
                continue;
            }
            let Some(v) = at_b(b, x, y) else { continue };
            if v.side == p.side {
                continue;
            }
            // Король берётся и наступлением на его клетку — см. checker_captures_from.
            if v.kind == Kind::King {
                push_move(&mut out, sq, (x, y), Some((x, y)));
            }
            let (mut lx, mut ly) = (x + dx, y + dy);
            while inb_b(lx, ly) && empty_b(b, lx, ly) {
                push_move(&mut out, sq, (lx, ly), Some((x, y)));
                lx += dx;
                ly += dy;
            }
        }
    } else {
        for (dx, dy) in DIAG {
            let (vx, vy) = (f + dx, r + dy);
            let (lx, ly) = (f + 2 * dx, r + 2 * dy);
            if !inb_b(lx, ly) {
                continue;
            }
            if let Some(v) = at_b(b, vx, vy) {
                if v.side != p.side && empty_b(b, lx, ly) {
                    push_move(&mut out, sq, (lx, ly), Some((vx, vy)));
                }
            }
        }
    }
    out
}

/// Доска после взятия этим ходом (с коронацией на последнем ряду).
fn board_after_capture(b: &Board, mv: Move) -> Board {
    let mut out = *b;
    let Some(mut p) = out[mv.from.1][mv.from.0].take() else {
        return out;
    };
    if let Some(cap) = mv.captured {
        out[cap.1][cap.0] = None;
    }
    let last = if p.side == Side::White { 7 } else { 0 };
    if mv.to.1 as usize == last {
        p.crowned = true;
    }
    out[mv.to.1][mv.to.0] = Some(p);
    out
}

/// Сколько ещё взятий даёт ход (без учёта самого этого взятия).
fn run_after(b: &Board, mv: Move) -> usize {
    let after = board_after_capture(b, mv);
    max_capture_run(&after, mv.to, 0)
}

/// Максимальная длина серии взятий из клетки: число взятых фигур.
fn max_capture_run(b: &Board, sq: Square, depth: u8) -> usize {
    if depth > 12 {
        return 0;
    }
    let caps = captures_on(b, sq);
    let mut best = 0;
    for mv in caps {
        let run = 1 + run_after(b, mv);
        if run > best {
            best = run;
        }
    }
    best
}

fn at_b(b: &Board, f: i32, r: i32) -> Option<Piece> {
    if (0..8).contains(&f) && (0..8).contains(&r) {
        b[r as usize][f as usize]
    } else {
        None
    }
}

/// Клетка в один байт: файл × 8 + ряд; 0xFF — пусто.
fn pack_sq(sq: Square) -> u8 {
    (sq.0 as u8) * 8 + sq.1 as u8
}

fn unpack_sq(b: u8) -> Option<Square> {
    if b == 0xFF {
        return None;
    }
    let f = (b / 8) as usize;
    let r = (b % 8) as usize;
    if f < 8 && r < 8 {
        Some((f, r))
    } else {
        None
    }
}

/// Разобранное состояние партии (см. `Game::export_state`).
struct GameStateData {
    phase: Phase,
    turn: Side,
    twilight_by_moves: bool,
    auto_phase: bool,
    mandatory_capture: bool,
    crown_non_pawns: bool,
    promotion_choice: bool,
    promotion_free: bool,
    passes: u8,
    royal_captured: Option<Side>,
    phase_left: f32,
    phase_total: f32,
    chess_len: f32,
    checkers_len: f32,
    auto_max: f32,
    move_time_limit: f32,
    move_left: f32,
    move_anim_secs: f32,
    move_twilight_wait: f32,
    move_twilight_pending: bool,
    trans: f32,
    rng_state: u64,
    ply: u32,
    captured_by_white: u32,
    captured_by_black: u32,
    board: Board,
    chain: Option<Square>,
    last_move: Option<(Square, Square)>,
    pending_promotion: Option<Square>,
    max_chain: bool,
    chess_moves: u8,
    checkers_moves: u8,
    plies_in_phase: u16,
    kings_only_plies: u8,
}

fn rd_u8(d: &[u8], i: &mut usize) -> Option<u8> {
    let v = *d.get(*i)?;
    *i += 1;
    Some(v)
}

fn rd_f32(d: &[u8], i: &mut usize) -> Option<f32> {
    if *i + 4 > d.len() {
        return None;
    }
    let mut b = [0u8; 4];
    b.copy_from_slice(&d[*i..*i + 4]);
    *i += 4;
    Some(f32::from_le_bytes(b))
}

fn rd_u32(d: &[u8], i: &mut usize) -> Option<u32> {
    if *i + 4 > d.len() {
        return None;
    }
    let mut b = [0u8; 4];
    b.copy_from_slice(&d[*i..*i + 4]);
    *i += 4;
    Some(u32::from_le_bytes(b))
}

fn rd_u64(d: &[u8], i: &mut usize) -> Option<u64> {
    if *i + 8 > d.len() {
        return None;
    }
    let mut b = [0u8; 8];
    b.copy_from_slice(&d[*i..*i + 8]);
    *i += 8;
    Some(u64::from_le_bytes(b))
}

fn parse_game_state(d: &[u8]) -> Option<GameStateData> {
    if d.len() < 5 || &d[0..4] != b"SMST" || d[4] != 1 {
        return None;
    }
    let mut i = 5usize;
    let phase = match rd_u8(d, &mut i)? {
        0 => Phase::Chess,
        1 => Phase::Checkers,
        _ => return None,
    };
    let turn = match rd_u8(d, &mut i)? {
        0 => Side::White,
        1 => Side::Black,
        _ => return None,
    };
    let flags = [
        rd_u8(d, &mut i)?,
        rd_u8(d, &mut i)?,
        rd_u8(d, &mut i)?,
        rd_u8(d, &mut i)?,
        rd_u8(d, &mut i)?,
        rd_u8(d, &mut i)?,
    ];
    let passes = rd_u8(d, &mut i)?;
    let royal = rd_u8(d, &mut i)?;
    let royal_captured = match royal {
        0xFF => None,
        0 => Some(Side::White),
        1 => Some(Side::Black),
        _ => return None,
    };
    let phase_left = rd_f32(d, &mut i)?;
    let phase_total = rd_f32(d, &mut i)?;
    let chess_len = rd_f32(d, &mut i)?;
    let checkers_len = rd_f32(d, &mut i)?;
    let auto_max = rd_f32(d, &mut i)?;
    let move_time_limit = rd_f32(d, &mut i)?;
    let move_left = rd_f32(d, &mut i)?;
    let move_anim_secs = rd_f32(d, &mut i)?;
    let move_twilight_wait = rd_f32(d, &mut i)?;
    let move_twilight_pending = rd_u8(d, &mut i)? != 0;
    let trans = rd_f32(d, &mut i)?;
    let rng_state = rd_u64(d, &mut i)?;
    let ply = rd_u32(d, &mut i)?;
    let captured_by_white = rd_u32(d, &mut i)?;
    let captured_by_black = rd_u32(d, &mut i)?;
    let mut board: Board = [[None; 8]; 8];
    for r in 0..8 {
        for f in 0..8 {
            let side_b = rd_u8(d, &mut i)?;
            if side_b == 0xFF {
                continue;
            }
            let side = match side_b {
                0 => Side::White,
                1 => Side::Black,
                _ => return None,
            };
            let kind = match rd_u8(d, &mut i)? {
                0 => Kind::Pawn,
                1 => Kind::King,
                2 => Kind::Queen,
                3 => Kind::Rook,
                4 => Kind::Bishop,
                5 => Kind::Knight,
                _ => return None,
            };
            let crowned = rd_u8(d, &mut i)? != 0;
            board[r][f] = Some(Piece { side, kind, crowned });
        }
    }
    let opt_sq = |d: &[u8], i: &mut usize| -> Option<Option<Square>> {
        let b = rd_u8(d, i)?;
        if b == 0xFF {
            return Some(None);
        }
        Some(unpack_sq(b))
    };
    let chain = opt_sq(d, &mut i)?;
    let last_move = match (opt_sq(d, &mut i)?, opt_sq(d, &mut i)?) {
        (None, None) => None,
        (Some(from), Some(to)) => Some((from, to)),
        _ => return None,
    };
    let pending_promotion = opt_sq(d, &mut i)?;
    let max_chain = *d.get(i)? != 0;
    let chess_moves = (*d.get(i + 1)?).clamp(1, 8);
    let checkers_moves = (*d.get(i + 2)?).clamp(1, 8);
    let plies_in_phase = u16::from_le_bytes([*d.get(i + 3)?, *d.get(i + 4)?]);
    // Байт добавлен в 0.21.1; обрезанные данные по-прежнему отвергаются.
    let kings_only_plies = *d.get(i + 5)?;
    Some(GameStateData {
        phase,
        turn,
        twilight_by_moves: flags[0] != 0,
        auto_phase: flags[1] != 0,
        mandatory_capture: flags[2] != 0,
        crown_non_pawns: flags[3] != 0,
        promotion_choice: flags[4] != 0,
        promotion_free: flags[5] != 0,
        passes,
        royal_captured,
        phase_left,
        phase_total,
        chess_len,
        checkers_len,
        auto_max,
        move_time_limit,
        move_left,
        move_anim_secs,
        move_twilight_wait,
        move_twilight_pending,
        trans,
        rng_state,
        ply,
        captured_by_white,
        captured_by_black,
        board,
        chain,
        last_move,
        pending_promotion,
        max_chain,
        chess_moves,
        checkers_moves,
        plies_in_phase,
        kings_only_plies,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_game() -> Game {
        Game::new(25.0)
    }

    /// Сумерки не сменяют фазу, пока в шашках можно съесть короля.
    #[test]
    fn twilight_waits_while_king_can_be_captured() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[0][0] = Some(Piece::new(Side::White, Kind::King)); // a1
        g.board[3][3] = Some(Piece::new(Side::Black, Kind::King)); // d4
        g.board[2][2] = Some(Piece::new(Side::White, Kind::Pawn)); // c3 — бьёт короля
        g.turn = Side::White;
        g.twilight_by_moves = true;
        g.set_phase_len(Phase::Chess, 6);
        g.set_phase_len(Phase::Checkers, 6);
        g.move_twilight_pending = true;
        g.move_twilight_wait = 0.0;
        g.move_anim = 1.0;
        g.tick(0.01);
        assert_eq!(g.trans, 0.0, "сумерки ждут: короля можно съесть");
        assert_eq!(g.phase, Phase::Checkers, "фаза не сменилась");
    }

    /// Сумерки не сменяют фазу, пока в шахматах не отыгран шах.
    #[test]
    fn twilight_waits_while_in_check() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Chess;
        g.board[0][4] = Some(Piece::new(Side::White, Kind::King)); // e1
        g.board[7][7] = Some(Piece::new(Side::Black, Kind::King)); // h8
        g.board[7][4] = Some(Piece::new(Side::Black, Kind::Rook)); // e8 — шах
        g.turn = Side::White;
        g.twilight_by_moves = true;
        g.move_twilight_pending = true;
        g.move_twilight_wait = 0.0;
        g.move_anim = 1.0;
        g.tick(0.01);
        assert_eq!(g.trans, 0.0, "шах надо отыграть");
        assert_eq!(g.phase, Phase::Chess, "фаза не сменилась");
    }

    /// Короли вплотную: сумерки ждут, иначе в шахматах получился бы «мат»
    /// из невозможной позиции (так закончилась одна из демо-партий).
    #[test]
    fn twilight_waits_while_kings_are_adjacent() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[0][4] = Some(Piece::new(Side::White, Kind::King)); // e1
        g.board[1][4] = Some(Piece::new(Side::Black, Kind::King)); // e2 — вплотную
        g.board[7][0] = Some(Piece::new(Side::Black, Kind::Pawn)); // a8
        g.turn = Side::White;
        g.twilight_by_moves = true;
        g.move_twilight_pending = true;
        g.move_twilight_wait = 0.0;
        g.move_anim = 1.0;
        g.tick(0.01);
        assert_eq!(g.trans, 0.0, "короли рядом — сумерки ждут");
        assert_eq!(g.phase, Phase::Checkers, "фаза не сменилась");
    }

    /// Удержание сумерек ограничено: после двух отложек фаза меняется
    /// принудительно, а короли вплотную — тот, чей ход, забирает короля.
    #[test]
    fn twilight_forces_flip_after_two_holds() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[0][4] = Some(Piece::new(Side::White, Kind::King)); // e1
        g.board[1][4] = Some(Piece::new(Side::Black, Kind::King)); // e2 — вплотную
        g.board[0][0] = Some(Piece::new(Side::White, Kind::Pawn)); // a1
        g.board[7][0] = Some(Piece::new(Side::Black, Kind::Pawn)); // a8
        g.turn = Side::White;
        g.twilight_by_moves = true;
        g.set_phase_len(Phase::Chess, 6);
        g.set_phase_len(Phase::Checkers, 6);
        let mut w = (4, 0);
        let mut b = (0, 7);
        for _ in 0..12 {
            if g.is_over() {
                break;
            }
            if g.turn == Side::White {
                let to = if w == (4, 0) { (3, 1) } else { (4, 0) };
                g.apply(Move {
                    from: w,
                    to,
                    captured: None,
                });
                w = to;
            } else {
                let to = (b.0 + 1, b.1 - 1);
                g.apply(Move {
                    from: b,
                    to,
                    captured: None,
                });
                b = to;
            }
            // Смена фаз «наступила бы сейчас».
            g.move_anim = 1.0;
            g.move_twilight_pending = true;
            g.move_twilight_wait = 0.0;
            g.tick(0.1);
            for _ in 0..30 {
                if g.trans == 0.0 {
                    break;
                }
                g.tick(0.1);
            }
        }
        assert!(g.is_over(), "фаза не сменилась принудительно: {:?}", g.phase);
        assert!(
            matches!(g.status, Status::Win { .. }),
            "короли вплотную — король съеден: {:?}",
            g.status
        );
    }

    /// Мат, открывшийся сразу после смены фазы, помечается отдельно:
    /// последний ход был в прежней игре, и «мат» без пояснения сбивает.
    #[test]
    fn mate_right_after_twilight_is_marked() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[7][0] = Some(Piece::new(Side::Black, Kind::King)); // a8
        g.board[0][0] = Some(Piece::new(Side::White, Kind::Rook)); // a1 — шах по вертикали
        g.board[6][2] = Some(Piece::new(Side::White, Kind::King)); // c7 — держит b7/b8
        g.turn = Side::White;

        g.flip_phase();
        match &g.status {
            Status::Win { side, reason } => {
                assert_eq!(*side, Side::White);
                assert_eq!(reason, "мат после сумерек");
            }
            other => panic!("ожидали мат, получили {:?}", other),
        }
    }

    /// Мат, найденный обычным ходом, остаётся просто «мат».
    #[test]
    fn mate_after_regular_move_is_plain() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Chess;
        g.board[7][0] = Some(Piece::new(Side::Black, Kind::King)); // a8
        g.board[0][4] = Some(Piece::new(Side::White, Kind::King)); // e1
        g.board[6][6] = Some(Piece::new(Side::White, Kind::Queen)); // g7
        g.board[5][1] = Some(Piece::new(Side::White, Kind::Queen)); // b6
        g.turn = Side::White;

        // Ферзь g7 идёт на b7 — мат, но это следствие хода, а не сумерек.
        g.apply(Move {
            from: (6, 6),
            to: (1, 6),
            captured: None,
        });
        match &g.status {
            Status::Win { side, reason } => {
                assert_eq!(*side, Side::White);
                assert_eq!(reason, "мат");
            }
            other => panic!("ожидали мат, получили {:?}", other),
        }
    }

    /// В спокойной позиции сумерки начинаются как обычно.
    #[test]
    fn twilight_flips_when_position_is_quiet() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Chess;
        g.board[0][4] = Some(Piece::new(Side::White, Kind::King)); // e1
        g.board[7][4] = Some(Piece::new(Side::Black, Kind::King)); // e8
        g.turn = Side::White;
        g.twilight_by_moves = true;
        g.move_twilight_pending = true;
        g.move_twilight_wait = 0.0;
        g.move_anim = 1.0;
        g.tick(0.01);
        assert!(g.trans > 0.0, "спокойная позиция — сумерки пошли");
    }

    /// Мёртвый эндшпиль: одинокий король против короля и одной сильной
    /// фигуры — ничья через 10 полуходов без взятий.
    #[test]
    fn draw_in_lone_king_endgame_after_no_captures() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Chess;
        g.board[0][0] = Some(Piece::new(Side::White, Kind::King)); // a1
        g.board[0][7] = Some(Piece::new(Side::White, Kind::Queen)); // h1
        g.board[7][7] = Some(Piece::new(Side::Black, Kind::King)); // h8
        g.turn = Side::White;
        let mut sq_w = (0, 0);
        let mut sq_b = (7, 7);
        for _ in 0..NO_CAPTURE_DRAW_PLIES {
            let (from, to) = if g.turn == Side::White {
                let to = if sq_w == (0, 0) { (0, 1) } else { (0, 0) };
                let m = (sq_w, to);
                sq_w = to;
                m
            } else {
                let to = if sq_b == (7, 7) { (7, 6) } else { (7, 7) };
                let m = (sq_b, to);
                sq_b = to;
                m
            };
            g.apply(Move {
                from,
                to,
                captured: None,
            });
        }
        assert!(
            matches!(g.status, Status::Draw { .. }),
            "одинокий король против фигуры: {:?}",
            g.status
        );
    }

    /// Две сильные фигуры у соперника: правило не применяется.
    #[test]
    fn lone_king_with_two_pieces_is_not_auto_draw() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Chess;
        g.board[0][0] = Some(Piece::new(Side::White, Kind::King)); // a1
        g.board[0][7] = Some(Piece::new(Side::White, Kind::Queen)); // h1
        g.board[0][6] = Some(Piece::new(Side::White, Kind::Rook)); // g1
        g.board[7][7] = Some(Piece::new(Side::Black, Kind::King)); // h8
        g.turn = Side::White;
        let mut sq_w = (0, 0);
        let mut sq_b = (7, 7);
        for _ in 0..NO_CAPTURE_DRAW_PLIES {
            let (from, to) = if g.turn == Side::White {
                let to = if sq_w == (0, 0) { (0, 1) } else { (0, 0) };
                let m = (sq_w, to);
                sq_w = to;
                m
            } else {
                let to = if sq_b == (7, 7) { (7, 6) } else { (7, 7) };
                let m = (sq_b, to);
                sq_b = to;
                m
            };
            g.apply(Move {
                from,
                to,
                captured: None,
            });
        }
        assert!(
            !matches!(g.status, Status::Draw { .. }),
            "две сильные фигуры — не мёртвая позиция: {:?}",
            g.status
        );
    }

    /// Король под боем может уйти вместо обязательного взятия.
    #[test]
    fn king_escape_lets_king_leave_forced_capture() {
        let pos = || {
            let mut g = test_game();
            g.board = [[None; 8]; 8];
            g.phase = Phase::Checkers;
            g.board[1][1] = Some(Piece::new(Side::White, Kind::King)); // b2
            g.board[2][0] = Some(Piece::new(Side::Black, Kind::Pawn)); // a3
            g.board[3][3] = Some(Piece::new(Side::White, Kind::Pawn)); // d4
            g.board[4][2] = Some(Piece::new(Side::Black, Kind::Pawn)); // c5
            g.turn = Side::White;
            g.mandatory_capture = true;
            g.max_chain = true;
            g
        };
        let mut on = pos();
        on.king_escape = true;
        let moves_on = on.all_moves();
        assert!(
            moves_on
                .iter()
                .any(|m| m.from == (1, 1) && m.captured.is_none()),
            "король уходит из-под боя: {:?}",
            moves_on
        );
        let mut off = pos();
        off.king_escape = false;
        let moves_off = off.all_moves();
        assert!(
            !moves_off
                .iter()
                .any(|m| m.from == (1, 1) && m.captured.is_none()),
            "без правила король не ходит: {:?}",
            moves_off
        );
        assert!(
            moves_off.iter().any(|m| m.captured.is_some()),
            "взятие шашкой остаётся обязательным"
        );
    }

    #[test]
    fn kings_only_draw_after_two_empty_plies() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Chess;
        g.board[0][0] = Some(Piece::new(Side::White, Kind::King));
        g.board[7][7] = Some(Piece::new(Side::Black, Kind::King));
        g.turn = Side::White;
        g.apply(Move {
            from: (0, 0),
            to: (0, 1),
            captured: None,
        });
        assert!(
            matches!(g.status, Status::Playing),
            "после одного пустого полухода ничьей ещё нет: {:?}",
            g.status
        );
        g.apply(Move {
            from: (7, 7),
            to: (7, 6),
            captured: None,
        });
        assert!(
            matches!(g.status, Status::Draw { .. }),
            "оба короля сходили пусто — ничья: {:?}",
            g.status
        );
        assert!(g.is_over(), "ничья завершает партию");
    }

    /// Шашка обязана бить — но король всё равно может срубить чужого
    /// короля. Раньше в такой позиции король считался «приколотым»
    /// и его взятия не попадали в список ходов.
    #[test]
    fn king_can_capture_royal_while_checker_must_capture() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[4][2] = Some(Piece::new(Side::Black, Kind::King)); // c5
        g.board[2][4] = Some(Piece::new(Side::White, Kind::King)); // e3
        g.board[6][6] = Some(Piece::new(Side::Black, Kind::Pawn)); // g7
        g.board[5][5] = Some(Piece::new(Side::White, Kind::Pawn)); // f6
        g.turn = Side::Black;
        g.mandatory_capture = true;
        let moves = g.all_moves();
        assert!(
            moves.iter().any(|m| m.captured == Some((4, 2))),
            "король бьёт чужого короля: {:?}",
            moves
        );
        assert!(
            moves.iter().any(|m| m.captured == Some((5, 5))),
            "шашка обязана бить: {:?}",
            moves
        );
    }

    #[test]
    fn kings_only_draw_skipped_when_king_can_be_captured() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        // Белый король на (2,2) бьёт чёрного на (1,1): это победа, а не тупик.
        g.board[2][2] = Some(Piece::new(Side::White, Kind::King));
        g.board[1][1] = Some(Piece::new(Side::Black, Kind::King));
        g.turn = Side::White;
        g.kings_only_plies = 5;
        g.refresh_status();
        assert!(
            !matches!(g.status, Status::Draw { .. }),
            "взятие короля возможно — ничьей нет: {:?}",
            g.status
        );
    }

    #[test]
    fn draw_counter_roundtrips_in_state() {
        let mut g = test_game();
        g.kings_only_plies = 2;
        let data = g.export_state();
        let mut back = test_game();
        assert!(back.import_state(&data));
        assert_eq!(back.kings_only_plies, 2, "счётчик ничьей переносится");
    }

    #[test]
    fn phase_switches_after_full_moves() {
        let mut g = test_game();
        g.set_twilight_by_moves(true);
        g.set_phase_len(Phase::Chess, 4);
        g.set_phase_len(Phase::Checkers, 4);
        let phase = g.phase;
        // Четыре полных хода = восемь полуходов: только тогда фаза меняется.
        for _ in 0..7 {
            let mv = g.all_moves()[0];
            g.apply(mv);
        }
        assert!(!g.move_twilight_pending, "на семи полуходах смены нет");
        let mv = g.all_moves()[0];
        g.apply(mv);
        assert_eq!(g.plies_in_phase, 0, "счётчик сбрасывается");
        assert!(g.move_twilight_pending, "после четырёх полных ходов — смена");
        assert_eq!(g.phase, phase, "фаза меняется после перехода сумерек");
    }

    #[test]
    fn full_moves_to_switch_counts_down() {
        let mut g = test_game();
        g.set_twilight_by_moves(true);
        g.set_phase_len(Phase::Chess, 6);
        g.set_phase_len(Phase::Checkers, 6);
        assert_eq!(g.full_moves_to_switch(), 6);
        let mv = g.all_moves()[0];
        g.apply(mv);
        assert_eq!(g.full_moves_to_switch(), 6, "после своего хода ещё шесть");
        let mv = g.all_moves()[0];
        g.apply(mv);
        assert_eq!(g.full_moves_to_switch(), 5, "после полного хода — пять");
    }

    #[test]
    fn king_moves_like_damka_in_checkers() {
        let mut g = test_game();
        g.phase = Phase::Checkers;
        g.board = [[None; 8]; 8];
        g.board[0][0] = Some(Piece::new(Side::White, Kind::King));
        g.turn = Side::White;
        let moves = g.moves_for((0, 0));
        assert!(
            moves.iter().any(|m| m.to == (3, 3)),
            "король в шашках ходит как дамка по диагонали"
        );
    }

    #[test]
    fn king_is_not_forced_to_capture() {
        let mut g = test_game();
        g.phase = Phase::Checkers;
        g.board = [[None; 8]; 8];
        g.board[2][2] = Some(Piece::new(Side::White, Kind::King));
        g.board[3][1] = Some(Piece::new(Side::Black, Kind::Pawn));
        g.turn = Side::White;
        g.mandatory_capture = true;
        let moves = g.moves_for((2, 2));
        assert!(
            moves.iter().any(|m| m.captured.is_none()),
            "король не обязан бить: обычный ход доступен"
        );
        assert!(
            moves.iter().any(|m| m.captured.is_some()),
            "король может бить по желанию"
        );
        // Обычная шашка в той же позиции обязана бить.
        g.board[2][2] = Some(Piece::new(Side::White, Kind::Pawn));
        let pawn_moves = g.moves_for((2, 2));
        assert!(!pawn_moves.is_empty());
        assert!(
            pawn_moves.iter().all(|m| m.captured.is_some()),
            "шашка обязана брать"
        );
    }

    #[test]
    fn max_chain_requires_longest_series() {
        let mut g = test_game();
        g.phase = Phase::Checkers;
        g.board = [[None; 8]; 8];
        g.board[2][2] = Some(Piece::new(Side::White, Kind::Pawn));
        g.board[3][1] = Some(Piece::new(Side::Black, Kind::Pawn));
        g.board[3][3] = Some(Piece::new(Side::Black, Kind::Pawn));
        g.board[5][1] = Some(Piece::new(Side::Black, Kind::Pawn));
        g.turn = Side::White;
        g.mandatory_capture = true;
        g.max_chain = false;
        let any = g.all_moves();
        g.max_chain = true;
        let longest = g.all_moves();
        assert!(any.len() >= 2, "без ограничения доступны оба взятия");
        assert_eq!(longest.len(), 1, "с максимумом остаётся только длинная серия");
        assert_eq!(longest[0].from, (2, 2));
    }

    #[test]
    fn state_export_import_preserves_hash() {
        let mut g = test_game();
        let mv = g.all_moves()[0];
        g.apply(mv);
        let mv2 = g.all_moves()[0];
        g.apply(mv2);
        g.phase_left = 13.5;
        g.move_left = 33.3;
        g.seed_rng(0x1234_5678);
        g.crown_non_pawns = true;
        g.mandatory_capture = false;
        let data = g.export_state();
        let mut back = test_game();
        assert!(back.import_state(&data));
        assert_eq!(back.turn, g.turn);
        assert_eq!(back.phase, g.phase);
        assert_eq!(back.ply, g.ply);
        assert_eq!(back.rng_state, 0x1234_5678);
        assert!((back.phase_left - 13.5).abs() < 0.001);
        assert!((back.move_left - 33.3).abs() < 0.001);
        assert!(back.crown_non_pawns);
        assert!(!back.mandatory_capture);
        assert_eq!(
            crate::net::position_hash(&back),
            crate::net::position_hash(&g),
            "хеши позиций совпадают после переноса"
        );
        // Повреждённые данные отклоняются.
        assert!(!back.import_state(&data[..data.len() - 1]));
        assert!(!back.import_state(b"XXXX"));
    }

    #[test]
    fn initial_chess_moves() {
        let g = test_game();
        assert_eq!(g.all_moves().len(), 20, "у белых 20 ходов в начале");
        assert!(g.king_sq(Side::White).is_some());
        assert!(g.king_sq(Side::Black).is_some());
    }

    #[test]
    fn initial_position_has_16_pieces_each() {
        let g = test_game();
        assert_eq!(g.count(Side::White), 16, "у белых 16 фигур");
        assert_eq!(g.count(Side::Black), 16, "у чёрных 16 фигур");
    }

    #[test]
    fn checkers_start_without_damki() {
        let g = test_game();
        assert!(
            g.board.iter().flatten().flatten().all(|p| !p.is_damka()),
            "в начале шашечной фазы дамок нет"
        );
    }

    #[test]
    fn checker_crowns_on_last_rank() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[6][0] = Some(Piece::new(Side::White, Kind::Pawn)); // a7
        g.board[7][7] = Some(Piece::new(Side::Black, Kind::King)); // h8
        g.turn = Side::White;
        let mv = Move {
            from: (0, 6),
            to: (1, 7),
            captured: None,
        };
        g.apply(mv);
        let p = g.piece_at((1, 7)).unwrap();
        assert!(p.crowned, "простая шашка на последнем ряду стала дамкой");
        assert!(p.is_damka());
    }

    #[test]
    fn black_checker_crowns_on_last_rank() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[1][1] = Some(Piece::new(Side::Black, Kind::Pawn)); // b2
        g.turn = Side::Black;
        let mv = Move {
            from: (1, 1),
            to: (0, 0),
            captured: None,
        };
        g.apply(mv);
        let p = g.piece_at((0, 0)).unwrap();
        assert!(p.crowned, "чёрная шашка на последнем ряду стала дамкой");
        assert!(p.is_damka());
    }

    #[test]
    fn checker_crowns_and_continues_as_damka() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[5][1] = Some(Piece::new(Side::White, Kind::Pawn)); // b6
        g.board[6][2] = Some(Piece::new(Side::Black, Kind::Pawn)); // c7
        g.board[5][5] = Some(Piece::new(Side::Black, Kind::Pawn)); // f6
        g.turn = Side::White;
        let mv = Move {
            from: (1, 5),
            to: (3, 7),
            captured: Some((2, 6)),
        };
        g.apply(mv);
        let p = g.piece_at((3, 7)).unwrap();
        assert!(p.crowned, "шашка стала дамкой на последнем ряду");
        assert_eq!(g.chain, Some((3, 7)), "серия взятий продолжается");
        assert!(
            g.legal.iter().any(|m| m.to == (6, 4)),
            "дамка продолжает серию дальним взятием f6"
        );
    }

    #[test]
    fn move_history_records_piece_and_coords() {
        let mut g = test_game();
        g.apply(Move {
            from: (4, 1),
            to: (4, 3),
            captured: None,
        });
        assert_eq!(g.history.len(), 1);
        let m = &g.history[0];
        assert_eq!(m.side, Side::White);
        assert_eq!(m.kind, Kind::Pawn);
        assert_eq!(m.from, (4, 1));
        assert_eq!(m.to, (4, 3));
        assert_eq!(m.phase, Phase::Chess);
        assert!(m.captured.is_none());
        assert_eq!(sq_name(m.from), "e2");
        assert_eq!(sq_name(m.to), "e4");
    }

    #[test]
    fn chess_king_cannot_be_captured() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.board[0][4] = Some(Piece::new(Side::White, Kind::King)); // e1
        g.board[7][7] = Some(Piece::new(Side::Black, Kind::King)); // h8
        g.board[6][5] = Some(Piece::new(Side::White, Kind::Knight)); // f7
        g.turn = Side::White;
        assert!(
            g.chess_from((5, 6)).iter().all(|m| m.to != (7, 7)),
            "в шахматах короля не бьют — только шах/мат"
        );
        g.apply(Move {
            from: (5, 6),
            to: (7, 7),
            captured: Some((7, 7)),
        });
        assert!(!g.is_over(), "король не может быть съеден");
        assert!(g.piece_at((7, 7)).is_some(), "король остался на доске");
    }

    #[test]
    fn transition_gives_checked_king_a_move() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.turn = Side::White;
        g.board[0][0] = Some(Piece::new(Side::White, Kind::King)); // a1
        g.board[7][6] = Some(Piece::new(Side::Black, Kind::King)); // g8
        g.board[7][3] = Some(Piece::new(Side::White, Kind::Pawn)); // d8
        g.flip_phase();
        assert_eq!(g.phase, Phase::Chess);
        assert_eq!(g.turn, Side::Black, "король под шахом получает ход");
        assert!(matches!(g.status, Status::Check(Side::Black)));
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
        let mut damka = Piece::new(Side::White, Kind::Rook);
        damka.crowned = true;
        g.board[0][0] = Some(damka); // a1 — дамка
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
        g.twilight_by_moves = false;
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
    fn phase_lengths_are_independent() {
        let mut g = test_game();
        g.twilight_by_moves = false;
        assert_eq!(g.chess_len, 25.0);
        assert_eq!(g.checkers_len, 25.0);
        g.set_len(Phase::Chess, 10.0);
        g.set_len(Phase::Checkers, 40.0);
        assert_eq!(g.phase_left, 10.0, "текущая шахматная фаза перезапущена");
        g.phase_left = 0.05;
        g.tick(0.05);
        while g.phase == Phase::Chess {
            g.tick(0.1);
        }
        assert!(
            (g.phase_left - 40.0).abs() < 0.2,
            "после перехода включилось своё время шашек: {}",
            g.phase_left
        );
    }

    #[test]
    fn auto_phase_intervals_vary_and_stay_in_range() {
        let mut g = test_game();
        g.auto_max = 60.0;
        g.seed_rng(0x1234_5678_9ABC_DEF1);
        g.set_auto(true);
        let mut prev = g.phase_left;
        assert!((MIN_AUTO_SECS..=MAX_PHASE).contains(&prev), "{prev}");
        for _ in 0..12 {
            g.flip_phase();
            let next = g.phase_left;
            assert!((MIN_AUTO_SECS..=MAX_PHASE).contains(&next), "{next}");
            assert!(
                (next - prev).abs() >= 1.0 || g.auto_max <= MIN_AUTO_SECS,
                "соседние интервалы разные: {prev} и {next}"
            );
            prev = next;
        }
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
    fn stalemate_starts_twilight() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.board[7][0] = Some(Piece::new(Side::Black, Kind::King)); // a8
        g.board[5][2] = Some(Piece::new(Side::White, Kind::King)); // c6
        g.board[5][1] = Some(Piece::new(Side::White, Kind::Queen)); // b6
        g.turn = Side::Black;
        g.refresh_status();
        assert!(matches!(g.status, Status::Playing), "пат не ничья: {:?}", g.status);
        assert!(g.trans > 0.0, "пат запускает сумерки");
        assert_eq!(g.winner(), None);
    }

    #[test]
    fn pawn_promotes_to_queen() {
        let mut g = test_game();
        g.promotion_choice = true;
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
    fn pawn_promotes_to_queen_by_default() {
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
        assert_eq!(g.pending_promotion, None, "по умолчанию без диалога");
        assert_eq!(g.piece_at((0, 7)).unwrap().kind, Kind::Queen);
        assert_eq!(g.turn, Side::Black);
        assert!(g.log.last().unwrap().contains("=Ф"));
    }

    #[test]
    fn pawn_can_promote_to_any_piece() {
        for kind in [Kind::Queen, Kind::Rook, Kind::Bishop, Kind::Knight] {
            let mut g = test_game();
            g.promotion_choice = true;
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
    fn flip_promotes_all_to_queens_by_default() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[0][4] = Some(Piece::new(Side::White, Kind::King)); // e1
        g.board[0][7] = Some(Piece::new(Side::Black, Kind::King)); // a8
        let mut e8 = Piece::new(Side::White, Kind::Pawn);
        e8.crowned = true;
        let mut h8 = Piece::new(Side::White, Kind::Pawn);
        h8.crowned = true;
        g.board[7][4] = Some(e8);
        g.board[7][7] = Some(h8);
        g.turn = Side::White;
        g.flip_phase();
        assert_eq!(g.pending_promotion, None, "по умолчанию без диалога");
        assert_eq!(g.piece_at((4, 7)).unwrap().kind, Kind::Queen);
        assert_eq!(g.piece_at((7, 7)).unwrap().kind, Kind::Queen);
    }

    #[test]
    fn flip_promotes_pawns_on_last_rank() {
        let mut g = test_game();
        g.promotion_choice = true;
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[0][4] = Some(Piece::new(Side::White, Kind::King)); // e1
        g.board[0][7] = Some(Piece::new(Side::Black, Kind::King)); // a8
        let mut e8 = Piece::new(Side::White, Kind::Pawn);
        e8.crowned = true;
        let mut h8 = Piece::new(Side::White, Kind::Pawn);
        h8.crowned = true;
        g.board[7][4] = Some(e8); // e8
        g.board[7][7] = Some(h8); // h8
        g.turn = Side::White;

        g.flip_phase(); // шашки -> шахматы
        assert_eq!(g.phase, Phase::Chess);
        assert_eq!(
            g.pending_promotion,
            Some((4, 7)),
            "первая пешка на последнем ряду ждёт выбора"
        );
        assert_eq!(
            g.piece_at((7, 7)).unwrap().kind,
            Kind::Queen,
            "вторая пешка сразу стала ферзём"
        );
        assert_eq!(g.turn, Side::White);

        g.choose_promotion(Kind::Queen);
        assert_eq!(g.piece_at((4, 7)).unwrap().kind, Kind::Queen);
        assert_eq!(
            g.turn,
            Side::White,
            "превращение после перехода не передаёт очередь"
        );
    }

    #[test]
    fn flip_to_checkers_crowns_all_last_rank_pieces() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Chess;
        g.crown_non_pawns = true;
        g.board[0][4] = Some(Piece::new(Side::White, Kind::King)); // e1
        g.board[7][4] = Some(Piece::new(Side::Black, Kind::King)); // e8
        g.board[7][3] = Some(Piece::new(Side::White, Kind::Queen)); // d8 — последний ряд белых
        g.board[0][3] = Some(Piece::new(Side::Black, Kind::Rook)); // d1 — последний ряд чёрных
        g.turn = Side::White;

        g.flip_phase();
        assert_eq!(g.phase, Phase::Checkers);
        assert!(
            g.piece_at((3, 7)).unwrap().crowned,
            "ферзь на последнем ряду стал дамкой при переходе"
        );
        assert!(
            g.piece_at((3, 0)).unwrap().crowned,
            "ладья на последнем ряду чёрных стала дамкой при переходе"
        );
        assert!(
            !g.piece_at((4, 0)).unwrap().crowned,
            "белый король не на своём последнем ряду — не дамка"
        );
        assert!(
            !g.piece_at((4, 7)).unwrap().crowned,
            "чёрный король не на своём последнем ряду — не дамка"
        );
        assert!(
            g.log
                .iter()
                .any(|l| l.contains("Дамки: все")),
            "в журнале отмечена массовая коронация"
        );
    }

    #[test]
    fn flip_to_checkers_crowns_only_pawns_when_restricted() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Chess;
        g.crown_non_pawns = false;
        g.board[0][4] = Some(Piece::new(Side::White, Kind::King)); // e1
        g.board[7][4] = Some(Piece::new(Side::Black, Kind::King)); // e8
        g.board[7][3] = Some(Piece::new(Side::White, Kind::Queen)); // d8
        g.board[0][3] = Some(Piece::new(Side::Black, Kind::Rook)); // d1
        g.turn = Side::White;

        g.flip_phase();
        assert_eq!(g.phase, Phase::Checkers);
        assert!(
            !g.piece_at((3, 7)).unwrap().crowned,
            "с «Дамки: пешки» ферзь не коронуется при переходе"
        );
        assert!(
            !g.piece_at((3, 0)).unwrap().crowned,
            "с «Дамки: пешки» ладья не коронуется при переходе"
        );
    }

    #[test]
    fn phase_timer_waits_for_move_animation() {
        let mut g = test_game();
        g.twilight_by_moves = false;
        g.phase_left = 0.01;
        g.move_anim = 0.0;
        g.tick(0.03);
        assert_eq!(g.trans, 0.0, "переход не начался во время анимации хода");
        for _ in 0..6 {
            g.tick(0.1);
        }
        assert!(g.trans > 0.0, "после анимации переход начался");
    }

    #[test]
    fn phase_hold_stops_phase_timer() {
        let mut g = test_game();
        g.twilight_by_moves = false;
        g.phase_left = 0.01;
        g.move_anim = 1.0;
        g.phase_hold = true;
        g.tick(0.1);
        assert_eq!(g.trans, 0.0, "на паузе фазы переход не начался");
        g.phase_hold = false;
        g.tick(0.1);
        assert!(g.trans > 0.0, "после снятия паузы переход начался");
    }

    #[test]
    fn grace_hold_stops_phase_timer_in_last_seconds() {
        let mut g = test_game();
        g.twilight_by_moves = false;
        g.phase_left = 4.0;
        g.move_anim = 1.0;
        // Игрок взял фигуру в последние 5 секунд до смены.
        g.selected = Some((4, 1));
        g.tick(0.1);
        assert_eq!(g.phase_left, 4.0, "таймер замер, пока фигура в руке");
        assert_eq!(g.trans, 0.0, "сумерки не начались");
        // Ход сделан — таймер снова идёт.
        g.selected = None;
        g.tick(0.1);
        assert!(g.phase_left < 4.0, "после хода таймер продолжил отсчёт");
    }

    #[test]
    fn grace_hold_stops_move_clock_in_last_seconds() {
        let mut g = test_game();
        g.twilight_by_moves = true;
        g.move_time_limit = 60.0;
        g.move_left = 4.0;
        g.move_anim = 1.0;
        g.selected = Some((4, 1));
        g.tick(0.1);
        assert_eq!(g.move_left, 4.0, "часы хода замерли, пока фигура в руке");
        assert_eq!(g.turn, Side::White, "ход не перешёл противнику");
        g.selected = None;
        g.tick(0.1);
        assert!(g.move_left < 4.0, "после хода часы снова идут");
    }

    #[test]
    fn grace_hold_only_in_last_five_seconds() {
        let mut g = test_game();
        g.twilight_by_moves = false;
        g.phase_left = 30.0;
        g.move_anim = 1.0;
        g.selected = Some((4, 1));
        g.tick(0.1);
        assert!(g.phase_left < 30.0, "вне последних 5 секунд таймер идёт");
    }

        /// Часы хода не идут в чужой ход: пока компьютер «думает», время игрока
    /// стоит, а после ответа отсчёт нового хода начинается сразу.
    #[test]
    fn move_clock_pauses_during_opponent_turn() {
        let mut g = test_game();
        g.twilight_by_moves = true;
        g.move_time_limit = 60.0;
        g.move_left = 60.0;
        g.move_anim = 1.0;
        for _ in 0..30 {
            g.tick(0.1);
        }
        assert!((g.move_left - 57.0).abs() < 0.05, "свои часы идут");
        g.apply(Move {
            from: (7, 1),
            to: (7, 3),
            captured: None,
        }); // h2-h4
        assert_eq!(g.turn, Side::Black);
        assert_eq!(g.move_left, 60.0, "новый ход — новые 60 секунд");
        // Ход компьютера: приложение выставляет phase_hold.
        g.phase_hold = true;
        for _ in 0..30 {
            g.tick(0.1);
        }
        assert_eq!(g.move_left, 60.0, "пока думает компьютер, часы стоят");
        // Ответил — часы пошли (минус анимация хода 0.22 с).
        g.phase_hold = false;
        for _ in 0..10 {
            g.tick(0.1);
        }
        assert!(
            g.move_left < 60.0 && g.move_left > 58.0,
            "после ответа отсчёт пошёл сразу: {}",
            g.move_left
        );
    }

    #[test]
    fn crown_option_can_restrict_damki_to_pawns() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.crown_non_pawns = false;
        g.board[6][1] = Some(Piece::new(Side::White, Kind::Knight)); // b7
        g.turn = Side::White;
        g.apply(Move {
            from: (1, 6),
            to: (0, 7),
            captured: None,
        });
        assert!(!g.board[7][0].unwrap().crowned, "не-пешка не коронуется");
        g.crown_non_pawns = true;
        g.board[6][1] = Some(Piece::new(Side::White, Kind::Knight));
        g.board[7][0] = None;
        g.turn = Side::White;
        g.apply(Move {
            from: (1, 6),
            to: (0, 7),
            captured: None,
        });
        assert!(g.board[7][0].unwrap().crowned, "не-пешка коронуется");
    }

    #[test]
    fn snapshots_grow_with_moves() {
        let mut g = test_game();
        let before = g.snapshots.len();
        let mv = g.all_moves()[0];
        g.apply(mv);
        assert_eq!(g.snapshots.len(), before + 1);
    }

    #[test]
    fn forced_capture_squares_are_reported() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[3][2] = Some(Piece::new(Side::White, Kind::Pawn)); // c4
        g.board[4][3] = Some(Piece::new(Side::Black, Kind::Pawn)); // d5
        g.turn = Side::White;
        assert_eq!(g.forced_capture_squares(), vec![(2, 3)]);
        g.mandatory_capture = false;
        assert!(g.forced_capture_squares().is_empty(), "без обязательного боя нет красной пульсации");
    }

    #[test]
    fn move_twilight_holds_time_and_arms_after_move() {
        let mut g = test_game();
        g.twilight_by_moves = true;
        g.phase_left = 10.0;
        g.phase_total = 10.0;
        g.tick(0.5);
        assert_eq!(g.phase_left, 10.0, "по ходам таймер не идёт");
        // Смена фаз идёт по полным ходам: после одного полухода её ещё нет.
        let mv = g.all_moves()[0];
        g.apply(mv);
        assert_eq!(g.plies_in_phase, 1);
        assert!(
            !g.move_twilight_pending,
            "после одного полухода смена не взводится"
        );
        while !g.move_twilight_pending {
            let mv = g.all_moves()[0];
            g.apply(mv);
        }
        assert_eq!(g.plies_in_phase, 0, "после полных ходов счётчик сброшен");
        g.phase_left = 10.0;
        g.tick(0.1);
        assert_eq!(g.phase_left, 10.0, "таймер по-прежнему не идёт");
    }

    #[test]
    fn move_twilight_waits_two_seconds_after_move() {
        let mut g = test_game();
        g.twilight_by_moves = true;
        g.move_twilight_pending = true;
        g.move_twilight_wait = MOVE_TWILIGHT_PAUSE;
        g.move_anim = 1.0;
        // 0,5 с: решение ещё не принято.
        for _ in 0..5 {
            g.tick(0.1);
        }
        assert!(g.move_twilight_pending, "через 0,5 с сумерки ещё не решены");
        // Ещё 1,6 с: суммарно 2,1 с — решение принято.
        for _ in 0..16 {
            g.tick(0.1);
        }
        assert!(!g.move_twilight_pending, "через 2,1 с решение уже принято");
    }

    #[test]
    fn move_clock_expires_and_passes_turn() {
        let mut g = test_game();
        g.twilight_by_moves = true;
        g.move_time_limit = 10.0;
        g.move_left = 0.05;
        g.tick(0.1);
        assert_eq!(g.turn, Side::Black, "время хода истекло — ход у чёрных");
        assert_eq!(g.move_left, 10.0, "часы сброшены на новый ход");
        assert!(
            !g.move_twilight_pending,
            "истечение времени не запускает сумерки"
        );
    }

    #[test]
    fn move_twilight_waits_while_piece_is_held() {
        let mut g = test_game();
        g.twilight_by_moves = true;
        g.move_twilight_pending = true;
        g.move_twilight_wait = MOVE_TWILIGHT_PAUSE;
        g.move_anim = 1.0;
        g.selected = Some((4, 1));
        // 2,5 с — дольше паузы, но фигура в руке: решение не принято.
        for _ in 0..25 {
            g.tick(0.1);
        }
        assert!(
            g.move_twilight_pending,
            "сумерки не решаются, пока фигура в руке"
        );
        g.selected = None;
        // Отпускание запускает отсчёт паузы: 2,1 с — решение принято.
        for _ in 0..21 {
            g.tick(0.1);
        }
        assert!(!g.move_twilight_pending, "после отпускания фигуры решение принято");
    }

    #[test]
    fn move_time_limit_is_clamped() {
        let mut g = test_game();
        g.set_move_time_limit(5.0);
        assert_eq!(g.move_time_limit, MIN_MOVE_SECS);
        g.set_move_time_limit(500.0);
        assert_eq!(g.move_time_limit, MAX_MOVE_SECS);
    }

    #[test]
    fn optional_capture_allows_quiet_move() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[3][2] = Some(Piece::new(Side::White, Kind::Pawn)); // c4
        g.board[4][3] = Some(Piece::new(Side::Black, Kind::Pawn)); // d5
        g.turn = Side::White;
        assert!(
            g.all_moves().iter().all(|m| m.captured.is_some()),
            "по умолчанию бить обязательно"
        );
        g.mandatory_capture = false;
        assert!(
            g.all_moves().iter().any(|m| m.captured.is_none()),
            "без обязательного взятия можно тихо ходить"
        );
        assert!(
            g.moves_for((2, 3)).iter().any(|m| m.captured.is_none()),
            "у c4 есть тихие ходы"
        );
    }

    #[test]
    fn optional_capture_can_end_chain() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.mandatory_capture = false;
        g.board[5][1] = Some(Piece::new(Side::White, Kind::Pawn)); // b6
        g.board[6][2] = Some(Piece::new(Side::Black, Kind::Pawn)); // c7
        g.board[5][5] = Some(Piece::new(Side::Black, Kind::Pawn)); // f6
        g.turn = Side::White;
        g.apply(Move {
            from: (1, 5),
            to: (3, 7),
            captured: Some((2, 6)),
        });
        assert!(g.chain.is_some(), "серия продолжается");
        g.end_chain();
        assert!(g.chain.is_none());
        assert_eq!(g.turn, Side::Black);
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
    fn both_stuck_triggers_rescue_twilight() {
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
        // первый пропуск, второй сразу запускает сумерки
        g.tick(0.05);
        assert_eq!(g.passes, 1, "первый пропуск");
        g.tick(0.05);
        assert!(g.trans > 0.0, "оба застряли — сумерки включились");
        assert_eq!(g.phase, Phase::Checkers, "фаза сменится после перехода");
        assert_eq!(g.passes, 0);
        assert!(g
            .log
            .iter()
            .any(|l| l.contains("сумерки выводят в шахматы")));
        // переход завершается — выходим в шахматы
        for _ in 0..40 {
            g.tick(0.05);
        }
        assert_eq!(g.phase, Phase::Chess, "сумерки вывели в шахматы");
        assert_eq!(g.trans, 0.0);
    }

    #[test]
    fn rescue_twilight_is_temporary() {
        let mut g = Game::new(25.0);
        g.twilight_by_moves = false;
        g.phase = Phase::Checkers;
        g.passes = 2;
        g.tick(0.05);
        assert!(g.trans > 0.0, "тупик запускает сумерки");
        for _ in 0..40 {
            g.tick(0.05);
        }
        assert_eq!(g.phase, Phase::Chess, "вышли в шахматы");
        // обычный таймер снова работает: следующий переход вернёт шашки
        g.phase_left = 0.0;
        g.tick(0.05);
        assert!(g.trans > 0.0, "обычные сумерки не отключены");
        for _ in 0..40 {
            g.tick(0.05);
        }
        assert_eq!(g.phase, Phase::Checkers, "цикл продолжается");
    }

    #[test]
    fn winning_capture_is_recorded() {
        let mut g = test_game();
        g.phase = Phase::Checkers;
        g.board = [[None; 8]; 8];
        let mut damka = Piece::new(Side::White, Kind::Rook);
        damka.crowned = true;
        g.board[3][3] = Some(damka); // d4
        g.board[4][4] = Some(Piece::new(Side::Black, Kind::King)); // e5
        g.turn = Side::White;
        g.apply(Move {
            from: (3, 3),
            to: (5, 5),
            captured: Some((4, 4)),
        });
        assert!(g.is_over());
        assert_eq!(g.last_move, Some(((3, 3), (5, 5))));
        assert_eq!(
            g.last_capture.map(|(sq, p)| (sq, p.kind)),
            Some(((4, 4), Kind::King))
        );
        assert!(g.piece_at((5, 5)).is_some());
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

    /// Дамка берёт короля наступлением на его клетку: на краю доски за
    /// королём нет поля приземления, иначе край был бы крепостью.
    #[test]
    fn damka_takes_king_on_board_edge() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[0][7] = Some(Piece::new(Side::White, Kind::King)); // h1 — край
        let mut damka = Piece::new(Side::Black, Kind::Rook);
        damka.crowned = true;
        g.board[3][4] = Some(damka); // e4 — по диагонали к h1
        g.turn = Side::Black;
        let moves = g.all_moves();
        assert!(
            moves.iter().any(|m| m.to == (7, 0) && m.captured == Some((7, 0))),
            "дамка должна взять короля на краю: {moves:?}"
        );
    }

    /// Простая шашка край не пробивает: ей нужна клетка за королём.
    #[test]
    fn simple_piece_cannot_take_king_on_edge() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Checkers;
        g.board[0][7] = Some(Piece::new(Side::White, Kind::King)); // h1 — край
        g.board[1][6] = Some(Piece::new(Side::Black, Kind::Pawn)); // g2
        g.turn = Side::Black;
        let moves = g.all_moves();
        assert!(
            moves.iter().all(|m| m.captured != Some((7, 0))),
            "простая шашка не берёт короля на краю: {moves:?}"
        );
    }

    /// Перед сменой фаз панель должна предупреждать: король под ударом.
    #[test]
    fn chess_phase_warns_about_king_after_flip() {
        let mut g = test_game();
        g.board = [[None; 8]; 8];
        g.phase = Phase::Chess;
        g.set_twilight_by_moves(true);
        g.set_phase_len(Phase::Chess, 1);
        g.board[1][5] = Some(Piece::new(Side::White, Kind::King)); // f2
        g.board[3][7] = Some(Piece::new(Side::Black, Kind::King)); // h4
        g.turn = Side::White;
        assert!(
            g.king_danger_after_flip(Side::White),
            "белый король на диагонали чёрного — смена фаз его погубит"
        );
    }

}
