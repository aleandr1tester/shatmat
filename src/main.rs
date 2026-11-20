#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod art;
mod fonts;
mod game;
mod i18n;
mod logging;
mod music;
mod sound;

use std::path::PathBuf;

use ai::{Difficulty, ALL_DIFFICULTIES};
use art::{
    alpha, blend, draw_checker, draw_chess_piece, glow_dot, gradient, mix, rr, smooth, spaced_text,
    sparkle, text, Palette,
};
use eframe::egui::{self, Align2, Color32, CursorIcon, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use game::{Game, Kind, Move, Phase, Piece, Side, Square};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Hotseat,
    Computer,
}

// ---------------------------------------------------------------------------
// Точка входа
// ---------------------------------------------------------------------------

fn main() -> eframe::Result<()> {
    i18n::init();
    let args: Vec<String> = std::env::args().collect();
    let mut shot_path: Option<PathBuf> = None;
    let mut demo: Option<String> = None;
    let mut write_sounds: Option<PathBuf> = None;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--screenshot" {
            if let Some(p) = args.get(i + 1) {
                shot_path = Some(PathBuf::from(p));
                i += 1;
            }
        }
        if args[i] == "--demo" {
            if let Some(p) = args.get(i + 1) {
                demo = Some(p.clone());
                i += 1;
            }
        }
        if args[i] == "--write-sounds" {
            if let Some(p) = args.get(i + 1) {
                write_sounds = Some(PathBuf::from(p));
                i += 1;
            }
        }
        i += 1;
    }
    if let Some(dir) = write_sounds {
        sound::write_defaults(&dir);
        return Ok(());
    }
    let shot_mode = shot_path.is_some();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("ModeShift: Bipolar — режимный сдвиг: биполярность")
        .with_inner_size([1240.0, 820.0])
        .with_min_inner_size([1040.0, 700.0]);

    if shot_mode {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }

    let native = eframe::NativeOptions {
        viewport,
        run_and_return: true,
        ..Default::default()
    };

    #[cfg(target_os = "macos")]
    let native = {
        let mut native = native;
        if shot_mode {
            native.event_loop_builder = Some(Box::new(|builder| {
                builder.with_activate_ignoring_other_apps(false);
            }));
        }
        native
    };

    let shot_for_app = shot_path.clone();
    let demo_for_app = demo.clone();
    eframe::run_native(
        "shatmat",
        native,
        Box::new(move |cc| {
            Ok(Box::new(App::new(cc, shot_for_app.clone(), demo_for_app.clone())))
        }),
    )
}

// ---------------------------------------------------------------------------
// Приложение
// ---------------------------------------------------------------------------

struct Shot {
    path: PathBuf,
    requested: bool,
    saved: bool,
}

#[derive(Clone, Copy)]
struct SoundMark {
    last_move: Option<(Square, Square)>,
    white: u32,
    black: u32,
}

struct PieceArt {
    color: egui::TextureHandle,
    mask: egui::TextureHandle,
}

struct PieceTextures {
    white: [Option<PieceArt>; 6],
    black: [Option<PieceArt>; 6],
    /// Шашки: [белая, чёрная].
    checker: [Option<PieceArt>; 2],
    damka: [Option<PieceArt>; 2],
    royal: [Option<PieceArt>; 2],
}

impl PieceTextures {
    fn empty() -> Self {
        Self {
            white: std::array::from_fn(|_| None),
            black: std::array::from_fn(|_| None),
            checker: std::array::from_fn(|_| None),
            damka: std::array::from_fn(|_| None),
            royal: std::array::from_fn(|_| None),
        }
    }

    fn get(&self, side: Side, kind: Kind) -> Option<&PieceArt> {
        let arr = match side {
            Side::White => &self.white,
            Side::Black => &self.black,
        };
        arr[kind_index(kind)].as_ref()
    }

    fn side_index(side: Side) -> usize {
        if side == Side::White {
            0
        } else {
            1
        }
    }

    /// Картинка шашки: пешка-король, дамка или обычная шашка.
    fn checker_art(&self, side: Side, damka: bool, royal: bool) -> Option<&PieceArt> {
        let i = Self::side_index(side);
        if royal {
            if let Some(a) = self.royal[i].as_ref() {
                return Some(a);
            }
        }
        if damka {
            if let Some(a) = self.damka[i].as_ref() {
                return Some(a);
            }
        }
        self.checker[i].as_ref()
    }

    fn has_royal(&self, side: Side) -> bool {
        self.royal[Self::side_index(side)].is_some()
    }

    fn has_damka(&self, side: Side) -> bool {
        self.damka[Self::side_index(side)].is_some()
    }
}

fn kind_index(kind: Kind) -> usize {
    match kind {
        Kind::Pawn => 0,
        Kind::Knight => 1,
        Kind::Bishop => 2,
        Kind::Rook => 3,
        Kind::Queen => 4,
        Kind::King => 5,
    }
}

fn kind_file(kind: Kind) -> &'static str {
    match kind {
        Kind::Pawn => "pawn",
        Kind::Knight => "knight",
        Kind::Bishop => "bishop",
        Kind::Rook => "rook",
        Kind::Queen => "queen",
        Kind::King => "king",
    }
}

fn pieces_dir() -> PathBuf {
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
    {
        let p = dir.join("pieces");
        if p.is_dir() {
            return p;
        }
    }
    PathBuf::from("pieces")
}

/// Растрирует SVG в RGBA-пиксели. Картинки фигур — векторные, поэтому
/// SVG читается в любом размере без потери качества.
fn rasterize_svg(data: &[u8], target: u32) -> Option<(Vec<u8>, usize, usize)> {
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(data, &opt).ok()?;
    let size = tree.size();
    let (w, h) = (size.width(), size.height());
    if !(w > 0.0 && h > 0.0) {
        return None;
    }
    let scale = target as f32 / w.max(h);
    let pw = ((w * scale).round() as u32).max(1);
    let ph = ((h * scale).round() as u32).max(1);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(pw, ph)?;
    let ts = resvg::tiny_skia::Transform::from_scale(pw as f32 / w, ph as f32 / h);
    resvg::render(&tree, ts, &mut pixmap.as_mut());
    let rgba = pixmap.take_demultiplied();
    Some((rgba, pw as usize, ph as usize))
}

/// Загружает картинку фигуры: сначала пробует `.svg` (вектор), затем `.png`.
fn load_piece_textures(ctx: &egui::Context) -> PieceTextures {
    let mut out = PieceTextures::empty();
    let dir = pieces_dir();
    let load = |name: &str| -> Option<PieceArt> {
        let svg_path = dir.join(format!("{name}.svg"));
        let png_path = dir.join(format!("{name}.png"));
        let (rgba, w, h) = if svg_path.is_file() {
            let bytes = std::fs::read(&svg_path).ok()?;
            rasterize_svg(&bytes, 512)?
        } else {
            let bytes = std::fs::read(&png_path).ok()?;
            let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
            let (w, h) = (img.width() as usize, img.height() as usize);
            (img.into_raw(), w, h)
        };
        let color = egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba);
        let color_tex = ctx.load_texture(
            format!("piece_{name}"),
            color,
            egui::TextureOptions::LINEAR,
        );
        let mut mask_rgba = rgba;
        for px in mask_rgba.chunks_exact_mut(4) {
            px[0] = 255;
            px[1] = 255;
            px[2] = 255;
        }
        let mask = egui::ColorImage::from_rgba_unmultiplied([w, h], &mask_rgba);
        let mask_tex = ctx.load_texture(
            format!("piece_{name}_mask"),
            mask,
            egui::TextureOptions::LINEAR,
        );
        Some(PieceArt {
            color: color_tex,
            mask: mask_tex,
        })
    };
    for side in [Side::White, Side::Black] {
        let prefix = if side == Side::White { "white" } else { "black" };
        for kind in [
            Kind::Pawn,
            Kind::Knight,
            Kind::Bishop,
            Kind::Rook,
            Kind::Queen,
            Kind::King,
        ] {
            let name = format!("{}_{}", prefix, kind_file(kind));
            if let Some(art) = load(&name) {
                let arr = match side {
                    Side::White => &mut out.white,
                    Side::Black => &mut out.black,
                };
                arr[kind_index(kind)] = Some(art);
            }
        }
        let i = PieceTextures::side_index(side);
        out.checker[i] = load(&format!("{prefix}_checker"));
        out.damka[i] = load(&format!("{prefix}_damka"));
        out.royal[i] = load(&format!("{prefix}_royal"));
    }
    out
}

struct Layout {
    full: Rect,
    header: Rect,
    frame: Rect,
    board: Rect,
    panel: Rect,
    cell: f32,
}

impl Layout {
    fn new(ctx: &egui::Context) -> Self {
        let full = ctx.screen_rect();
        let m = 26.0;
        let header_h = 84.0;
        let panel_w = (full.width() * 0.30).clamp(304.0, 384.0);
        let gap = 24.0;
        let board_area = Rect::from_min_max(
            Pos2::new(full.left() + m, full.top() + header_h),
            Pos2::new(full.right() - m - panel_w - gap, full.bottom() - m),
        );
        let side = board_area.width().min(board_area.height()).max(240.0);
        let frame = Rect::from_center_size(board_area.center(), Vec2::splat(side));
        let board = frame.shrink(16.0);
        let cell = board.width() / 8.0;
        Layout {
            full,
            header: Rect::from_min_max(
                Pos2::new(full.left() + m, full.top()),
                Pos2::new(full.right() - m, full.top() + header_h),
            ),
            frame,
            board,
            panel: Rect::from_min_max(
                Pos2::new(full.right() - m - panel_w, full.top() + header_h),
                Pos2::new(full.right() - m, full.bottom() - m),
            ),
            cell,
        }
    }

    fn square_rect(&self, sq: Square) -> Rect {
        let x = self.board.left() + sq.0 as f32 * self.cell;
        let y = self.board.bottom() - (sq.1 as f32 + 1.0) * self.cell;
        Rect::from_min_size(Pos2::new(x, y), Vec2::splat(self.cell))
    }

    fn square_at(&self, pos: Pos2) -> Option<Square> {
        if !self.board.contains(pos) {
            return None;
        }
        let f = ((pos.x - self.board.left()) / self.cell).floor() as i32;
        let r = ((self.board.bottom() - pos.y) / self.cell).floor() as i32;
        if (0..8).contains(&f) && (0..8).contains(&r) {
            Some((f as usize, r as usize))
        } else {
            None
        }
    }
}

#[derive(Clone, Copy)]
struct GuideDemo {
    from: Square,
    to: Square,
    victim: Option<Square>,
    t: f32,
}

pub struct App {
    game: Game,
    time: f32,
    rules: bool,
    rules_rect: Rect,
    settings: bool,
    settings_rect: Rect,
    piece_tex: PieceTextures,
    mode: Mode,
    difficulty: Difficulty,
    ai_wait: f32,
    ai_seed: u64,
    ai_preview: Option<Move>,
    ai_preview_phase: Option<Phase>,
    shot: Option<Shot>,
    frames: u32,
    twilight_sounded: bool,
    replay: f32,
    logging_allowed: bool,
    log_enabled: bool,
    show_checker_icons: bool,
    mate_lines: bool,
    mate_escape: bool,
    font_faces: Vec<fonts::FontFace>,
    font_index: usize,
    game_start: Option<String>,
    log_written: bool,
    last_log: Option<PathBuf>,
    journal: bool,
    journal_rect: Rect,
    journal_text: String,
    review: bool,
    review_saved: Option<Game>,
    review_ply: usize,
    guide: bool,
    guide_kind: Kind,
    guide_rect: Rect,
    guide_demo: Option<GuideDemo>,
    guide_checkers: bool,
    guide_damka: bool,
    started: bool,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, shot: Option<PathBuf>, demo: Option<String>) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        style.animation_time = 0.16;
        style.visuals = egui::Visuals::dark();
        cc.egui_ctx.set_style(style);

        let mut game = initial_game();
        let demo_game = if demo.as_deref() == Some("journal") {
            Some("win")
        } else {
            demo.as_deref()
        };
        apply_demo(&mut game, demo_game);
        let rules_open = demo.as_deref() == Some("rules");
        let settings_open = matches!(
            demo.as_deref(),
            Some("settings") | Some("settings_times") | Some("settings_auto") | Some("settings_moves")
        );
        let piece_tex = load_piece_textures(&cc.egui_ctx);
        let font_faces = fonts::load();
        let font_index = 0;
        fonts::apply(&cc.egui_ctx, &font_faces, font_index);
        sound::prepare();
        // Музыка по умолчанию выключена; включается в настройках.
        let logging_allowed = shot.is_none() && demo.is_none();
        let game_start = None;

        let mut app = App {
            game,
            time: 0.0,
            rules: rules_open,
            rules_rect: Rect::NOTHING,
            settings: settings_open,
            settings_rect: Rect::NOTHING,
            piece_tex,
            mode: Mode::Hotseat,
            difficulty: Difficulty::Medium,
            ai_wait: 0.6,
            ai_seed: 0x9E37_79B9_7F4A_7C15,
            ai_preview: None,
            ai_preview_phase: None,
            shot: shot.map(|path| Shot {
                path,
                requested: false,
                saved: false,
            }),
            frames: 0,
            twilight_sounded: false,
            replay: 0.0,
            logging_allowed,
            log_enabled: true,
            show_checker_icons: true,
            mate_lines: true,
            mate_escape: true,
            font_faces,
            font_index,
            game_start,
            log_written: false,
            last_log: None,
            journal: false,
            journal_rect: Rect::NOTHING,
            journal_text: String::new(),
            review: false,
            review_saved: None,
            review_ply: 0,
            guide: false,
            guide_kind: Kind::Pawn,
            guide_rect: Rect::NOTHING,
            guide_demo: None,
            guide_checkers: false,
            guide_damka: false,
            started: demo.is_some(),
        };
        if demo.as_deref() == Some("journal") {
            app.journal_text = app.build_journal_text();
            app.journal = true;
        }
        if demo.as_deref() == Some("review") {
            app.start_review();
        }
        if demo.as_deref() == Some("guide") {
            app.guide = true;
            app.guide_kind = Kind::Pawn;
            app.guide_checkers = false;
            app.guide_damka = false;
            app.guide_demo = None;
        }
        if demo.as_deref() == Some("guide_checkers") {
            app.guide = true;
            app.guide_kind = Kind::King;
            app.guide_checkers = true;
            app.guide_damka = false;
            app.guide_demo = None;
        }
        app
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        if self.review {
            return;
        }
        if self.guide {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.guide = false;
            }
            return;
        }
        if !self.started {
            let (space, enter, h, esc) = ctx.input(|i| {
                (
                    i.key_pressed(egui::Key::Space),
                    i.key_pressed(egui::Key::Enter),
                    i.key_pressed(egui::Key::H) || i.key_pressed(egui::Key::Questionmark),
                    i.key_pressed(egui::Key::Escape),
                )
            });
            if esc {
                if self.journal {
                    self.journal = false;
                } else if self.settings {
                    self.settings = false;
                } else if self.rules {
                    self.rules = false;
                }
            }
            if !self.settings && !self.rules && !self.journal {
                if space || enter {
                    self.start_game();
                    return;
                }
                if h {
                    self.rules = true;
                }
            }
            return;
        }
        let (space, r, h, esc) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Space),
                i.key_pressed(egui::Key::R),
                i.key_pressed(egui::Key::H) || i.key_pressed(egui::Key::Questionmark),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if esc {
            if self.journal {
                self.journal = false;
            } else if self.settings {
                self.settings = false;
            } else if self.rules {
                self.rules = false;
            } else {
                self.game.deselect();
            }
        }
        if h && !self.settings && !self.journal {
            self.rules = !self.rules;
        }
        if space && !self.rules && !self.settings && !self.journal {
            self.game.paused = !self.game.paused;
        }
        if r && !self.rules && !self.settings && !self.journal {
            self.new_game();
        }
    }

    fn finalize_log(&mut self, interrupted: &str) {
        if !self.logging_allowed || !self.log_enabled || self.log_written {
            return;
        }
        let Some(start) = self.game_start.clone() else {
            return;
        };
        let result = if !interrupted.is_empty() {
            interrupted.to_string()
        } else {
            match &self.game.status {
                game::Status::Win { side, reason } => i18n::logf(
                    "победа {side} — {reason}",
                    &[("side", side.name_log_lower()), ("reason", &i18n::log_owned(reason.clone()))],
                ),
                _ => i18n::log("не завершена").to_string(),
            }
        };
        let mode = self.mode_label();
        if let Some(path) = logging::write_game_log(&self.game, &start, &mode, &result) {
            self.last_log = Some(path);
        }
        self.log_written = true;
        self.game_start = None;
    }

    fn mode_label(&self) -> String {
        match self.mode {
            Mode::Hotseat => i18n::log("вдвоём").to_string(),
            Mode::Computer => i18n::logf(
                "с компьютером ({level})",
                &[("level", self.difficulty.label_log())],
            ),
        }
    }

    fn build_journal_text(&self) -> String {
        if let Some(path) = &self.last_log {
            if let Ok(text) = std::fs::read_to_string(path) {
                return text;
            }
        }
        let start = self
            .game_start
            .clone()
            .unwrap_or_else(|| "—".to_string());
        let end = logging::now_string();
        let result = match &self.game.status {
            game::Status::Win { side, reason } => i18n::logf(
                "победа {side} — {reason}",
                &[("side", side.name_log_lower()), ("reason", &i18n::log_owned(reason.clone()))],
            ),
            _ => i18n::log("не завершена").to_string(),
        };
        logging::format_game_log(&self.game, &start, &end, &self.mode_label(), &result)
    }

    fn start_game(&mut self) {
        if self.started {
            return;
        }
        self.started = true;
        self.game.paused = false;
        if self.logging_allowed && self.log_enabled && self.game_start.is_none() {
            self.game_start = Some(logging::now_string());
            self.log_written = false;
        }
    }

    fn new_game(&mut self) {
        self.finalize_log("прервана (новая партия)");
        let smooth = self.game.move_anim_secs;
        let promotion_choice = self.game.promotion_choice;
        let chess_len = self.game.chess_len;
        let checkers_len = self.game.checkers_len;
        let auto_phase = self.game.auto_phase;
        let auto_max = self.game.auto_max;
        let twilight_by_moves = self.game.twilight_by_moves;
        let move_time_limit = self.game.move_time_limit;
        let mandatory_capture = self.game.mandatory_capture;
        let crown_non_pawns = self.game.crown_non_pawns;
        self.game = Game::new(chess_len);
        self.game.checkers_len = checkers_len;
        self.game.auto_phase = auto_phase;
        self.game.auto_max = auto_max;
        self.game.twilight_by_moves = twilight_by_moves;
        self.game.move_time_limit = move_time_limit;
        self.game.move_left = move_time_limit;
        self.game.mandatory_capture = mandatory_capture;
        self.game.crown_non_pawns = crown_non_pawns;
        self.game.seed_rng(random_seed());
        self.game.refresh_phase_timer();
        self.game.move_anim_secs = smooth;
        self.game.promotion_choice = promotion_choice;
        self.ai_wait = 0.6;
        self.ai_preview = None;
        self.ai_preview_phase = None;
        self.review = false;
        self.review_saved = None;
        self.review_ply = 0;
        self.twilight_sounded = false;
        self.replay = 0.0;
        self.game_start = if self.started && self.logging_allowed {
            Some(logging::now_string())
        } else {
            None
        };
        self.log_written = false;
        self.last_log = None;
        self.journal = false;
        self.journal_text.clear();
    }

    fn sound_mark(&self) -> SoundMark {
        SoundMark {
            last_move: self.game.last_move,
            white: self.game.captured_by_white,
            black: self.game.captured_by_black,
        }
    }

    /// Звук подбираем по разнице состояния: взятие важнее обычного хода.
    fn play_sound_since(&self, mark: SoundMark) {
        if self.game.captured_by_white > mark.white || self.game.captured_by_black > mark.black {
            sound::capture();
        } else if self.game.last_move != mark.last_move {
            sound::piece_move();
        }
    }

    /// Компьютер играет чёрными: 2–3 секунды показывает цель, затем ходит.
    fn update_ai(&mut self, dt: f32) {
        if !self.started {
            self.ai_preview = None;
            self.ai_preview_phase = None;
            self.ai_wait = 0.6;
            return;
        }
        if self.mode != Mode::Computer {
            self.ai_preview = None;
            self.ai_preview_phase = None;
            self.ai_wait = 0.6;
            return;
        }
        if self.rules
            || self.settings
            || self.game.paused
            || self.game.is_over()
            || self.game.trans > 0.0
            || self.game.pending_promotion.is_some()
        {
            return;
        }
        if self.game.turn != Side::Black {
            self.ai_preview = None;
            self.ai_preview_phase = None;
            self.ai_wait = 0.6;
            return;
        }
        if self.ai_preview.is_some() && self.ai_preview_phase != Some(self.game.phase) {
            // Пока компьютер «думал», сумерки успели сменить фазу.
            // Старый ход по правилам другой игры больше не годится.
            self.ai_preview = None;
            self.ai_preview_phase = None;
            self.ai_wait = 0.35;
            return;
        }
        if self.ai_preview.is_none() {
            let mut seed = self.ai_seed;
            let chosen = ai::best_move(&self.game, self.difficulty, &mut seed);
            self.ai_seed = seed;
            if let Some(mv) = chosen {
                self.ai_preview = Some(mv);
                self.ai_preview_phase = Some(self.game.phase);
                let frac = (seed % 1000) as f32 / 1000.0;
                self.ai_wait = 2.0 + frac;
            } else {
                self.ai_wait = 0.6;
            }
        }
        self.ai_wait -= dt;
        if self.ai_wait > 0.0 {
            return;
        }
        if let Some(mv) = self.ai_preview.take() {
            self.ai_preview_phase = None;
            let still_legal = self.game.all_moves().iter().any(|m| {
                m.from == mv.from && m.to == mv.to && m.captured == mv.captured
            });
            if still_legal {
                let mark = self.sound_mark();
                self.game.apply(mv);
                if self.game.pending_promotion.is_some() {
                    self.game.choose_promotion(Kind::Queen);
                }
                self.play_sound_since(mark);
                self.ai_wait = 0.6;
            } else {
                self.ai_wait = 0.3;
            }
        } else {
            self.ai_wait = 0.6;
        }
    }

    fn start_review(&mut self) {
        if self.game.snapshots.len() < 2 {
            return;
        }
        let saved = self.game.clone();
        let mut view = saved.clone();
        view.status = game::Status::Playing;
        view.selected = None;
        view.legal.clear();
        view.chain = None;
        view.trans = 0.0;
        view.paused = false;
        view.pending_promotion = None;
        self.review_saved = Some(saved);
        self.review = true;
        self.game = view;
        self.review_ply = 0;
        self.ai_preview = None;
        self.journal = false;
        self.settings = false;
        self.rules = false;
        self.set_review_ply(0);
    }

    fn exit_review(&mut self) {
        if let Some(saved) = self.review_saved.take() {
            self.game = saved;
        }
        self.review = false;
        self.review_ply = 0;
    }

    fn set_review_ply(&mut self, ply: usize) {
        let snap = match &self.review_saved {
            Some(saved) => saved.snapshots.get(ply).copied(),
            None => None,
        };
        let Some(snap) = snap else {
            return;
        };
        self.game.board = snap.board;
        self.game.turn = snap.turn;
        self.game.phase = snap.phase;
        self.game.last_move = snap.last_move;
        self.game.last_capture = snap.last_capture;
        self.game.move_anim = 0.0;
        self.game.status = game::Status::Playing;
        self.game.selected = None;
        self.game.legal.clear();
        self.game.chain = None;
        self.game.trans = 0.0;
        self.game.paused = false;
        self.game.pending_promotion = None;
        self.review_ply = ply;
    }

    fn review_labels(&self) -> Vec<String> {
        let Some(saved) = &self.review_saved else {
            return Vec::new();
        };
        let mut labels = vec![i18n::tr("0. Начальная позиция").to_string()];
        for (i, r) in saved.history.iter().enumerate() {
            let side = if r.side == Side::White {
                i18n::tr("белые")
            } else {
                i18n::tr("чёрные")
            };
            let piece = if r.phase == Phase::Checkers {
                if r.damka {
                    i18n::tr("дамка")
                } else {
                    i18n::tr("шашка")
                }
            } else {
                piece_ru(r.kind)
            };
            let phase = if r.phase == Phase::Checkers {
                i18n::tr("шашки")
            } else {
                i18n::tr("шахматы")
            };
            let cap = if r.captured.is_some() { "×" } else { "–" };
            labels.push(format!(
                "{}. {} {} {}{}{} · {}",
                i + 1,
                side,
                piece,
                game::sq_name(r.from),
                cap,
                game::sq_name(r.to),
                phase
            ));
        }
        labels
    }

    fn handle_pointer(&mut self, ctx: &egui::Context, layout: &Layout) {
        if self.review {
            return;
        }
        if self.guide {
            if ctx.input(|i| i.pointer.primary_clicked()) {
                if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                    if !self.guide_rect.contains(pos) {
                        self.guide = false;
                    }
                }
            }
            return;
        }
        let clicked = ctx.input(|i| i.pointer.primary_clicked());
        if !clicked {
            return;
        }
        let pos = match ctx.input(|i| i.pointer.interact_pos()) {
            Some(p) => p,
            None => return,
        };
        if self.journal {
            if !self.journal_rect.contains(pos) {
                self.journal = false;
            }
            return;
        }
        if self.settings {
            if !self.settings_rect.contains(pos) {
                self.settings = false;
            }
            return;
        }
        if self.game.pending_promotion.is_some() {
            return;
        }
        if self.rules {
            if !self.rules_rect.contains(pos) {
                self.rules = false;
            }
            return;
        }
        if !self.started {
            return;
        }
        if self.game.paused {
            let pill = Rect::from_center_size(layout.board.center(), Vec2::new(470.0, 104.0));
            if pill.contains(pos) {
                self.game.paused = false;
            }
            return;
        }
        if self.game.is_over() {
            return;
        }
        if self.mode == Mode::Computer && self.game.turn == Side::Black {
            return;
        }
        if let Some(sq) = layout.square_at(pos) {
            let mark = self.sound_mark();
            self.game.click(sq);
            self.play_sound_since(mark);
        }
    }

    fn draw(&mut self, ctx: &egui::Context, layout: &Layout) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(8, 8, 13)))
            .show(ctx, |ui| {
                let painter = ui.painter().clone();
                self.paint_background(ctx, &painter, layout);
                self.paint_header(&painter, layout);
                self.paint_board(ctx, &painter, layout);
                if !self.started {
                    self.paint_start_frost(&painter, layout);
                }
                if self.review {
                    self.paint_review(ui, &painter, layout);
                } else {
                    self.paint_panel(ui, &painter, layout);
                    self.paint_promotion(ui, &painter, layout);
                    if self.settings {
                        self.paint_settings(ui, &painter, layout);
                    }
                    if self.rules && !self.settings {
                        self.paint_rules(ui, ctx, &painter, layout);
                    }
                    if self.game.is_over() && !self.rules && !self.settings {
                        self.paint_result(ui, ctx, &painter, layout);
                    }
                    if self.journal {
                        self.paint_journal(ui, &painter, layout);
                    }
                }
                if self.guide {
                    self.paint_guide(ui, &painter, layout);
                }
            });
    }

    // ------------------------- фон -------------------------

    fn paint_background(&self, ctx: &egui::Context, painter: &egui::Painter, layout: &Layout) {
        let phase = self.game.phase;
        let pal = Palette::for_phase(phase);
        let next = Palette::for_phase(phase.other());
        let m = smooth(self.game.trans);
        let mixed = blend(pal, next, m * 0.7);

        gradient(painter, layout.full, mixed.bg_top, mixed.bg_bottom);

        // мягкие цветные пятна
        let t = self.time;
        radial(
            painter,
            Pos2::new(
                layout.full.left() + layout.full.width() * 0.24 + (t * 0.10).sin() * 24.0,
                layout.full.top() + layout.full.height() * 0.30,
            ),
            460.0,
            mixed.accent,
            0.055,
        );
        radial(
            painter,
            Pos2::new(
                layout.full.right() - 160.0,
                layout.full.bottom() - 120.0 + (t * 0.13).cos() * 18.0,
            ),
            420.0,
            mix(mixed.accent, Color32::from_rgb(120, 90, 200), 0.55),
            0.05,
        );
        radial(
            painter,
            Pos2::new(layout.full.center().x, layout.full.top()+40.0),
            520.0,
            Color32::from_rgb(70, 60, 120),
            0.035,
        );

        // звёздная пыль
        for i in 0..90u32 {
            let h1 = hash(i as f32 * 1.31);
            let h2 = hash(i as f32 * 7.77 + 3.3);
            let drift = (t * (0.004 + h2 * 0.006)) % 1.0;
            let x = layout.full.left() + ((h1 + drift * 0.35) % 1.0) * layout.full.width();
            let y = layout.full.top()
                + ((h2 + drift) % 1.0) * layout.full.height();
            let tw = 0.30 + 0.70 * (0.5 + 0.5 * (t * 0.9 + h1 * 6.28).sin());
            let r = 0.6 + h2 * 1.1;
            painter.circle_filled(
                Pos2::new(x, y),
                r,
                alpha(Color32::from_rgb(230, 220, 240), 0.09 + 0.16 * tw),
            );
        }

        // виньетка
        for i in 0..10 {
            let f = i as f32 / 10.0;
            let r = layout.full.expand(40.0 - f * 40.0);
            painter.rect_stroke(
                r,
                0.0,
                Stroke::new(6.0_f32, Color32::from_black_alpha(6)),
                egui::StrokeKind::Middle,
            );
        }

        let _ = ctx;
    }

    // ------------------------- шапка -------------------------

    fn paint_header(&self, painter: &egui::Painter, layout: &Layout) {
        let pal = Palette::for_phase(self.game.phase);
        let x = layout.header.left();
        spaced_text(
            painter,
            Pos2::new(x, layout.header.top() + 22.0),
            "MODESHIFT: BIPOLAR",
            28.0,
            pal.text,
            3.5,
        );
        text(
            painter,
            Pos2::new(x + 1.0, layout.header.top() + 57.0),
            Align2::LEFT_TOP,
            "режимный сдвиг: биполярность · фигуры меняют природу",
            12.0,
            alpha(pal.text_dim, 0.95),
        );
    }

    // ------------------------- доска -------------------------

    /// Активная анимация хода: (откуда, куда, прогресс 0..1).
    fn active_move_anim(&self, g: &Game) -> Option<(Square, Square, f32)> {
        let (from, to) = g.last_move?;
        if g.is_over() {
            let p = ((self.replay % 4.0) / 2.8).clamp(0.0, 1.0);
            if p >= 1.0 {
                return None;
            }
            Some((from, to, smooth(p)))
        } else if g.move_anim < 1.0 {
            Some((from, to, smooth(g.move_anim)))
        } else {
            None
        }
    }

    fn paint_board(&mut self, ctx: &egui::Context, painter: &egui::Painter, layout: &Layout) {
        let g = &self.game;
        let phase = g.phase;
        let pal = Palette::for_phase(phase);
        let next = Palette::for_phase(phase.other());
        let m = smooth(g.trans);
        let board_diag = layout.board.width() * 1.4142;

        // тень и рамка
        for i in 0..6 {
            let e = 6.0 + i as f32 * 3.0;
            painter.rect_filled(
                layout.frame.expand(e),
                26.0 + e * 0.4,
                Color32::from_black_alpha(12),
            );
        }
        rr(
            painter,
            layout.frame,
            24.0,
            alpha(pal.frame, 0.94),
            Stroke::new(1.0_f32, alpha(Color32::WHITE, 0.07)),
        );
        rr(
            painter,
            layout.board.expand(4.0),
            14.0,
            alpha(Color32::BLACK, 0.25),
            Stroke::NONE,
        );

        // клетки: волна сумерек идёт от центра к краям
        let ring = m * 1.25;
        for r in 0..8usize {
            for f in 0..8usize {
                let rect = layout.square_rect((f, r));
                let light = (f + r) % 2 == 1;
                let old_c = if light { pal.square_light } else { pal.square_dark };
                let new_c = if light {
                    next.square_light
                } else {
                    next.square_dark
                };
                let local = if g.trans > 0.0 {
                    let dn = (rect.center() - layout.board.center()).length() / (board_diag * 0.5);
                    smooth(((ring - dn) / 0.12).clamp(0.0, 1.0))
                } else {
                    0.0
                };
                let c = mix(old_c, new_c, local);
                painter.rect_filled(rect.shrink(1.0), 5.0, c);

                // координаты
                let dark_text = luminance(c) > 0.45;
                let cc = if dark_text {
                    alpha(Color32::from_rgb(40, 32, 26), 0.42)
                } else {
                    alpha(Color32::WHITE, 0.36)
                };
                if r == 0 {
                    text(
                        painter,
                        rect.right_bottom() + Vec2::new(-6.0, -4.0),
                        Align2::RIGHT_BOTTOM,
                        ((b'a' + f as u8) as char).to_string(),
                        10.0,
                        cc,
                    );
                }
                if f == 0 {
                    text(
                        painter,
                        rect.left_top() + Vec2::new(5.0, 4.0),
                        Align2::LEFT_TOP,
                        (r + 1).to_string(),
                        10.0,
                        cc,
                    );
                }
            }
        }

        // последний ход
        if let Some((from, to)) = g.last_move {
            for sq in [from, to] {
                painter.rect_filled(
                    layout.square_rect(sq).shrink(1.0),
                    5.0,
                    alpha(pal.accent, 0.16),
                );
            }
        }

        // рамка внутри доски
        painter.rect_stroke(
            layout.board.expand(1.5),
            12.0,
            Stroke::new(1.0_f32, alpha(Color32::WHITE, 0.10)),
            egui::StrokeKind::Inside,
        );

        // выделение
        if let Some(sel) = g.selected {
            let rect = layout.square_rect(sel);
            painter.rect_filled(rect.shrink(1.0), 5.0, alpha(pal.accent, 0.14));
            painter.rect_stroke(
                rect.shrink(2.0),
                5.0,
                Stroke::new(2.0_f32, alpha(pal.accent, 0.85)),
                egui::StrokeKind::Inside,
            );
        }

        // наведение
        let hover_sq = ctx
            .input(|i| i.pointer.hover_pos())
            .and_then(|p| layout.square_at(p));
        if let Some(h) = hover_sq {
            if !g.is_locked() && !self.rules {
                let target = g.legal.iter().any(|mv| mv.to == h);
                let own = g
                    .piece_at(h)
                    .map(|p| p.side == g.turn)
                    .unwrap_or(false);
                if target || own {
                    painter.rect_stroke(
                        layout.square_rect(h).shrink(1.5),
                        5.0,
                        Stroke::new(1.5_f32, alpha(pal.text, 0.35)),
                        egui::StrokeKind::Inside,
                    );
                    ctx.set_cursor_icon(CursorIcon::PointingHand);
                }
            }
        }

        // сумеречная дымка и водяной знак — под фигурами
        if g.trans > 0.0 {
            let a = bell(g.trans);
            painter.rect_filled(
                layout.board,
                8.0,
                alpha(Color32::from_rgb(48, 30, 62), 0.10 * a),
            );
            let wm = "СУМЕРКИ";
            let wm_w = art::spaced_width(painter, wm, 46.0, 12.0);
            spaced_text(
                painter,
                Pos2::new(
                    layout.board.center().x - wm_w * 0.5,
                    layout.board.center().y - 32.0,
                ),
                wm,
                46.0,
                alpha(Color32::from_rgb(244, 226, 196), 0.20 * a),
                12.0,
            );
        }

        // фигуры
        let anim = self.active_move_anim(g);
        for r in 0..8usize {
            for f in 0..8usize {
                let sq = (f, r);
                let seed = f as f32 * 0.83 + r as f32 * 1.71;
                let base_size = layout.cell * 1.08;
                if let Some(piece) = g.piece_at(sq) {
                    let mut center = layout.square_rect(sq).center();
                    let mut size = base_size;
                    let mut moving = false;
                    if let Some((from, to, t)) = anim {
                        if sq == to {
                            center = lerp_pos(
                                layout.square_rect(from).center(),
                                layout.square_rect(to).center(),
                                t,
                            );
                            size *= 1.0 + 0.06 * (std::f32::consts::PI * t).sin();
                            moving = true;
                        }
                    }
                    if g.selected == Some(sq) && g.trans == 0.0 {
                        center.y -= layout.cell * 0.035;
                    }
                    if moving {
                        glow_dot(painter, center, layout.cell * 0.34, pal.accent, 0.32);
                    }
                    if g.selected == Some(sq) {
                        glow_dot(
                            painter,
                            center,
                            layout.cell * 0.42,
                            pal.accent,
                            0.55,
                        );
                    }
                    if piece.side == g.turn
                        && g.trans == 0.0
                        && !g.is_over()
                        && g.pending_promotion.is_none()
                        && !g.moves_for(sq).is_empty()
                    {
                        // Мягкая пульсация по контуру фигур того, кто ходит.
                        let pulse = 0.5 + 0.5 * (self.time * 3.6 + seed * 0.35).sin();
                        let (glow, strength) = if piece.side == Side::White {
                            (
                                Color32::from_rgb(255, 248, 236),
                                0.18 + 0.28 * pulse,
                            )
                        } else {
                            (
                                Color32::from_rgb(104, 108, 118),
                                0.20 + 0.30 * pulse,
                            )
                        };
                        if phase == Phase::Chess {
                            if let Some(art) = self.piece_tex.get(piece.side, piece.kind) {
                                let uv = Rect::from_min_max(
                                    Pos2::new(0.0, 0.0),
                                    Pos2::new(1.0, 1.0),
                                );
                                let rim = if piece.side == Side::White {
                                    Color32::from_rgb(64, 46, 34)
                                } else {
                                    Color32::from_rgb(214, 218, 228)
                                };
                                let rim_tint = Color32::from_rgba_unmultiplied(
                                    rim.r(),
                                    rim.g(),
                                    rim.b(),
                                    (strength * 0.22 * 255.0) as u8,
                                );
                                painter.image(
                                    art.mask.id(),
                                    Rect::from_center_size(center, Vec2::splat(size * 1.18)),
                                    uv,
                                    rim_tint,
                                );
                                let tint = Color32::from_rgba_unmultiplied(
                                    glow.r(),
                                    glow.g(),
                                    glow.b(),
                                    (strength * 255.0) as u8,
                                );
                                painter.image(
                                    art.mask.id(),
                                    Rect::from_center_size(center, Vec2::splat(size * 1.12)),
                                    uv,
                                    tint,
                                );
                            } else {
                                art::draw_piece_glow(
                                    painter,
                                    center,
                                    size,
                                    piece.kind,
                                    piece.side,
                                    false,
                                    glow,
                                    strength,
                                );
                            }
                        } else {
                            art::draw_piece_glow(
                                painter,
                                center,
                                size,
                                piece.kind,
                                piece.side,
                                true,
                                glow,
                                strength,
                            );
                        }
                    }
                    if g.trans > 0.0 {
                        let dn = (center - layout.board.center()).length() / (board_diag * 0.5);
                        let local = smooth(((ring - dn) / 0.14).clamp(0.0, 1.0));
                        let near = (1.0 - ((ring - dn).abs() / 0.14)).clamp(0.0, 1.0);
                        let c = center + Vec2::new(0.0, -layout.cell * 0.05 * near);
                        let old_a = 1.0 - local;
                        let new_a = local;
                        let old_scale = 1.0 - 0.10 * local;
                        let new_scale = 0.90 + 0.10 * local;
                        glow_dot(
                            painter,
                            c,
                            layout.cell * 0.30,
                            Color32::from_rgb(255, 238, 206),
                            0.55 * near,
                        );
                        if near > 0.35 {
                            for k in 0..3u32 {
                                let ang = 1.7 * k as f32 + 0.9;
                                let rad = layout.cell * (0.18 + 0.10 * hash(k as f32 + sq.0 as f32));
                                sparkle(
                                    painter,
                                    c + Vec2::angled(ang) * rad,
                                    2.2,
                                    Color32::from_rgb(255, 226, 178),
                                    near * 0.6,
                                );
                            }
                        }
                        self.paint_piece(
                            painter,
                            c,
                            size * old_scale,
                            piece.kind,
                            piece.is_damka(),
                            piece.side,
                            &pal,
                            old_a,
                            phase == Phase::Checkers,
                        );
                        self.paint_piece(
                            painter,
                            c,
                            size * new_scale,
                            piece.kind,
                            piece.is_damka(),
                            piece.side,
                            &next,
                            new_a,
                            phase.other() == Phase::Checkers,
                        );
                    } else {
                        self.paint_piece(
                            painter,
                            center,
                            size,
                            piece.kind,
                            piece.is_damka(),
                            piece.side,
                            &pal,
                            1.0,
                            phase == Phase::Checkers,
                        );
                    }

                    if let Some((csq, t)) = g.crowned_flash {
                        if csq == sq {
                            let rr = layout.cell * (0.5 + (1.0 - t) * 0.7);
                            painter.circle_stroke(
                                center,
                                rr,
                                Stroke::new(2.0_f32, alpha(pal.accent, t * 0.8)),
                            );
                        }
                    }
                }
            }
        }

        // Срубленная фигура плавно тает. В шахматах клетка взятия совпадает
        // с клеткой назначения, поэтому призрак рисуем поверх фигур:
        // атакующий постепенно проявляется сквозь исчезающего противника.
        if let (Some((cap, victim)), Some((_, _, t))) = (g.last_capture, anim) {
            let center = layout.square_rect(cap).center();
            let ghost = (1.0 - t).powf(0.65);
            glow_dot(
                painter,
                center,
                layout.cell * 0.32,
                pal.danger,
                0.30 * ghost,
            );
            self.paint_piece(
                painter,
                center,
                layout.cell * 1.08 * (1.0 - 0.25 * t),
                victim.kind,
                victim.is_damka(),
                victim.side,
                &pal,
                ghost * 0.9,
                g.phase == Phase::Checkers,
            );
        }

        // Обязательное взятие: красная пульсация на шашке, которая должна бить.
        let forced = g.forced_capture_squares();
        if !forced.is_empty() {
            let pulse = 0.5 + 0.5 * (self.time * 4.6).sin();
            let red = Color32::from_rgb(255, 76, 76);
            for sq in forced {
                let center = layout.square_rect(sq).center();
                painter.circle_filled(
                    center,
                    layout.cell * 0.50,
                    alpha(red, 0.035 + 0.045 * pulse),
                );
                painter.circle_stroke(
                    center,
                    layout.cell * 0.47,
                    Stroke::new(3.0_f32, alpha(red, 0.45 + 0.42 * pulse)),
                );
                painter.circle_stroke(
                    center,
                    layout.cell * 0.38,
                    Stroke::new(1.5_f32, alpha(red, 0.30 + 0.35 * pulse)),
                );
            }
        }

        // Компьютер выбрал ход: 2–3 секунды показываем, какую фигуру он берёт.
        if let Some(mv) = self.ai_preview {
            if self.mode == Mode::Computer && g.turn == Side::Black && !g.is_over() {
                let from_rect = layout.square_rect(mv.from);
                let to_rect = layout.square_rect(mv.to);
                painter.rect_filled(from_rect.shrink(1.0), 5.0, alpha(pal.accent, 0.18));
                painter.rect_filled(to_rect.shrink(1.0), 5.0, alpha(pal.accent, 0.14));
                if let Some(cap) = mv.captured {
                    let rect = layout.square_rect(cap);
                    let pulse = 0.5 + 0.5 * (self.time * 5.0).sin();
                    painter.line_segment(
                        [from_rect.center(), rect.center()],
                        Stroke::new(2.0_f32, alpha(pal.danger, 0.30 + 0.25 * pulse)),
                    );
                    painter.circle_stroke(
                        rect.center(),
                        layout.cell * 0.46,
                        Stroke::new(2.0_f32, alpha(pal.danger, 0.35 + 0.35 * pulse)),
                    );
                    painter.circle_stroke(
                        rect.center(),
                        layout.cell * 0.38,
                        Stroke::new(1.5_f32, alpha(pal.danger, 0.25 + 0.30 * pulse)),
                    );
                }
            }
        }

        // Финальная демонстрация мата: красные линии атак и жёлтые линии
        // окружения включаются независимо.
        if (self.mate_lines || self.mate_escape) && g.is_over() && g.phase == Phase::Chess {
            if let game::Status::Win { side, reason } = &g.status {
                if reason.contains("мат") {
                    if let Some(king) = g.king_sq(side.other()) {
                        let kc = layout.square_rect(king).center();
                        let attackers = g.attackers_to(king, *side);
                        let pulse = 0.5 + 0.5 * (self.time * 3.2).sin();
                        if self.mate_lines {
                        painter.circle_stroke(
                            kc,
                            layout.cell * (0.48 + 0.04 * pulse),
                            Stroke::new(2.5_f32, alpha(pal.danger, 0.65)),
                        );
                        painter.circle_stroke(
                            kc,
                            layout.cell * 0.36,
                            Stroke::new(1.5_f32, alpha(pal.danger, 0.45)),
                        );
                        for (from, kind) in &attackers {
                            let from = *from;
                            let kind = *kind;
                            let fc = layout.square_rect(from).center();
                            let path: Vec<Pos2> = if kind == Kind::Knight {
                                let corner = if (from.0 as i32 - king.0 as i32).abs() == 1 {
                                    (king.0, from.1)
                                } else {
                                    (from.0, king.1)
                                };
                                vec![fc, layout.square_rect(corner).center(), kc]
                            } else {
                                vec![fc, kc]
                            };
                            for pair in path.windows(2) {
                                painter.line_segment(
                                    [pair[0], pair[1]],
                                    Stroke::new(7.0_f32, Color32::from_black_alpha(80)),
                                );
                                painter.line_segment(
                                    [pair[0], pair[1]],
                                    Stroke::new(3.0_f32, alpha(pal.danger, 0.30 + 0.25 * pulse)),
                                );
                                painter.line_segment(
                                    [pair[0], pair[1]],
                                    Stroke::new(1.2_f32, alpha(Color32::WHITE, 0.55)),
                                );
                            }
                            painter.circle_stroke(
                                fc,
                                layout.cell * 0.38,
                                Stroke::new(2.0_f32, alpha(pal.danger, 0.75)),
                            );
                            let dir = (kc - fc).normalized();
                            let perp = Vec2::new(-dir.y, dir.x);
                            let tip = kc - dir * layout.cell * 0.34;
                            let base = tip - dir * layout.cell * 0.26;
                            painter.add(egui::Shape::convex_polygon(
                                vec![
                                    tip,
                                    base + perp * layout.cell * 0.16,
                                    base - perp * layout.cell * 0.16,
                                ],
                                alpha(pal.danger, 0.85),
                                Stroke::NONE,
                            ));
                        }
                        }

                        // Жёлтые линии: другие фигуры победившей стороны держат
                        // клетки вокруг короля — видно, что ему некуда идти.
                        if self.mate_escape {
                        let yellow = Color32::from_rgb(240, 205, 100);
                        let mut escape_lines: Vec<(Square, Square)> = Vec::new();
                        for dx in -1i32..=1 {
                            for dy in -1i32..=1 {
                                if dx == 0 && dy == 0 {
                                    continue;
                                }
                                let nx = king.0 as i32 + dx;
                                let ny = king.1 as i32 + dy;
                                if !(0..8).contains(&nx) || !(0..8).contains(&ny) {
                                    continue;
                                }
                                let sq = (nx as usize, ny as usize);
                                if g.piece_at(sq).is_some() {
                                    continue;
                                }
                                for (from, _) in g.attackers_to(sq, *side) {
                                    if from == king {
                                        continue;
                                    }
                                    if attackers.iter().any(|(f, _)| *f == from) {
                                        continue;
                                    }
                                    escape_lines.push((from, sq));
                                }
                            }
                        }
                        for (from, sq) in escape_lines {
                            let fc = layout.square_rect(from).center();
                            let sc = layout.square_rect(sq).center();
                            painter.line_segment(
                                [fc, sc],
                                Stroke::new(6.0_f32, Color32::from_black_alpha(70)),
                            );
                            painter.line_segment(
                                [fc, sc],
                                Stroke::new(2.4_f32, alpha(yellow, 0.55 + 0.25 * pulse)),
                            );
                            painter.circle_stroke(
                                sc,
                                layout.cell * 0.20,
                                Stroke::new(1.6_f32, alpha(yellow, 0.70)),
                            );
                        }
                        }
                    }
                }
            }
        }

        // подсказки ходов
        for mv in &g.legal {
            let rect = layout.square_rect(mv.to);
            if mv.captured.is_some() {
                painter.rect_filled(rect.shrink(1.0), 5.0, alpha(pal.danger, 0.16));
                painter.rect_stroke(
                    rect.shrink(4.0),
                    5.0,
                    Stroke::new(2.0_f32, alpha(pal.danger, 0.8)),
                    egui::StrokeKind::Inside,
                );
            } else {
                let c = rect.center();
                let r = layout.cell * 0.10;
                painter.circle_filled(c, r, alpha(pal.accent, 0.55));
                painter.circle_stroke(c, r, Stroke::new(1.0_f32, alpha(pal.accent, 0.8)));
            }
        }

        // взятие в цепочке: подсветить фигуру, которая бьёт
        if let Some(chain) = g.chain {
            let rect = layout.square_rect(chain);
            let t = 0.5 + 0.5 * (self.time * 5.0).sin();
            painter.rect_stroke(
                rect.shrink(2.0),
                5.0,
                Stroke::new(2.5_f32, alpha(pal.accent, 0.6 + 0.4 * t)),
                egui::StrokeKind::Inside,
            );
        }

        // шах
        if let Some(k) = g.check_square() {
            let rect = layout.square_rect(k);
            let t = 0.5 + 0.5 * (self.time * 4.0).sin();
            painter.rect_filled(rect.shrink(1.0), 5.0, alpha(pal.danger, 0.14 + 0.10 * t));
            painter.rect_stroke(
                rect.shrink(2.0),
                5.0,
                Stroke::new(2.0_f32, alpha(pal.danger, 0.55 + 0.35 * t)),
                egui::StrokeKind::Inside,
            );
        }

        // пауза
        if g.paused && !g.is_over() {
            painter.rect_filled(layout.board, 10.0, Color32::from_black_alpha(130));
            let pill = Rect::from_center_size(layout.board.center(), Vec2::new(470.0, 104.0));
            rr(
                painter,
                pill,
                18.0,
                Color32::from_rgb(23, 21, 33),
                Stroke::new(1.0_f32, alpha(Color32::WHITE, 0.12)),
            );
            let title = "ПАУЗА";
            let tw = art::spaced_width(painter, title, 22.0, 6.0);
            spaced_text(
                painter,
                Pos2::new(pill.center().x - tw * 0.5, pill.top() + 24.0),
                title,
                22.0,
                pal.text,
                6.0,
            );
            text(
                painter,
                Pos2::new(pill.center().x, pill.top() + 64.0),
                Align2::CENTER_CENTER,
                "Space (клавиша пробел) — продолжить партию",
                12.5,
                alpha(pal.text_dim, 0.95),
            );
        }

        // сумеречный переход: волна от центра
        if g.trans > 0.0 {
            let t = g.trans;
            let center = layout.board.center();
            let ring_r = (m * 1.25 * (board_diag * 0.5)).min(board_diag * 0.54);
            let a = bell(t);
            let warm = Color32::from_rgb(248, 226, 186);
            let clip = painter.with_clip_rect(layout.board);

            for i in 0..3 {
                let rr = (ring_r - i as f32 * 9.0).max(6.0);
                clip.circle_stroke(
                    center,
                    rr,
                    Stroke::new(
                        2.2 - i as f32 * 0.5,
                        alpha(warm, a * (0.55 - i as f32 * 0.13)),
                    ),
                );
            }
            clip.circle_stroke(
                center,
                ring_r,
                Stroke::new(12.0_f32, alpha(warm, a * 0.10)),
            );

            for i in 0..30u32 {
                let ang = i as f32 * 0.2417 + t * 0.8;
                let rad = ring_r * (0.94 + 0.09 * hash(i as f32 * 3.1));
                let p = center + Vec2::angled(ang) * rad;
                if layout.board.contains(p) {
                    sparkle(&clip, p, 2.0 + hash(i as f32) * 2.5, warm, a * 0.8);
                }
            }

            let veil = alpha(Color32::from_rgb(255, 232, 196), 0.03 * a);
            painter.rect_filled(layout.board, 6.0, veil);
        }

        // подпись под доской не нужна — всё в шапке
        let _ = painter;
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_piece(
        &self,
        painter: &egui::Painter,
        center: Pos2,
        size: f32,
        kind: Kind,
        damka: bool,
        side: Side,
        pal: &Palette,
        a: f32,
        checkers_form: bool,
    ) {
        if a <= 0.01 {
            return;
        }
        if !checkers_form {
            if let Some(art) = self.piece_tex.get(side, kind) {
                let rect = Rect::from_center_size(center, Vec2::splat(size));
                let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
                painter.image(art.color.id(), rect, uv, alpha(Color32::WHITE, a));
                return;
            }
        }
        if checkers_form {
            let show_icon = self.show_checker_icons && kind != Kind::Pawn;
            let royal = kind == Kind::King && !show_icon;
            if let Some(art) = self.piece_tex.checker_art(side, damka, royal) {
                let rect = Rect::from_center_size(center, Vec2::splat(size));
                let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
                painter.image(art.color.id(), rect, uv, alpha(Color32::WHITE, a));
                if damka && !self.piece_tex.has_damka(side) {
                    // Своей картинки дамки нет — дорисуем корону поверх шашки.
                    art::draw_damka_crown(painter, center, size, side, a);
                }
                if royal && !self.piece_tex.has_royal(side) {
                    // Своей картинки пешки-короля нет — дорисуем точку.
                    art::draw_royal_dot(painter, center, size, side, a);
                }
                if show_icon {
                    art::draw_checker_icon(painter, center, size, kind, side, a);
                }
                return;
            }
            draw_checker(
                painter,
                center,
                size,
                damka,
                royal,
                side,
                pal,
                a,
            );
            if show_icon {
                art::draw_checker_icon(painter, center, size, kind, side, a);
            }
        } else {
            draw_chess_piece(painter, center, size, kind, side, pal, a);
        }
    }

    // ------------------------- панель -------------------------

    fn paint_start_frost(&self, painter: &egui::Painter, layout: &Layout) {
        let b = layout.board;
        let frost = painter.with_clip_rect(b);
        // Сильное размытие: плотная матовая вуаль в два слоя.
        frost.rect_filled(
            b,
            10.0,
            Color32::from_rgba_unmultiplied(8, 10, 18, 208),
        );
        frost.rect_filled(
            b,
            10.0,
            Color32::from_rgba_unmultiplied(16, 18, 30, 34),
        );
        radial(
            &frost,
            b.center() + Vec2::new(-b.width() * 0.18, -b.height() * 0.10),
            b.width() * 0.55,
            Color32::from_rgb(178, 194, 235),
            0.05,
        );
        radial(
            &frost,
            b.center() + Vec2::new(b.width() * 0.22, b.height() * 0.18),
            b.width() * 0.45,
            Color32::from_rgb(255, 220, 190),
            0.035,
        );
        // Матовая вуаль: только мягкое размытие поля, без полос и точек.
        painter.rect_stroke(
            b,
            10.0,
            Stroke::new(1.0_f32, Color32::from_white_alpha(28)),
            egui::StrokeKind::Inside,
        );
        let label = Rect::from_center_size(b.center(), Vec2::new(520.0, 108.0));
        painter.rect_filled(
            label.translate(Vec2::new(0.0, 10.0)),
            20.0,
            Color32::from_black_alpha(120),
        );
        rr(
            painter,
            label,
            20.0,
            Color32::from_rgba_unmultiplied(6, 7, 12, 234),
            Stroke::new(1.0_f32, Color32::from_white_alpha(58)),
        );
        painter.text(
            label.center() + Vec2::new(0.0, -14.0),
            Align2::CENTER_CENTER,
            i18n::tr("ПАРТИЯ ЕЩЁ НЕ НАЧАТА"),
            FontId::proportional(21.0),
            Color32::from_rgb(246, 243, 252),
        );
        painter.text(
            label.center() + Vec2::new(0.0, 20.0),
            Align2::CENTER_CENTER,
            i18n::tr("Справа — «Играть», настройки и правила"),
            FontId::proportional(11.5),
            Color32::from_rgb(210, 206, 226),
        );
    }

    fn paint_panel(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, layout: &Layout) {
        let g = &self.game;
        let forced_now = g.forced_capture_squares();
        let mut end_chain_clicked = false;
        let pal = Palette::for_phase(g.phase);
        let next = Palette::for_phase(g.phase.other());
        let m = smooth(g.trans);
        let blended = blend(pal, next, m);
        let accent = if g.trans > 0.55 {
            next.accent
        } else {
            pal.accent
        };
        let cur = Palette { accent, ..blended };
        let rect = layout.panel;

        rr(
            painter,
            rect,
            24.0,
            alpha(Color32::WHITE, 0.045),
            Stroke::new(1.0_f32, alpha(Color32::WHITE, 0.075)),
        );
        // тонкая подсветка сверху
        painter.rect_filled(
            Rect::from_min_size(rect.min + Vec2::new(18.0, 0.0), Vec2::new(rect.width() - 36.0, 1.0)),
            0.0,
            alpha(cur.accent, 0.35),
        );

        let p = rect.shrink(22.0);
        let x = p.left();
        let w = p.width();
        let mut y = p.top();

        // Версия — в правом верхнем углу панели, без строки «прототип».
        text(
            painter,
            Pos2::new(p.right(), p.top()),
            Align2::RIGHT_TOP,
            format!("v{VERSION}"),
            9.5,
            alpha(cur.text_dim, 0.9),
        );

        let display_phase = if g.trans > 0.55 { g.phase.other() } else { g.phase };
        let display_pal = Palette::for_phase(display_phase);
        spaced_text(
            painter,
            Pos2::new(x, y),
            display_phase.label_ru(),
            23.0,
            display_pal.accent,
            2.2,
        );
        y += 34.0;

        let countdown_text = if g.trans > 0.0 {
            "···".to_string()
        } else if g.twilight_by_moves {
            let secs = g.move_left.max(0.0).ceil() as i32;
            format!("{}:{:02}", secs / 60, secs % 60)
        } else {
            let secs = g.phase_left.max(0.0).ceil() as i32;
            format!("{}:{:02}", secs / 60, secs % 60)
        };
        let urgent = g.trans == 0.0
            && !g.is_over()
            && !g.paused
            && if g.twilight_by_moves {
                g.move_left <= 5.0
            } else {
                g.phase_left <= 5.0
            };
        let countdown_color = if urgent {
            mix(
                cur.text,
                cur.danger,
                0.45 + 0.45 * (self.time * 5.0).sin(),
            )
        } else {
            cur.text
        };
        text(
            painter,
            Pos2::new(x, y),
            Align2::LEFT_TOP,
            countdown_text,
            if g.twilight_by_moves { 36.0 } else { 44.0 },
            countdown_color,
        );
        text(
            painter,
            Pos2::new(x + 4.0, y + 50.0),
            Align2::LEFT_TOP,
            if g.trans > 0.0 {
                "сумерки: фигуры меняются".to_string()
            } else if g.paused {
                "пауза".to_string()
            } else if g.twilight_by_moves {
                // Обратный отсчёт уже показан крупным таймером выше —
                // подпись только называет событие, без дублирования времени.
                i18n::tr("ход перейдёт противнику").to_string()
            } else if g.twilight_pending() {
                "переход ждёт конца взятия".to_string()
            } else if g.passes >= 2 {
                "нет ходов — сумерки выводят из тупика".to_string()
            } else if self.mode == Mode::Computer && g.turn == Side::Black {
                "компьютер думает…".to_string()
            } else {
                i18n::trf(
                    "до превращения в {phase}",
                    &[("phase", g.phase.other().one_ru())],
                )
            },
            11.5,
            alpha(cur.text_dim, 0.95),
        );
        y += 72.0;

        let frac = if g.trans > 0.0 {
            1.0 - g.trans
        } else if g.twilight_by_moves {
            (g.move_left / g.move_time_limit.max(0.1)).clamp(0.0, 1.0)
        } else {
            (g.phase_left / g.phase_total.max(0.1)).clamp(0.0, 1.0)
        };
        let bar = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, 4.0));
        painter.rect_filled(bar, 2.0, alpha(Color32::WHITE, 0.07));
        let fill_w = (w * frac).max(2.0);
        painter.rect_filled(
            Rect::from_min_size(bar.min, Vec2::new(fill_w, 4.0)),
            2.0,
            cur.accent,
        );
        y += 18.0;

        // предпросмотр фаз
        let pill_h = 22.0;
        let ph_half = (w - 8.0) * 0.5;
        for (i, ph) in [Phase::Chess, Phase::Checkers].iter().enumerate() {
            let pill = Rect::from_min_size(
                Pos2::new(x + i as f32 * (ph_half + 8.0), y),
                Vec2::new(ph_half, pill_h),
            );
            let active = display_phase == *ph;
            let col = Palette::for_phase(*ph).accent;
            rr(
                painter,
                pill,
                7.0,
                alpha(col, if active { 0.16 } else { 0.04 }),
                Stroke::new(1.0_f32, alpha(col, if active { 0.55 } else { 0.12 })),
            );
            text(
                painter,
                pill.center(),
                Align2::CENTER_CENTER,
                ph.one_ru(),
                10.5,
                if active {
                    col
                } else {
                    alpha(cur.text_dim, 0.85)
                },
            );
        }
        y += pill_h + 12.0;

        // соперник
        text(
            painter,
            Pos2::new(x, y + 5.0),
            Align2::LEFT_TOP,
            "СОПЕРНИК",
            8.5,
            alpha(cur.text_dim, 0.75),
        );
        let label_w = 66.0;
        let chip_h = 24.0;
        let chips_x = x + label_w;
        let chips_w = w - label_w;
        let chip_half = (chips_w - 8.0) * 0.5;
        let hot = Rect::from_min_size(Pos2::new(chips_x, y), Vec2::new(chip_half, chip_h));
        let comp = Rect::from_min_size(
            Pos2::new(chips_x + chip_half + 8.0, y),
            Vec2::new(chip_half, chip_h),
        );
        if chip(ui, hot, "вдвоём", self.mode == Mode::Hotseat, &cur) {
            self.mode = Mode::Hotseat;
        }
        if chip(ui, comp, "с компьютером", self.mode == Mode::Computer, &cur) {
            self.mode = Mode::Computer;
            self.ai_wait = 0.5;
        }
        y += chip_h + 8.0;
        if self.mode == Mode::Computer {
            text(
                painter,
                Pos2::new(x, y + 5.0),
                Align2::LEFT_TOP,
                "СЛОЖНОСТЬ",
                8.5,
                alpha(cur.text_dim, 0.75),
            );
            let cw = (chips_w - 16.0) / 3.0;
            for (i, d) in ALL_DIFFICULTIES.iter().enumerate() {
                let r = Rect::from_min_size(
                    Pos2::new(chips_x + i as f32 * (cw + 8.0), y),
                    Vec2::new(cw, chip_h),
                );
                if chip(ui, r, d.label_ru(), self.difficulty == *d, &cur) {
                    self.difficulty = *d;
                }
            }
            y += chip_h + 8.0;
        }

        divider(painter, x, y, w, cur.text_dim);
        y += 20.0;

        // ход
        let dot = Pos2::new(x + 5.0, y + 9.0);
        let side_color = match g.turn {
            Side::White => Color32::from_rgb(244, 232, 210),
            Side::Black => Color32::from_rgb(70, 82, 110),
        };
        painter.circle_filled(dot, 5.0, side_color);
        painter.circle_stroke(dot, 5.0, Stroke::new(1.0_f32, alpha(cur.text_dim, 0.6)));
        text(
            painter,
            Pos2::new(x + 18.0, y),
            Align2::LEFT_TOP,
            i18n::trf("Ход {side}", &[("side", g.turn.gen_ru())]),
            19.0,
            cur.text,
        );
        if let game::Status::Check(_) = &g.status {
            badge(painter, Pos2::new(x + w, y - 2.0), "ШАХ", cur.danger, true, 11.0);
        } else if g.chain.is_some() {
            if !g.mandatory_capture && !self.settings && !self.rules {
                let r = Rect::from_min_size(
                    Pos2::new(x + w - 120.0, y - 2.0),
                    Vec2::new(120.0, 28.0),
                );
                if button(ui, r, "Закончить", false, &cur, 10.5) {
                    end_chain_clicked = true;
                }
            } else {
                badge(painter, Pos2::new(x + w, y - 2.0), "ВЗЯТИЕ", cur.accent, true, 11.0);
            }
        } else if !forced_now.is_empty() {
            badge(painter, Pos2::new(x + w, y - 2.0), "БОЙ", cur.danger, true, 11.0);
        }
        y += 34.0;

        // материал
        let diff = g.material_diff();
        let material = match diff {
            0 => i18n::tr("материал равный").to_string(),
            d if d > 0 => i18n::trf("белые впереди на {n}", &[("n", &d.to_string())]),
            d => i18n::trf("чёрные впереди на {n}", &[("n", &(-d).to_string())]),
        };
        text(
            painter,
            Pos2::new(x, y),
            Align2::LEFT_TOP,
            i18n::trf(
                "{material} · взято {white} : {black}",
                &[
                    ("material", &material),
                    ("white", &g.captured_by_white.to_string()),
                    ("black", &g.captured_by_black.to_string()),
                ],
            ),
            11.5,
            alpha(cur.text_dim, 0.95),
        );
        y += 24.0;

        divider(painter, x, y, w, cur.text_dim);
        y += 16.0;

        text(
            painter,
            Pos2::new(x, y),
            Align2::LEFT_TOP,
            "ХОДЫ",
            9.5,
            alpha(cur.text_dim, 0.85),
        );
        y += 18.0;

        // нижние элементы: Играть/Пауза (с заливкой) и Новая партия рядом.
        let gap = 8.0;
        let half = (w - gap) * 0.5;
        let bottom_y = p.bottom() - 46.0;
        let play_btn = Rect::from_min_size(Pos2::new(x, bottom_y), Vec2::new(half, 46.0));
        let new_btn = Rect::from_min_size(
            Pos2::new(x + half + gap, bottom_y),
            Vec2::new(half, 46.0),
        );
        let row_y = bottom_y - 10.0 - 36.0;
        let rules_btn = Rect::from_min_size(Pos2::new(x, row_y), Vec2::new(half, 36.0));
        let settings_btn = Rect::from_min_size(
            Pos2::new(x + half + gap, row_y),
            Vec2::new(half, 36.0),
        );
        let chips_y = row_y - 30.0 - 30.0;
        let chips = [
            (10.0, "10 с"),
            (15.0, "15 с"),
            (20.0, "20 с"),
            (40.0, "40 с"),
        ];
        let chip_w = (w - 24.0) / 4.0;
        let mut chip_rects = Vec::new();
        for (i, _) in chips.iter().enumerate() {
            chip_rects.push(Rect::from_min_size(
                Pos2::new(x + i as f32 * (chip_w + 8.0), chips_y),
                Vec2::new(chip_w, 30.0),
            ));
        }
        // журнал ходов (последние строки)
        let log_top = y;
        let log_bottom = chips_y - 46.0;
        let line_h = 17.0;
        let max_lines = (((log_bottom - log_top) / line_h).floor() as usize).max(0);
        if max_lines > 0 && !g.log.is_empty() {
            let start = g.log.len().saturating_sub(max_lines);
            let mut ly = log_top;
            for (idx, line) in g.log[start..].iter().enumerate() {
                let is_last = start + idx + 1 == g.log.len();
                text(
                    painter,
                    Pos2::new(x, ly),
                    Align2::LEFT_TOP,
                    line,
                    12.5,
                    if is_last {
                        alpha(cur.text, 0.95)
                    } else {
                        alpha(cur.text_dim, 0.8)
                    },
                );
                ly += line_h;
            }
        } else if max_lines > 0 {
            text(
                painter,
                Pos2::new(x, log_top),
                Align2::LEFT_TOP,
                "пока тихо — сделайте первый ход",
                12.5,
                alpha(cur.text_dim, 0.78),
            );
            let tip = "Совет: в шашечной фазе король превращается в дамку — съешьте его и победите.";
            let galley = painter.layout(
                tip.to_string(),
                FontId::proportional(11.5),
                alpha(cur.text_dim, 0.65),
                w,
            );
            painter.galley(Pos2::new(x, log_top + 30.0), galley, Color32::WHITE);
        }

        // Предварительные настройки игры: компактная карточка с единым
        // стилем, чтобы блок читался так же, как остальные строки панели.
        let setup_bg = Rect::from_min_size(
            Pos2::new(x - 10.0, chips_y - 24.0),
            Vec2::new(w + 20.0, 74.0),
        );
        rr(
            painter,
            setup_bg,
            12.0,
            alpha(Color32::WHITE, 0.035),
            Stroke::new(1.0_f32, alpha(Color32::WHITE, 0.07)),
        );
        text(
            painter,
            Pos2::new(x, chips_y - 18.0),
            Align2::LEFT_TOP,
            if g.twilight_by_moves {
                "СМЕНА ФАЗ"
            } else if g.auto_phase {
                "АВТОСМЕНА СУМЕРЕК"
            } else {
                "РИТМ ТЕКУЩЕЙ ФАЗЫ"
            },
            9.5,
            alpha(cur.text_dim, 0.9),
        );
        if g.twilight_by_moves {
            text(
                painter,
                Pos2::new(x, chips_y),
                Align2::LEFT_TOP,
                "По ходам: случайно",
                12.5,
                cur.accent,
            );
            text(
                painter,
                Pos2::new(x, chips_y + 19.0),
                Align2::LEFT_TOP,
                "После хода может остаться или смениться",
                11.0,
                alpha(cur.text_dim, 0.85),
            );
        } else if g.auto_phase {
            text(
                painter,
                Pos2::new(x, chips_y),
                Align2::LEFT_TOP,
                i18n::trf("Случайно: 10–{max} с", &[("max", &format!("{:.0}", g.auto_max))]),
                12.5,
                cur.accent,
            );
            text(
                painter,
                Pos2::new(x, chips_y + 19.0),
                Align2::LEFT_TOP,
                "Следующий интервал будет другим",
                11.0,
                alpha(cur.text_dim, 0.85),
            );
        }
        text(
            painter,
            Pos2::new(x, chips_y + 35.0),
            Align2::LEFT_TOP,
            "клик — ход · Space — пауза · R — новая партия · H — правила",
            10.0,
            alpha(cur.text_dim, 0.82),
        );

        // --- взаимодействие ---
        if self.game.is_over() {
            return;
        }
        if self.settings || self.rules || self.guide {
            return;
        }
        if self.game.pending_promotion.is_some() {
            return;
        }
        if button(ui, rules_btn, "Правила", false, &cur, 12.0) {
            self.rules = true;
        }
        if button_gear(ui, settings_btn, "Настройки", false, &cur, 11.5) {
            self.settings = true;
            self.rules = false;
        }
        let pause_label = if !self.started {
            "Играть"
        } else if self.game.paused {
            "Играть"
        } else {
            "Пауза"
        };
        if button(ui, play_btn, pause_label, true, &cur, 14.5) {
            if !self.started {
                self.start_game();
            } else {
                self.game.paused = !self.game.paused;
            }
        }
        if button(ui, new_btn, "Новая партия", false, &cur, 14.5) {
            self.new_game();
        }
        if !self.game.twilight_by_moves && !self.game.auto_phase {
            for (i, (secs, label)) in chips.iter().enumerate() {
                let cur_len = self.game.len_for(self.game.phase);
                let selected = (cur_len - secs).abs() < 0.5;
                let clicked = chip(ui, chip_rects[i], label, selected, &cur);
                if clicked && self.game.trans == 0.0 && !self.game.is_over() {
                    self.game.set_len(self.game.phase, *secs);
                }
            }
        }
        if end_chain_clicked {
            self.game.end_chain();
        }
    }

    // ------------------------- настройки -------------------------

    fn paint_settings(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, layout: &Layout) {
        painter.rect_filled(layout.full, 0.0, Color32::from_black_alpha(165));
        let card = Rect::from_center_size(layout.full.center(), Vec2::new(680.0, 660.0));
        self.settings_rect = card;
        let pal = Palette::for_phase(self.game.phase);
        rr(
            painter,
            card,
            24.0,
            Color32::from_rgb(22, 20, 30),
            Stroke::new(1.0_f32, alpha(pal.accent, 0.35)),
        );
        let pad = 28.0;
        let x = card.left() + pad;
        let w = card.width() - pad * 2.0;
        let left_w = 300.0;
        let right_x = x + left_w + 24.0;
        let right_w = w - left_w - 24.0;
        let mut y = card.top() + 26.0;

        spaced_text(painter, Pos2::new(x, y), "НАСТРОЙКИ", 22.0, pal.text, 2.4);
        text(
            painter,
            Pos2::new(card.right() - pad, card.top() + 30.0),
            Align2::RIGHT_TOP,
            format!("v{VERSION}"),
            10.0,
            alpha(pal.text_dim, 0.9),
        );
        // язык интерфейса
        let n = i18n::Lang::ALL.len() as f32;
        let lw = 34.0;
        let lgap = 5.0;
        let total = n * lw + (n - 1.0) * lgap;
        let lx = card.right() - pad - total;
        for (i, lang) in i18n::Lang::ALL.iter().enumerate() {
            let r = Rect::from_min_size(
                Pos2::new(lx + i as f32 * (lw + lgap), card.top() + 48.0),
                Vec2::new(lw, 20.0),
            );
            if chip(ui, r, lang.label(), i18n::current() == *lang, &pal) {
                i18n::set_lang(*lang);
            }
        }
        text(
            painter,
            Pos2::new(lx - 8.0, card.top() + 51.0),
            Align2::RIGHT_TOP,
            "ЯЗЫК",
            8.0,
            alpha(pal.text_dim, 0.75),
        );
        // Больше воздуха под строкой языков, чтобы она не липла к параметрам.
        y += 62.0;
        let top = y;

        // левая колонка: звук и музыка
        let mut ly = top;
        text(
            painter,
            Pos2::new(x, ly),
            Align2::LEFT_TOP,
            "ЗВУКИ",
            9.5,
            alpha(pal.text_dim, 0.8),
        );
        ly += 16.0;
        let sound_on = sound::enabled();
        let sound_btn = Rect::from_min_size(Pos2::new(x, ly), Vec2::new(left_w, 36.0));
        if button(
            ui,
            sound_btn,
            if sound_on {
                "Звуки: включены"
            } else {
                "Звуки: выключены"
            },
            sound_on,
            &pal,
            13.0,
        ) {
            sound::set_enabled(!sound_on);
        }
        ly += 42.0;

        text(
            painter,
            Pos2::new(x, ly),
            Align2::LEFT_TOP,
            "ИСТОЧНИК",
            9.0,
            alpha(pal.text_dim, 0.75),
        );
        ly += 15.0;
        let chip_w = (left_w - 8.0) * 0.5;
        let builtin = Rect::from_min_size(Pos2::new(x, ly), Vec2::new(chip_w, 28.0));
        let custom = Rect::from_min_size(Pos2::new(x + chip_w + 8.0, ly), Vec2::new(chip_w, 28.0));
        if chip(ui, builtin, "Встроенные", !sound::use_files(), &pal) {
            sound::set_use_files(false);
        }
        if chip(ui, custom, "Из папки sounds", sound::use_files(), &pal) {
            sound::set_use_files(true);
        }
        ly += 36.0;

        text(
            painter,
            Pos2::new(x, ly),
            Align2::LEFT_TOP,
            "МУЗЫКА",
            9.0,
            alpha(pal.text_dim, 0.75),
        );
        ly += 15.0;
        let music_avail = music::available();
        let music_on = music::enabled();
        let music_btn = Rect::from_min_size(Pos2::new(x, ly), Vec2::new(left_w, 36.0));
        let label = if !music_avail {
            "Музыка: нет music/theme.wav"
        } else if music_on {
            "Музыка: включена"
        } else {
            "Музыка: выключена"
        };
        if button(ui, music_btn, label, music_on && music_avail, &pal, 12.5) && music_avail {
            music::set_enabled(!music_on);
        }
        ly += 40.0;
        text(
            painter,
            Pos2::new(x, ly),
            Align2::LEFT_TOP,
            "Фоновая музыка не прерывается звуками ходов.",
            10.0,
            alpha(pal.text_dim, 0.8),
        );
        ly += 26.0;

        text(
            painter,
            Pos2::new(x, ly),
            Align2::LEFT_TOP,
            "ЛОГИ ПАРТИЙ",
            9.0,
            alpha(pal.text_dim, 0.75),
        );
        ly += 15.0;
        let log_on = self.log_enabled;
        let log_btn = Rect::from_min_size(Pos2::new(x, ly), Vec2::new(left_w, 36.0));
        if button(
            ui,
            log_btn,
            if log_on {
                "Логи: включены"
            } else {
                "Логи: выключены"
            },
            log_on,
            &pal,
            12.5,
        ) {
            self.log_enabled = !log_on;
            if self.log_enabled && self.logging_allowed && self.game_start.is_none() {
                self.game_start = Some(logging::now_string());
                self.log_written = false;
            }
        }
        ly += 38.0;
        text(
            painter,
            Pos2::new(x, ly),
            Align2::LEFT_TOP,
            "Файлы — в папке logs рядом с игрой.",
            10.0,
            alpha(pal.text_dim, 0.8),
        );
        ly += 24.0;

        text(
            painter,
            Pos2::new(x, ly),
            Align2::LEFT_TOP,
            "ШРИФТ",
            9.0,
            alpha(pal.text_dim, 0.75),
        );
        ly += 15.0;
        let mut font_name = self
            .font_faces
            .get(self.font_index)
            .map(|f| f.name.clone())
            .unwrap_or_else(|| "Prosto".to_string());
        if font_name.chars().count() > 28 {
            font_name = font_name.chars().take(27).collect::<String>() + "…";
        }
        let font_btn = Rect::from_min_size(Pos2::new(x, ly), Vec2::new(left_w, 34.0));
        let font_label = i18n::trf("Шрифт: {name}", &[("name", &font_name)]);
        if button(
            ui,
            font_btn,
            &font_label,
            false,
            &pal,
            11.5,
        ) {
            if !self.font_faces.is_empty() {
                self.font_index = (self.font_index + 1) % self.font_faces.len();
                fonts::apply(ui.ctx(), &self.font_faces, self.font_index);
            }
        }
        ly += 40.0;
        text(
            painter,
            Pos2::new(x, ly),
            Align2::LEFT_TOP,
            "Свои .ttf/.otf — в папке fonts рядом с игрой.",
            9.0,
            alpha(pal.text_dim, 0.75),
        );
        ly += 24.0;

        // правая колонка: превращение, время фаз, плавность
        let mut ry = top;
        text(
            painter,
            Pos2::new(right_x, ry),
            Align2::LEFT_TOP,
            "ПРЕВРАЩЕНИЕ",
            9.0,
            alpha(pal.text_dim, 0.75),
        );
        ry += 15.0;
        let promo_w = (right_w - 8.0) * 0.5;
        let always = Rect::from_min_size(Pos2::new(right_x, ry), Vec2::new(promo_w, 28.0));
        let choose = Rect::from_min_size(
            Pos2::new(right_x + promo_w + 8.0, ry),
            Vec2::new(promo_w, 28.0),
        );
        if chip(ui, always, "Всегда ферзь", !self.game.promotion_choice, &pal) {
            self.game.promotion_choice = false;
        }
        if chip(ui, choose, "Выбор фигуры", self.game.promotion_choice, &pal) {
            self.game.promotion_choice = true;
        }
        ry += 36.0;

        text(
            painter,
            Pos2::new(right_x, ry),
            Align2::LEFT_TOP,
            "СМЕНА ФАЗ",
            9.0,
            alpha(pal.text_dim, 0.75),
        );
        ry += 15.0;
        let mode_w = (right_w - 8.0) * 0.5;
        let by_time_chip = Rect::from_min_size(Pos2::new(right_x, ry), Vec2::new(mode_w, 28.0));
        let by_moves_chip = Rect::from_min_size(
            Pos2::new(right_x + mode_w + 8.0, ry),
            Vec2::new(mode_w, 28.0),
        );
        if chip(ui, by_time_chip, "По времени", !self.game.twilight_by_moves, &pal) {
            self.game.set_twilight_by_moves(false);
        }
        if chip(ui, by_moves_chip, "По ходам", self.game.twilight_by_moves, &pal) {
            self.game.set_twilight_by_moves(true);
        }
        ry += 36.0;

        if self.game.twilight_by_moves {
            text(
                painter,
                Pos2::new(right_x, ry),
                Align2::LEFT_TOP,
                "После хода фаза может остаться или смениться случайно.",
                9.0,
                alpha(pal.text, 0.85),
            );
            ry += 14.0;
            text(
                painter,
                Pos2::new(right_x, ry),
                Align2::LEFT_TOP,
                "Внутри одного хода всегда одна игра.",
                9.0,
                alpha(pal.text_dim, 0.75),
            );
            ry += 22.0;
            text(
                painter,
                Pos2::new(right_x, ry),
                Align2::LEFT_TOP,
                "ВРЕМЯ ХОДА",
                9.0,
                alpha(pal.text_dim, 0.75),
            );
            ry += 15.0;
            if let Some(secs) = time_slider(
                ui,
                painter,
                right_x,
                ry,
                right_w,
                self.game.move_time_limit,
                &pal,
                "move_time",
                game::MIN_MOVE_SECS,
                game::MAX_MOVE_SECS,
            ) {
                self.game.set_move_time_limit(secs);
            }
            ry += 30.0;
            text(
                painter,
                Pos2::new(right_x, ry),
                Align2::LEFT_TOP,
                "По истечении ход переходит противнику.",
                9.0,
                alpha(pal.text_dim, 0.75),
            );
            ry += 24.0;
        } else {
            text(
                painter,
                Pos2::new(right_x, ry),
                Align2::LEFT_TOP,
                "АВТОСМЕНА СУМЕРЕК",
                9.0,
                alpha(pal.text_dim, 0.75),
            );
            ry += 15.0;
            let auto_w = (right_w - 8.0) * 0.5;
            let random_chip = Rect::from_min_size(Pos2::new(right_x, ry), Vec2::new(auto_w, 28.0));
            let fixed_chip = Rect::from_min_size(
                Pos2::new(right_x + auto_w + 8.0, ry),
                Vec2::new(auto_w, 28.0),
            );
            if chip(ui, random_chip, "Случайно", self.game.auto_phase, &pal) {
                self.game.set_auto(true);
            }
            if chip(ui, fixed_chip, "Фиксированно", !self.game.auto_phase, &pal) {
                self.game.set_auto(false);
            }
            ry += 36.0;

            if self.game.auto_phase {
                text(
                    painter,
                    Pos2::new(right_x, ry),
                    Align2::LEFT_TOP,
                    "МАКСИМУМ ИНТЕРВАЛА",
                    9.0,
                    alpha(pal.text_dim, 0.75),
                );
                ry += 15.0;
                if let Some(secs) = time_slider(
                    ui,
                    painter,
                    right_x,
                    ry,
                    right_w,
                    self.game.auto_max,
                    &pal,
                    "auto_max",
                    game::MIN_AUTO_SECS,
                    game::MAX_PHASE,
                ) {
                    self.game.set_auto_max(secs);
                }
                ry += 28.0;
                text(
                    painter,
                    Pos2::new(right_x, ry),
                    Align2::LEFT_TOP,
                    "Каждая смена случайна; следующий интервал отличается.",
                    9.0,
                    alpha(pal.text_dim, 0.75),
                );
                ry += 24.0;
            } else {
                text(
                    painter,
                    Pos2::new(right_x, ry),
                    Align2::LEFT_TOP,
                    "ВРЕМЯ · ШАХМАТЫ",
                    9.0,
                    alpha(pal.text_dim, 0.75),
                );
                ry += 15.0;
                if let Some(secs) = time_slider(
                    ui,
                    painter,
                    right_x,
                    ry,
                    right_w,
                    self.game.chess_len,
                    &pal,
                    "chess_time",
                    game::MIN_PHASE,
                    game::MAX_PHASE,
                ) {
                    self.game.set_len(Phase::Chess, secs);
                }
                ry += 30.0;

                text(
                    painter,
                    Pos2::new(right_x, ry),
                    Align2::LEFT_TOP,
                    "ВРЕМЯ · ШАШКИ",
                    9.0,
                    alpha(pal.text_dim, 0.75),
                );
                ry += 15.0;
                if let Some(secs) = time_slider(
                    ui,
                    painter,
                    right_x,
                    ry,
                    right_w,
                    self.game.checkers_len,
                    &pal,
                    "checkers_time",
                    game::MIN_PHASE,
                    game::MAX_PHASE,
                ) {
                    self.game.set_len(Phase::Checkers, secs);
                }
                ry += 30.0;
            }
        }

        text(
            painter,
            Pos2::new(right_x, ry),
            Align2::LEFT_TOP,
            "ШАШКИ",
            9.0,
            alpha(pal.text_dim, 0.75),
        );
        ry += 15.0;
        let mandatory = self.game.mandatory_capture;
        let crown = self.game.crown_non_pawns;
        let icons = self.show_checker_icons;
        let cw = (right_w - 16.0) / 3.0;
        let capture_chip = Rect::from_min_size(Pos2::new(right_x, ry), Vec2::new(cw, 28.0));
        let icons_chip = Rect::from_min_size(
            Pos2::new(right_x + cw + 8.0, ry),
            Vec2::new(cw, 28.0),
        );
        let crown_chip = Rect::from_min_size(
            Pos2::new(right_x + (cw + 8.0) * 2.0, ry),
            Vec2::new(cw, 28.0),
        );
        if chip(
            ui,
            capture_chip,
            if mandatory {
                "Взятие: да"
            } else {
                "Взятие: нет"
            },
            mandatory,
            &pal,
        ) {
            self.game.mandatory_capture = !mandatory;
        }
        if chip(
            ui,
            icons_chip,
            if icons {
                "Иконки: да"
            } else {
                "Иконки: нет"
            },
            icons,
            &pal,
        ) {
            self.show_checker_icons = !icons;
        }
        if chip(
            ui,
            crown_chip,
            if crown {
                "Дамки: все"
            } else {
                "Дамки: пешки"
            },
            crown,
            &pal,
        ) {
            self.game.crown_non_pawns = !crown;
        }
        ry += 34.0;
        text(
            painter,
            Pos2::new(right_x, ry),
            Align2::LEFT_TOP,
            if mandatory {
                "Бить обязательно; серию нужно продолжать."
            } else {
                "Можно без взятия; серию можно закончить."
            },
            9.0,
            alpha(pal.text_dim, 0.75),
        );
        text(
            painter,
            Pos2::new(right_x, ry + 13.0),
            Align2::LEFT_TOP,
            "Иконки — кроме пешек; дамки — все или только пешки.",
            9.0,
            alpha(pal.text_dim, 0.75),
        );
        ry += 32.0;

        text(
            painter,
            Pos2::new(right_x, ry),
            Align2::LEFT_TOP,
            "ПЛАВНОСТЬ ХОДА",
            9.0,
            alpha(pal.text_dim, 0.75),
        );
        ry += 16.0;
        let track = Rect::from_min_size(Pos2::new(right_x, ry), Vec2::new(right_w - 72.0, 6.0));
        let slider = ui.interact(
            track.expand2(Vec2::new(0.0, 14.0)),
            ui.id().with("smooth_slider"),
            Sense::click_and_drag(),
        );
        if slider.dragged() || slider.clicked() {
            if let Some(p) = slider.interact_pointer_pos() {
                let t = ((p.x - track.left()) / track.width()).clamp(0.0, 1.0);
                self.game.move_anim_secs = 0.05 + t * 0.55;
            }
        }
        let frac = ((self.game.move_anim_secs - 0.05) / 0.55).clamp(0.0, 1.0);
        painter.rect_filled(track, 3.0, alpha(Color32::WHITE, 0.10));
        painter.rect_filled(
            Rect::from_min_size(track.min, Vec2::new(track.width() * frac, 6.0)),
            3.0,
            pal.accent,
        );
        painter.circle_filled(
            Pos2::new(track.left() + track.width() * frac, track.center().y),
            8.0,
            pal.accent,
        );
        text(
            painter,
            Pos2::new(track.right() + 10.0, track.center().y - 8.0),
            Align2::LEFT_TOP,
            format!("{:.2} с", self.game.move_anim_secs),
            11.5,
            alpha(pal.text, 0.95),
        );
        ry += 30.0;

        text(
            painter,
            Pos2::new(right_x, ry),
            Align2::LEFT_TOP,
            "МАТ · ТРАЕКТОРИИ",
            9.0,
            alpha(pal.text_dim, 0.75),
        );
        ry += 15.0;
        let half = (right_w - 8.0) * 0.5;
        let mate_chip = Rect::from_min_size(Pos2::new(right_x, ry), Vec2::new(half, 28.0));
        if chip(
            ui,
            mate_chip,
            if self.mate_lines {
                "Атака: да"
            } else {
                "Атака: нет"
            },
            self.mate_lines,
            &pal,
        ) {
            self.mate_lines = !self.mate_lines;
        }
        let escape_chip = Rect::from_min_size(
            Pos2::new(right_x + half + 8.0, ry),
            Vec2::new(half, 28.0),
        );
        if chip(
            ui,
            escape_chip,
            if self.mate_escape {
                "Окружение: да"
            } else {
                "Окружение: нет"
            },
            self.mate_escape,
            &pal,
        ) {
            self.mate_escape = !self.mate_escape;
        }
        ry += 32.0;

        let columns_end = ly.max(ry);
        let summary_y = (columns_end + 16.0).max(card.bottom() - 132.0);
        let summary = if self.game.twilight_by_moves {
            i18n::trf(
                "Сейчас: смена фаз по ходам, время хода {secs} с",
                &[("secs", &format!("{:.0}", self.game.move_time_limit))],
            )
        } else if self.game.auto_phase {
            i18n::trf(
                "Сейчас: автосмена, 10–{max} с (текущая фаза {phase} с)",
                &[
                    ("max", &format!("{:.0}", self.game.auto_max)),
                    ("phase", &format!("{:.0}", self.game.phase_left)),
                ],
            )
        } else {
            i18n::trf(
                "Сейчас: шахматы {chess} с · шашки {checkers} с",
                &[
                    ("chess", &format!("{:.0}", self.game.chess_len)),
                    ("checkers", &format!("{:.0}", self.game.checkers_len)),
                ],
            )
        };
        text(
            painter,
            Pos2::new(x, summary_y),
            Align2::LEFT_TOP,
            summary,
            12.0,
            alpha(pal.text, 0.92),
        );
        text(
            painter,
            Pos2::new(x, summary_y + 26.0),
            Align2::LEFT_TOP,
            "Свои звуки — в папке sounds (WAV или MP3), музыка — music/theme.wav,",
            10.0,
            alpha(pal.text_dim, 0.75),
        );
        text(
            painter,
            Pos2::new(x, summary_y + 42.0),
            Align2::LEFT_TOP,
            "картинки фигур — в папке pieces (PNG с прозрачным фоном).",
            10.0,
            alpha(pal.text_dim, 0.75),
        );

        let close = Rect::from_min_size(
            Pos2::new(card.right() - pad - 160.0, card.bottom() - pad - 42.0),
            Vec2::new(160.0, 42.0),
        );
        if button(ui, close, "Готово", true, &pal, 14.0) {
            self.settings = false;
        }
    }

    // ------------------------- превращение пешки -------------------------

    fn paint_promotion(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, layout: &Layout) {
        if self.game.pending_promotion.is_none() {
            return;
        }
        let pal = Palette::for_phase(self.game.phase);
        let next = Palette::for_phase(self.game.phase.other());
        let cur = blend(pal, next, smooth(self.game.trans));
        painter.rect_filled(layout.full, 0.0, Color32::from_black_alpha(170));

        let card = Rect::from_center_size(layout.board.center(), Vec2::new(440.0, 240.0));
        rr(
            painter,
            card,
            22.0,
            Color32::from_rgb(24, 21, 32),
            Stroke::new(1.0_f32, alpha(Color32::WHITE, 0.12)),
        );

        text(
            painter,
            Pos2::new(card.center().x, card.top() + 28.0),
            Align2::CENTER_CENTER,
            "ПРЕВРАЩЕНИЕ ПЕШКИ",
            19.0,
            Color32::from_rgb(240, 231, 217),
        );
        text(
            painter,
            Pos2::new(card.center().x, card.top() + 52.0),
            Align2::CENTER_CENTER,
            "Пешка на последнем ряду — выберите новую фигуру",
            12.0,
            alpha(cur.text_dim, 0.95),
        );

        let choices = [
            (Kind::Queen, "Ферзь"),
            (Kind::Rook, "Ладья"),
            (Kind::Bishop, "Слон"),
            (Kind::Knight, "Конь"),
        ];
        let bw = 88.0;
        let bh = 112.0;
        let gap = 12.0;
        let total = 4.0 * bw + 3.0 * gap;
        let start = card.center().x - total * 0.5;
        let top = card.top() + 74.0;
        for (i, (kind, label)) in choices.iter().enumerate() {
            let r = Rect::from_min_size(
                Pos2::new(start + i as f32 * (bw + gap), top),
                Vec2::new(bw, bh),
            );
            let resp = ui.interact(r, ui.id().with(("promo", i)), Sense::click());
            let hover = resp.hovered();
            let bg = if hover {
                Color32::from_rgb(56, 47, 72)
            } else {
                Color32::from_rgb(36, 31, 48)
            };
            rr(
                painter,
                r,
                12.0,
                bg,
                Stroke::new(
                    1.0_f32,
                    alpha(
                        if hover { cur.accent } else { Color32::WHITE },
                        if hover { 0.55 } else { 0.08 },
                    ),
                ),
            );
            let icon = Pos2::new(r.center().x, r.top() + 40.0);
            self.paint_piece(painter, icon, 56.0, *kind, false, self.game.turn, &cur, 1.0, false);
            text(
                painter,
                Pos2::new(r.center().x, r.bottom() - 18.0),
                Align2::CENTER_CENTER,
                *label,
                12.5,
                alpha(cur.text, 0.95),
            );
            if hover {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if resp.clicked() {
                self.game.choose_promotion(*kind);
            }
        }
    }

    // ------------------------- правила -------------------------

    fn paint_rules(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, painter: &egui::Painter, layout: &Layout) {
        painter.rect_filled(layout.full, 0.0, Color32::from_black_alpha(150));
        let card_w = (layout.full.width() * 0.60).clamp(640.0, 780.0);
        let card_h = (layout.full.height() * 0.70).clamp(500.0, 580.0);
        let card = Rect::from_center_size(layout.full.center(), Vec2::new(card_w, card_h));
        self.rules_rect = card;
        rr(
            painter,
            card,
            26.0,
            Color32::from_rgb(22, 20, 30),
            Stroke::new(1.0_f32, alpha(Color32::WHITE, 0.10)),
        );

        let pad = 26.0;
        let x = card.left() + pad;
        let mut y = card.top() + 24.0;

        text(
            painter,
            Pos2::new(x, y),
            Align2::LEFT_TOP,
            "Правила ModeShift: Bipolar",
            22.0,
            Color32::from_rgb(240, 231, 217),
        );
        text(
            painter,
            Pos2::new(x + 2.0, y + 34.0),
            Align2::LEFT_TOP,
            "Одна партия — две игры. Раз в N секунд фигуры превращаются, и правила меняются вместе с ними.",
            12.5,
            Color32::from_rgb(158, 150, 168),
        );
        let tabs_y = card.top() + 80.0;
        let tab_w = 170.0;
        let rules_tab = Rect::from_min_size(Pos2::new(x, tabs_y), Vec2::new(tab_w, 32.0));
        let figures_tab = Rect::from_min_size(
            Pos2::new(x + tab_w + 8.0, tabs_y),
            Vec2::new(tab_w, 32.0),
        );
        let close = Rect::from_min_size(
            Pos2::new(card.right() - pad - 150.0, card.bottom() - pad - 42.0),
            Vec2::new(150.0, 42.0),
        );
        let pal = Palette::for_phase(self.game.phase);
        let _ = chip(ui, rules_tab, "ПРАВИЛА", true, &pal);
        if chip(ui, figures_tab, "ФИГУРЫ", false, &pal) {
            self.rules = false;
            self.guide = true;
            self.guide_kind = Kind::Pawn;
            self.guide_checkers = false;
            self.guide_damka = false;
            self.guide_demo = None;
        }
        y = card.top() + 132.0;

        let chess_rules = [
            "Ходы классические: пешка, конь, слон, ладья, ферзь, король.",
            "Без рокировки и взятия на проходе — только чистые ходы.",
            "Пешка на последнем ряду по умолчанию становится ферзём; выбор фигуры — в настройках.",
            "Шах и мат работают: короля нельзя оставить под боем.",
            "Пат не ничья: сумерки сразу продолжают бой.",
        ];
        let checkers_rules = [
            "В этом гибриде простые шашки ходят по диагонали вперёд и назад.",
            "Шашка остаётся на клетках своего цвета.",
            "Можно подставляться под удар, если обязательного взятия нет.",
            "Король в шашечной фазе — обычная шашка с точкой.",
            "Дамки летят по диагонали на любое расстояние.",
            "Бить обязательно; серия взятий продолжается, пока есть цель.",
            "Обязательное взятие можно отключить в настройках.",
            "Иконки на шашках показывают вид фигуры; пешки — без иконок.",
            "По умолчанию дамкой может стать любая фигура; «Дамки: пешки» — только пешки.",
            "Шашка на последнем ряду становится дамкой и продолжает серию как дамка.",
            "Короля можно съесть и победить.",
            "Если застряли оба — сумерки сразу выводят в шахматы.",
        ];
        let shared = [
            "Сумерки длятся 1,7 секунды: ходы замирают, фигуры перетекают.",
            "Переход ждёт, пока игрок завершит серию взятий.",
            "Победа: мат, съеденный король или истребление фигур.",
            "Время шахмат и шашек задаётся отдельно в настройках.",
            "Смена фаз: по времени или после ходов; по ходам — случайно.",
            "В режиме по ходам внутри хода всегда одна игра, после хода фаза может смениться.",
            "В режиме по ходам у каждого хода есть время 10–180 с; по истечении ход переходит противнику.",
            "Компьютер думает 2–3 с и подсвечивает, какую фигуру берёт.",
            "После партии «Просмотр» показывает ходы по шагам с анимацией.",
            "При мате видны красные линии атак на короля, жёлтые линии фигур, перекрывающих выходы, и список матующих фигур.",
            "С компьютером вы играете белыми; сложность меняет глубину.",
        ];

        let chess_col = Color32::from_rgb(232, 179, 106);
        let check_col = Color32::from_rgb(111, 217, 229);
        let shared_col = Color32::from_rgb(150, 142, 162);
        let body = Color32::from_rgb(214, 208, 222);

        let mut job = egui::text::LayoutJob::default();
        job.wrap.max_width = (card.right() - pad - x).max(200.0);
        fn push_line(job: &mut egui::text::LayoutJob, line: &str, size: f32, color: Color32) {
            job.append(
                line,
                0.0,
                egui::TextFormat {
                    font_id: egui::FontId::proportional(size),
                    color,
                    ..Default::default()
                },
            );
        }
        push_line(&mut job, "КОГДА ФИГУРЫ — ШАХМАТЫ\n", 14.5, chess_col);
        for line in chess_rules {
            push_line(&mut job, &format!("• {line}\n"), 13.0, body);
        }
        push_line(&mut job, "\n\n", 13.0, body);
        push_line(&mut job, "КОГДА ФИГУРЫ — ШАШКИ\n", 14.5, check_col);
        for line in checkers_rules {
            push_line(&mut job, &format!("• {line}\n"), 13.0, body);
        }
        push_line(&mut job, "\n\n", 13.0, body);
        push_line(&mut job, "СУМЕРКИ\n", 14.5, shared_col);
        for line in shared {
            push_line(&mut job, &format!("• {line}\n"), 13.0, body);
        }

        let text_rect = Rect::from_min_max(
            Pos2::new(x, y),
            Pos2::new(card.right() - pad, close.top() - 12.0),
        );
        let _ = ui.scope_builder(
            egui::UiBuilder::new().max_rect(text_rect),
            |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("rules_scroll")
                    .auto_shrink([false, false])
                    .max_height(text_rect.height())
                    .show(ui, |ui| {
                        ui.add(egui::Label::new(job).selectable(true).wrap());
                    });
            },
        );

        if button(ui, close, "Понятно", true, &pal, 14.0) {
            self.rules = false;
        }
        let _ = ctx;
    }
    // ------------------------- журнал партии -------------------------

    fn paint_journal(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, layout: &Layout) {
        painter.rect_filled(layout.full, 0.0, Color32::from_black_alpha(170));
        let card = Rect::from_center_size(
            layout.full.center(),
            Vec2::new(
                (layout.full.width() * 0.54).clamp(620.0, 760.0),
                (layout.full.height() * 0.55).clamp(360.0, 480.0),
            ),
        );
        self.journal_rect = card;
        let pal = Palette::for_phase(self.game.phase);
        rr(
            painter,
            card,
            22.0,
            Color32::from_rgb(22, 20, 30),
            Stroke::new(1.0_f32, alpha(pal.accent, 0.35)),
        );
        let pad = 22.0;
        let x = card.left() + pad;

        text(
            painter,
            Pos2::new(x, card.top() + 20.0),
            Align2::LEFT_TOP,
            "ЖУРНАЛ ПАРТИИ",
            20.0,
            pal.text,
        );
        let saved = if let Some(path) = &self.last_log {
            i18n::trf("Файл: {path}", &[("path", &path.display().to_string())])
        } else {
            "Файл не сохранён — журнал показан из памяти.".to_string()
        };
        text(
            painter,
            Pos2::new(x, card.top() + 48.0),
            Align2::LEFT_TOP,
            saved,
            10.0,
            alpha(pal.text_dim, 0.8),
        );

        let close = Rect::from_min_size(
            Pos2::new(card.right() - pad - 140.0, card.bottom() - pad - 36.0),
            Vec2::new(140.0, 36.0),
        );
        let copy = Rect::from_min_size(
            Pos2::new(x, card.bottom() - pad - 36.0),
            Vec2::new(160.0, 36.0),
        );
        let text_rect = Rect::from_min_max(
            Pos2::new(x, card.top() + 70.0),
            Pos2::new(card.right() - pad, close.top() - 10.0),
        );
        let journal_text = self.journal_text.clone();
        let _ = ui.scope_builder(
            egui::UiBuilder::new().max_rect(text_rect),
            |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("journal_scroll")
                    .auto_shrink([false, false])
                    .max_height(text_rect.height())
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(journal_text)
                                    .size(12.0)
                                    .color(Color32::from_rgb(214, 208, 222)),
                            )
                            .selectable(true)
                            .wrap(),
                        );
                    });
            },
        );

        if button(ui, copy, "Копировать", false, &pal, 12.0) {
            ui.ctx().copy_text(self.journal_text.clone());
        }
        if button(ui, close, "Закрыть", true, &pal, 13.0) {
            self.journal = false;
        }
    }

    // ------------------------- результат -------------------------

    fn paint_result(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, painter: &egui::Painter, layout: &Layout) {
        // Лёгкое затемнение: доска с повтором остаётся видимой.
        painter.rect_filled(layout.full, 0.0, Color32::from_black_alpha(70));
        let card = Rect::from_center_size(
            layout.panel.center(),
            Vec2::new((layout.panel.width() - 18.0).max(260.0), 320.0),
        );
        let pal = Palette::for_phase(self.game.phase);
        rr(
            painter,
            card,
            22.0,
            alpha(Color32::from_rgb(22, 20, 30), 0.92),
            Stroke::new(1.0_f32, alpha(pal.accent, 0.45)),
        );

        let g = &self.game;
        let (title, reason, color) = match &g.status {
            game::Status::Win { side, reason } => (
                i18n::trf("ПОБЕДА {side}", &[("side", &side.name_ru().to_uppercase())]),
                reason.clone(),
                pal.accent,
            ),
            _ => ("ПАРТИЯ".to_string(), String::new(), pal.text),
        };
        let pad = 22.0;
        let x = card.left() + pad;
        let w = card.width() - pad * 2.0;

        spaced_text(
            painter,
            Pos2::new(x, card.top() + 30.0),
            &title,
            17.0,
            color,
            2.0,
        );
        text(
            painter,
            Pos2::new(x, card.top() + 58.0),
            Align2::LEFT_TOP,
            reason,
            12.5,
            alpha(pal.text_dim, 0.95),
        );

        // Строка повтора: кто кого срубил и в каком режиме.
        let replay_line = if let Some((_, to)) = g.last_move {
            if let (Some(attacker), Some((_, victim))) = (g.piece_at(to), g.last_capture) {
                let phase = if g.phase == Phase::Checkers {
                    i18n::tr("шашки")
                } else {
                    i18n::tr("шахматы")
                };
                let side_ru = |s: Side| {
                    if s == Side::White {
                        i18n::tr("белые")
                    } else {
                        i18n::tr("чёрные")
                    }
                };
                let (att, vic) = if g.phase == Phase::Checkers {
                    (
                        if attacker.is_damka() {
                            i18n::tr("дамка")
                        } else {
                            i18n::tr("шашка")
                        },
                        if victim.kind == Kind::King {
                            i18n::tr("король")
                        } else {
                            i18n::tr("шашка")
                        },
                    )
                } else {
                    (piece_ru(attacker.kind), piece_ru(victim.kind))
                };
                i18n::trf(
                    "Повтор: {att_side} — {att}, {vic_side} — {vic} · {phase}",
                    &[
                        ("att_side", side_ru(attacker.side)),
                        ("att", att),
                        ("vic_side", side_ru(victim.side)),
                        ("vic", vic),
                        ("phase", phase),
                    ],
                )
            } else {
                "Повтор последнего хода".to_string()
            }
        } else {
            String::new()
        };
        if !replay_line.is_empty() {
            let galley = painter.layout(
                replay_line,
                FontId::proportional(11.5),
                alpha(pal.text, 0.9),
                w,
            );
            painter.galley(Pos2::new(x, card.top() + 84.0), galley, Color32::WHITE);
        }
        text(
            painter,
            Pos2::new(x, card.top() + 126.0),
            Align2::LEFT_TOP,
            i18n::trf("Ходов: {n}", &[("n", &g.history.len().to_string())]),
            11.0,
            alpha(pal.text_dim, 0.7),
        );
        if self.mate_lines && g.phase == Phase::Chess {
            if let game::Status::Win { side, reason } = &g.status {
                if reason.contains("мат") {
                    if let Some(king) = g.king_sq(side.other()) {
                        let attackers = g.attackers_to(king, *side);
                        if !attackers.is_empty() {
                            let names = attackers
                                .iter()
                                .map(|(sq, kind)| {
                                    format!("{} {}", piece_ru(*kind), game::sq_name(*sq))
                                })
                                .collect::<Vec<_>>()
                                .join(" · ");
                            let line = i18n::trf("Мат ставят: {names}", &[("names", &names)]);
                            let galley = painter.layout(
                                line,
                                FontId::proportional(10.5),
                                alpha(pal.danger, 0.95),
                                w,
                            );
                            painter.galley(
                                Pos2::new(x, card.top() + 146.0),
                                galley,
                                Color32::WHITE,
                            );
                        }
                    }
                }
            }
        }

        if self.journal {
            return;
        }
        let gap = 8.0;
        let half = (w - gap) * 0.5;
        let again = Rect::from_min_size(
            Pos2::new(x, card.bottom() - 104.0),
            Vec2::new(half, 42.0),
        );
        let journal = Rect::from_min_size(
            Pos2::new(x + half + gap, card.bottom() - 104.0),
            Vec2::new(half, 42.0),
        );
        let review = Rect::from_min_size(
            Pos2::new(x, card.bottom() - 54.0),
            Vec2::new(half, 38.0),
        );
        let rules = Rect::from_min_size(
            Pos2::new(x + half + gap, card.bottom() - 54.0),
            Vec2::new(half, 38.0),
        );
        if button(ui, again, "Новая партия", true, &pal, 13.0) {
            self.new_game();
        }
        if button(ui, journal, "Журнал", false, &pal, 13.0) {
            self.journal_text = self.build_journal_text();
            self.journal = true;
        }
        if button(ui, review, "Просмотр", false, &pal, 13.0) {
            self.start_review();
        }
        if button(ui, rules, "Правила", false, &pal, 13.0) {
            self.rules = true;
        }
        let _ = ctx;
    }

    fn paint_guide(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, layout: &Layout) {
        painter.rect_filled(layout.full, 0.0, Color32::from_black_alpha(180));
        let card = Rect::from_center_size(layout.full.center(), Vec2::new(860.0, 650.0));
        self.guide_rect = card;
        let pal = Palette::for_phase(if self.guide_checkers {
            Phase::Checkers
        } else {
            Phase::Chess
        });
        rr(
            painter,
            card,
            26.0,
            Color32::from_rgb(22, 20, 30),
            Stroke::new(1.0_f32, alpha(pal.accent, 0.40)),
        );
        let pad = 30.0;
        let left_x = card.left() + pad;
        let right_x = left_x + 300.0;
        let right_w = card.right() - pad - right_x;

        text(
            painter,
            Pos2::new(left_x, card.top() + 20.0),
            Align2::LEFT_TOP,
            "ФИГУРЫ",
            21.0,
            pal.text,
        );
        text(
            painter,
            Pos2::new(left_x, card.top() + 48.0),
            Align2::LEFT_TOP,
            "Сначала выберите игру, потом фигуру",
            10.0,
            alpha(pal.text_dim, 0.85),
        );
        let close = Rect::from_min_size(
            Pos2::new(card.right() - pad - 82.0, card.top() + 18.0),
            Vec2::new(82.0, 28.0),
        );
        let back_rules = Rect::from_min_size(
            Pos2::new(close.left() - 92.0, card.top() + 18.0),
            Vec2::new(84.0, 28.0),
        );

        // Сначала тип игры, затем фигура.
        let mode_w = 190.0;
        let mode_y = card.top() + 78.0;
        let chess_mode = Rect::from_min_size(
            Pos2::new(left_x, mode_y),
            Vec2::new(mode_w, 34.0),
        );
        let check_mode = Rect::from_min_size(
            Pos2::new(left_x + mode_w + 10.0, mode_y),
            Vec2::new(mode_w, 34.0),
        );
        if chip(ui, chess_mode, "ШАХМАТЫ", !self.guide_checkers, &pal) {
            self.guide_checkers = false;
            self.guide_damka = false;
            self.guide_demo = None;
        }
        if chip(ui, check_mode, "ШАШКИ", self.guide_checkers, &pal) {
            self.guide_checkers = true;
            self.guide_kind = Kind::Pawn;
            self.guide_damka = false;
            self.guide_demo = None;
        }

        let tab_gap = 8.0;
        if self.guide_checkers {
            let options = [
                ("Пешки", Kind::Pawn, false),
                ("Пешка-король", Kind::King, false),
                ("Дамка", Kind::Pawn, true),
            ];
            let tab_w = (card.width() - pad * 2.0 - tab_gap * 2.0) / 3.0;
            for (i, (label, kind, damka)) in options.iter().enumerate() {
                let r = Rect::from_min_size(
                    Pos2::new(left_x + i as f32 * (tab_w + tab_gap), card.top() + 124.0),
                    Vec2::new(tab_w, 30.0),
                );
                let selected = self.guide_kind == *kind && self.guide_damka == *damka;
                if chip(ui, r, label, selected, &pal) {
                    self.guide_kind = *kind;
                    self.guide_damka = *damka;
                    self.guide_demo = None;
                }
            }
        } else {
            let kinds = [
                Kind::Pawn,
                Kind::Knight,
                Kind::Bishop,
                Kind::Rook,
                Kind::Queen,
                Kind::King,
            ];
            let tab_w = (card.width() - pad * 2.0 - tab_gap * 5.0) / 6.0;
            for (i, kind) in kinds.iter().enumerate() {
                let r = Rect::from_min_size(
                    Pos2::new(left_x + i as f32 * (tab_w + tab_gap), card.top() + 124.0),
                    Vec2::new(tab_w, 30.0),
                );
                if chip(
                    ui,
                    r,
                    piece_title_ru(*kind),
                    self.guide_kind == *kind,
                    &pal,
                ) {
                    self.guide_kind = *kind;
                    self.guide_damka = false;
                    self.guide_demo = None;
                }
            }
        }

        text(
            painter,
            Pos2::new(
                left_x,
                card.top() + 190.0,
            ),
            Align2::LEFT_TOP,
            if self.guide_checkers {
                "ФИГУРА В ШАШКАХ"
            } else {
                "ФИГУРА В ШАХМАТАХ"
            },
            9.0,
            alpha(pal.text_dim, 0.75),
        );
        self.paint_piece(
            painter,
            Pos2::new(left_x + 126.0, card.top() + 292.0),
            116.0,
            self.guide_kind,
            self.guide_damka,
            Side::White,
            &pal,
            1.0,
            self.guide_checkers,
        );
        text(
            painter,
            Pos2::new(left_x, card.top() + 372.0),
            Align2::LEFT_TOP,
            "ДЕМОНСТРАЦИЯ",
            8.5,
            alpha(pal.text_dim, 0.65),
        );
        let board = Rect::from_min_size(
            Pos2::new(left_x + 18.0, card.top() + 394.0),
            Vec2::splat(210.0),
        );
        self.paint_guide_board(painter, board, &pal);

        let (move_text, capture_text) = if self.guide_checkers {
            guide_checkers_text(self.guide_kind, self.guide_damka)
        } else {
            guide_text(self.guide_kind, false)
        };
        text(
            painter,
            Pos2::new(right_x, card.top() + 190.0),
            Align2::LEFT_TOP,
            if self.guide_checkers {
                if self.guide_damka {
                    "Дамка"
                } else if self.guide_kind == Kind::King {
                    "Пешка-король"
                } else {
                    "Пешки"
                }
            } else {
                piece_title_ru(self.guide_kind)
            },
            27.0,
            pal.text,
        );
        let mut y = card.top() + 238.0;
        text(
            painter,
            Pos2::new(right_x, y),
            Align2::LEFT_TOP,
            "КАК ХОДИТ",
            9.0,
            pal.accent,
        );
        y += 19.0;
        let h = wrapped_text(
            painter,
            Pos2::new(right_x, y),
            right_w,
            move_text,
            11.5,
            pal.text,
        );
        y += h + 26.0;
        text(
            painter,
            Pos2::new(right_x, y),
            Align2::LEFT_TOP,
            "КАК БЬЁТ",
            9.0,
            pal.accent,
        );
        y += 19.0;
        let _ = wrapped_text(
            painter,
            Pos2::new(right_x, y),
            right_w,
            capture_text,
            11.5,
            pal.text,
        );

        let btn_y = card.bottom() - 62.0;
        let bw = (right_w - 10.0) * 0.5;
        let demo_move = Rect::from_min_size(Pos2::new(right_x, btn_y), Vec2::new(bw, 40.0));
        let demo_capture = Rect::from_min_size(
            Pos2::new(right_x + bw + 10.0, btn_y),
            Vec2::new(bw, 40.0),
        );
        if button(
            ui,
            demo_move,
            "Демо хода",
            self.guide_demo.map(|d| d.victim.is_none()) == Some(true),
            &pal,
            11.5,
        ) {
            let (start, dests) = guide_squares(
                self.guide_kind,
                self.guide_checkers,
                self.guide_damka,
            );
            if let Some(dest) = dests.first() {
                self.guide_demo = Some(GuideDemo {
                    from: start,
                    to: *dest,
                    victim: None,
                    t: 0.0,
                });
            }
        }
        if button(
            ui,
            demo_capture,
            "Демо взятия",
            self.guide_demo.map(|d| d.victim.is_some()) == Some(true),
            &pal,
            11.5,
        ) {
            let (from, victim, to) =
                guide_capture(self.guide_kind, self.guide_checkers, self.guide_damka);
            self.guide_demo = Some(GuideDemo {
                from,
                to,
                victim: Some(victim),
                t: 0.0,
            });
        }
        if button(ui, close, "Закрыть", false, &pal, 10.5) {
            self.guide = false;
        }
        if button(ui, back_rules, "Правила", false, &pal, 10.5) {
            self.guide = false;
            self.rules = true;
        }
    }

    fn paint_guide_board(&self, painter: &egui::Painter, rect: Rect, pal: &Palette) {
        let cell = rect.width() / 8.0;
        let checkers = self.guide_checkers;
        let (start, dests) = guide_squares(self.guide_kind, checkers, self.guide_damka);
        for y in 0..8 {
            for x in 0..8 {
                let r = Rect::from_min_size(
                    Pos2::new(
                        rect.left() + x as f32 * cell,
                        rect.top() + y as f32 * cell,
                    ),
                    Vec2::splat(cell),
                );
                let light = (x + y) % 2 == 0;
                let fill = if light {
                    Color32::from_rgb(205, 195, 214)
                } else {
                    Color32::from_rgb(84, 100, 142)
                };
                painter.rect_filled(r, 0.0, fill);
            }
        }
        let sq_rect = |(x, y): Square| {
            Rect::from_min_size(
                Pos2::new(
                    rect.left() + x as f32 * cell,
                    rect.top() + y as f32 * cell,
                ),
                Vec2::splat(cell),
            )
        };
        painter.rect_stroke(
            sq_rect(start).shrink(1.0),
            3.0,
            Stroke::new(2.0_f32, alpha(pal.accent, 0.9)),
            egui::StrokeKind::Inside,
        );
        for dest in &dests {
            let c = sq_rect(*dest).center();
            painter.circle_filled(c, cell * 0.10, alpha(pal.accent, 0.80));
            painter.circle_stroke(
                c,
                cell * 0.19,
                Stroke::new(1.0_f32, alpha(pal.accent, 0.35)),
            );
        }
        let (from, to, t, victim) = match self.guide_demo {
            Some(d) => (d.from, d.to, d.t, d.victim),
            None => (
                start,
                dests.first().copied().unwrap_or(start),
                0.0,
                None,
            ),
        };
        let from_c = sq_rect(from).center();
        let to_c = sq_rect(to).center();
        if t > 0.0 {
            painter.line_segment(
                [from_c, to_c],
                Stroke::new(2.0_f32, alpha(pal.accent, 0.35)),
            );
        }
        if let Some(v) = victim {
            let fade = (1.0 - t).clamp(0.0, 1.0);
            self.paint_piece(
                painter,
                sq_rect(v).center(),
                cell * 0.82,
                Kind::Pawn,
                false,
                Side::Black,
                pal,
                fade,
                checkers,
            );
        }
        let c = lerp_pos(from_c, to_c, t);
        self.paint_piece(
            painter,
            c,
            cell * 0.86,
            self.guide_kind,
            self.guide_damka,
            Side::White,
            pal,
            1.0,
            checkers,
        );
    }

    fn paint_review(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, layout: &Layout) {
        let pal = Palette::for_phase(self.game.phase);
        let card = layout.panel.shrink2(Vec2::new(10.0, 16.0));
        rr(
            painter,
            card,
            20.0,
            alpha(Color32::from_rgb(20, 18, 27), 0.97),
            Stroke::new(1.0_f32, alpha(pal.accent, 0.40)),
        );
        let pad = 16.0;
        let x = card.left() + pad;
        let w = card.width() - pad * 2.0;
        text(
            painter,
            Pos2::new(x, card.top() + 14.0),
            Align2::LEFT_TOP,
            "ПРОСМОТР ПАРТИИ",
            15.0,
            pal.text,
        );
        text(
            painter,
            Pos2::new(x, card.top() + 38.0),
            Align2::LEFT_TOP,
            i18n::trf(
                "Ход {n} из {total}",
                &[
                    ("n", &self.review_ply.to_string()),
                    ("total", &self.review_labels().len().saturating_sub(1).to_string()),
                ],
            ),
            10.0,
            alpha(pal.text_dim, 0.8),
        );
        let close = Rect::from_min_size(
            Pos2::new(card.right() - pad - 74.0, card.top() + 12.0),
            Vec2::new(74.0, 28.0),
        );

        let prev = Rect::from_min_size(
            Pos2::new(x, card.bottom() - 44.0),
            Vec2::new((w - 8.0) * 0.5, 34.0),
        );
        let next = Rect::from_min_size(
            Pos2::new(x + (w - 8.0) * 0.5 + 8.0, card.bottom() - 44.0),
            Vec2::new((w - 8.0) * 0.5, 34.0),
        );
        let list_rect = Rect::from_min_max(
            Pos2::new(x, card.top() + 62.0),
            Pos2::new(card.right() - pad, prev.top() - 10.0),
        );
        let labels = self.review_labels();
        let current = self.review_ply;
        let mut clicked = None;
        let _ = ui.scope_builder(
            egui::UiBuilder::new().max_rect(list_rect),
            |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("review_scroll")
                    .auto_shrink([false, false])
                    .max_height(list_rect.height())
                    .show(ui, |ui| {
                        ui.set_width(w);
                        for (i, label) in labels.iter().enumerate() {
                            let selected = i == current;
                            let resp = ui.add(
                                egui::Button::new(
                                    egui::RichText::new(if selected {
                                        format!("▶ {label}")
                                    } else {
                                        format!("   {label}")
                                    })
                                    .size(10.0)
                                    .color(if selected { pal.accent } else { pal.text }),
                                )
                                .frame(false)
                                .min_size(Vec2::new(w, 21.0)),
                            );
                            if resp.clicked() {
                                clicked = Some(i);
                            }
                        }
                    });
            },
        );
        if button(ui, close, "Закрыть", false, &pal, 10.5) {
            self.exit_review();
        }
        if button(ui, prev, "◀ Назад", false, &pal, 12.0) {
            self.set_review_ply(self.review_ply.saturating_sub(1));
        }
        if button(ui, next, "Вперёд ▶", false, &pal, 12.0) {
            let max = labels.len().saturating_sub(1);
            self.set_review_ply((self.review_ply + 1).min(max));
        }
        if let Some(ply) = clicked {
            self.set_review_ply(ply);
        }
    }

    // ------------------------- скриншот -------------------------

    fn handle_shot(&mut self, ctx: &egui::Context) {
        let Some(shot) = &mut self.shot else {
            return;
        };
        if shot.saved {
            return;
        }
        self.frames += 1;

        if self.frames == 100 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            shot.requested = true;
        }
        if self.frames == 170 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        if self.frames > 500 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        let image = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = image {
            if let Some(dir) = shot.path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let mut bytes = Vec::with_capacity(img.pixels.len() * 4);
            for p in &img.pixels {
                bytes.push(p.r());
                bytes.push(p.g());
                bytes.push(p.b());
                bytes.push(p.a());
            }
            match image::save_buffer(
                &shot.path,
                &bytes,
                img.size[0] as u32,
                img.size[1] as u32,
                image::ColorType::Rgba8,
            ) {
                Ok(()) => {}
                Err(_) => {
                    // без консоли ошибку писать некуда — просто закрываемся
                }
            }
            shot.saved = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        self.time += dt;
        music::tick();
        if self.review {
            self.game.move_anim = (self.game.move_anim
                + dt / self.game.move_anim_secs.max(0.03))
            .min(1.0);
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.exit_review();
            }
            ctx.request_repaint();
            let layout = Layout::new(ctx);
            self.draw(ctx, &layout);
            self.handle_shot(ctx);
            return;
        }
        if self.guide {
            if let Some(demo) = &mut self.guide_demo {
                demo.t = (demo.t + dt / 1.1).min(1.0);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.guide = false;
            }
            ctx.request_repaint();
            let layout = Layout::new(ctx);
            self.handle_pointer(ctx, &layout);
            self.draw(ctx, &layout);
            self.handle_shot(ctx);
            return;
        }
        if self.started && !self.rules && !self.settings {
            self.game.phase_hold = self.mode == Mode::Computer
                && self.game.turn == Side::Black
                && !self.game.is_over();
            self.game.tick(dt);
        }
        if self.game.trans > 0.0 {
            if !self.twilight_sounded {
                sound::twilight();
                self.twilight_sounded = true;
            }
        } else {
            self.twilight_sounded = false;
        }
        if !self.rules
            && !self.settings
            && self.mode == Mode::Computer
            && self.game.turn == Side::Black
            && self.game.pending_promotion.is_some()
        {
            self.game.choose_promotion(Kind::Queen);
        }
        if self.game.is_over() {
            self.replay += dt;
        } else {
            self.replay = 0.0;
        }
        self.update_ai(dt);
        if self.game.is_over() && !self.log_written {
            self.finalize_log("");
        }
        ctx.request_repaint();

        let layout = Layout::new(ctx);
        self.handle_keys(ctx);
        self.handle_pointer(ctx, &layout);
        self.draw(ctx, &layout);
        self.handle_shot(ctx);
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.finalize_log("прервана (приложение закрыто)");
    }
}

// ---------------------------------------------------------------------------
// Мелкие помощники
// ---------------------------------------------------------------------------

fn hash(v: f32) -> f32 {
    let x = (v * 12.9898).sin() * 43758.5453;
    x - x.floor()
}

fn lerp_pos(a: Pos2, b: Pos2, t: f32) -> Pos2 {
    Pos2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

fn wrapped_text(
    painter: &egui::Painter,
    pos: Pos2,
    width: f32,
    s: &str,
    size: f32,
    color: Color32,
) -> f32 {
    let galley = painter.layout(
        s.to_string(),
        FontId::proportional(size),
        color,
        width,
    );
    let h = galley.size().y;
    painter.galley(pos, galley, color);
    h
}

fn piece_ru(kind: Kind) -> &'static str {
    match kind {
        Kind::Pawn => i18n::tr("пешка"),
        Kind::Knight => i18n::tr("конь"),
        Kind::Bishop => i18n::tr("слон"),
        Kind::Rook => i18n::tr("ладья"),
        Kind::Queen => i18n::tr("ферзь"),
        Kind::King => i18n::tr("король"),
    }
}

fn piece_title_ru(kind: Kind) -> &'static str {
    match kind {
        Kind::Pawn => "Пешка",
        Kind::Knight => "Конь",
        Kind::Bishop => "Слон",
        Kind::Rook => "Ладья",
        Kind::Queen => "Ферзь",
        Kind::King => "Король",
    }
}

fn guide_checkers_text(kind: Kind, damka: bool) -> (&'static str, &'static str) {
    if damka {
        return (
            "Дамка летит по диагонали на любое расстояние, пока путь свободен.",
            "Бьёт через одну вражескую фигуру на любой дистанции, вставая на свободную клетку за ней. Серия взятий продолжается, пока есть цель.",
        );
    }
    if kind == Kind::King {
        return (
            "Пешка-король с кружком. Ходит и бьёт как обычная пешка, но потеря короля — поражение. Несмотря на обычный вид, следите за ней особенно внимательно.",
            "Бьёт как обычная шашка — через соседнюю фигуру на свободную клетку за ней. Может стать дамкой, если включено «Дамки: все».",
        );
    }
    (
        "Все пешки в шашках — обычные шашки: по диагонали на одну клетку вперёд и назад. На последнем ряду пешка становится дамкой.",
        "Бьёт через соседнюю вражескую шашку на свободную клетку за ней; серия продолжается. Иконки коня, слона, ладьи и ферзя — только шахматные метки, особых прав не дают; их можно скрыть в настройках.",
    )
}

fn guide_text(kind: Kind, checkers: bool) -> (&'static str, &'static str) {
    if checkers {
        return match kind {
            Kind::Pawn => (
                "Обычная шашка ходит по диагонали на одну клетку вперёд и назад. На последнем ряду становится дамкой, которая летит по диагонали на любое расстояние.",
                "Бьёт через соседнюю вражескую шашку, вставая на свободную клетку за ней. Серия взятий продолжается, пока есть цель. Обязательный бой можно отключить в настройках.",
            ),
            Kind::King => (
                "Король в шашечной фазе — обычная шашка с короной: ходит по диагонали на одну клетку вперёд и назад, а после коронации становится дамкой.",
                "Бьёт как шашка — через соседнюю фигуру на свободное поле за ней. Может стать дамкой, если включено «Дамки: все».",
            ),
            _ => (
                "Иконка показывает вид фигуры, но правила шашечные: по диагонали на одну клетку вперёд и назад. Дамка летит по диагонали на любое расстояние.",
                "Бьёт через соседнюю вражескую фигуру, вставая на свободную клетку за ней; серия продолжается. Может стать дамкой, если включено «Дамки: все».",
            ),
        };
    }
    match kind {
        Kind::Pawn => (
            "Идёт на одну клетку вперёд; с начальной позиции — на две, если путь свободен.",
            "Бьёт по диагонали на одну клетку вперёд. Взятия на проходе в этой версии нет. На последнем ряду превращается в ферзя или выбранную фигуру.",
        ),
        Kind::Knight => (
            "Ходит буквой Г: две клетки по одной оси и одна по другой. Перепрыгивает фигуры.",
            "Бьёт прыжком на вражескую фигуру в точке приземления; свои фигуры на пути не мешают.",
        ),
        Kind::Bishop => (
            "Ходит по диагонали на любое расстояние, пока путь свободен.",
            "Бьёт по диагонали: путь до вражеской фигуры должен быть свободен.",
        ),
        Kind::Rook => (
            "Ходит по вертикали и горизонтали на любое расстояние, пока путь свободен.",
            "Бьёт по вертикали или горизонтали: путь до вражеской фигуры должен быть свободен.",
        ),
        Kind::Queen => (
            "Ходит как ладья и слон: по вертикали, горизонтали и диагонали на любое расстояние.",
            "Бьёт по любой из этих линий, если путь до вражеской фигуры свободен.",
        ),
        Kind::King => (
            "Ходит на одну клетку в любом направлении.",
            "Бьёт на одну клетку в любом направлении. Под бой оставлять нельзя: шах и мат.",
        ),
    }
}

/// (откуда, где стоит жертва, куда встаёт атакующий)
fn guide_capture(kind: Kind, checkers: bool, damka: bool) -> (Square, Square, Square) {
    if checkers {
        if damka {
            return ((2, 3), (4, 5), (6, 7));
        }
        return ((2, 3), (3, 4), (4, 5));
    }
    match kind {
        Kind::Knight => ((3, 3), (4, 5), (4, 5)),
        Kind::Rook => ((3, 3), (3, 5), (3, 5)),
        _ => ((3, 3), (4, 4), (4, 4)),
    }
}

fn guide_squares(kind: Kind, checkers: bool, damka: bool) -> (Square, Vec<Square>) {
    let mut out = Vec::new();
    if checkers {
        let start = (4, 4);
        if damka {
            for (dx, dy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
                let mut x = start.0 as i32 + dx;
                let mut y = start.1 as i32 + dy;
                while (0..8).contains(&x) && (0..8).contains(&y) {
                    out.push((x as usize, y as usize));
                    x += dx;
                    y += dy;
                }
            }
            return (start, out);
        }
        for (dx, dy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
            let x = start.0 as i32 + dx;
            let y = start.1 as i32 + dy;
            if (0..8).contains(&x) && (0..8).contains(&y) {
                out.push((x as usize, y as usize));
            }
            let x2 = start.0 as i32 + dx * 2;
            let y2 = start.1 as i32 + dy * 2;
            if (0..8).contains(&x2) && (0..8).contains(&y2) {
                out.push((x2 as usize, y2 as usize));
            }
        }
        return (start, out);
    }
    let start = if kind == Kind::Pawn { (4, 1) } else { (4, 4) };
    let mut add = |x: i32, y: i32| {
        if (0..8).contains(&x) && (0..8).contains(&y) {
            out.push((x as usize, y as usize));
        }
    };
    match kind {
        Kind::Pawn => {
            add(4, 2);
            add(4, 3);
            add(3, 2);
            add(5, 2);
        }
        Kind::Knight => {
            for (dx, dy) in [
                (1, 2),
                (2, 1),
                (2, -1),
                (1, -2),
                (-1, -2),
                (-2, -1),
                (-2, 1),
                (-1, 2),
            ] {
                add(start.0 as i32 + dx, start.1 as i32 + dy);
            }
        }
        Kind::King => {
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if dx != 0 || dy != 0 {
                        add(start.0 as i32 + dx, start.1 as i32 + dy);
                    }
                }
            }
        }
        Kind::Bishop | Kind::Rook | Kind::Queen => {
            let dirs: &[(i32, i32)] = match kind {
                Kind::Bishop => &[(-1, -1), (-1, 1), (1, -1), (1, 1)],
                Kind::Rook => &[(-1, 0), (1, 0), (0, -1), (0, 1)],
                _ => &[
                    (-1, -1),
                    (-1, 1),
                    (1, -1),
                    (1, 1),
                    (-1, 0),
                    (1, 0),
                    (0, -1),
                    (0, 1),
                ],
            };
            for (dx, dy) in dirs {
                let mut x = start.0 as i32 + dx;
                let mut y = start.1 as i32 + dy;
                while (0..8).contains(&x) && (0..8).contains(&y) {
                    out.push((x as usize, y as usize));
                    x += dx;
                    y += dy;
                }
            }
        }
    }
    (start, out)
}

/// Партия начинается в шахматной фазе.
fn initial_game() -> Game {
    let mut game = Game::new(20.0);
    game.checkers_len = 10.0;
    game.phase_left = 20.0;
    game.phase_total = 20.0;
    game.seed_rng(random_seed());
    game
}

fn random_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
}

/// Демо-позиции для скриншотов и ручной проверки.
fn apply_demo(game: &mut Game, demo: Option<&str>) {
    match demo {
        Some("checkers") => {
            game.phase = Phase::Checkers;
            game.phase_left = 25.0;
            game.board = [[None; 8]; 8];
            game.board[3][3] = Some(Piece::new(Side::White, Kind::King)); // d4
            game.board[5][2] = Some(Piece::new(Side::Black, Kind::King)); // c6
            game.board[2][5] = Some(Piece::new(Side::White, Kind::Bishop)); // f3
            game.board[6][6] = Some(Piece::new(Side::Black, Kind::Bishop)); // g7
            game.board[1][1] = Some(Piece::new(Side::White, Kind::Pawn)); // b2
            game.board[6][1] = Some(Piece::new(Side::Black, Kind::Pawn)); // b7
            game.turn = Side::White;
            game.log.push("демо: шашечная фаза".into());
        }
        Some("promotion") => {
            game.board = [[None; 8]; 8];
            game.board[0][4] = Some(Piece::new(Side::White, Kind::King)); // e1
            game.board[7][7] = Some(Piece::new(Side::Black, Kind::King)); // h8
            game.board[7][0] = Some(Piece::new(Side::White, Kind::Pawn)); // a8
            game.turn = Side::White;
            game.pending_promotion = Some((0, 7));
            game.log.push("демо: превращение пешки".into());
        }
        Some("settings_times") => {
            game.auto_phase = false;
            game.chess_len = 25.0;
            game.checkers_len = 10.0;
            game.phase_left = 25.0;
            game.phase_total = 25.0;
            game.log.push("демо: шахматы 25 с, шашки 10 с".into());
        }
        Some("settings_auto") => {
            game.auto_phase = true;
            game.auto_max = 60.0;
            game.seed_rng(0x51A7_4C0D_9B2E_7711);
            game.refresh_phase_timer();
            game.log.push("демо: автосмена сумерек".into());
        }
        Some("settings_moves") => {
            game.twilight_by_moves = true;
            game.log.push("демо: смена фаз по ходам".into());
        }
        Some("moves") => {
            game.twilight_by_moves = true;
            game.log.push("демо: по ходам".into());
        }
        Some("force") => {
            game.board = [[None; 8]; 8];
            game.phase = Phase::Checkers;
            game.board[3][2] = Some(game::Piece::new(Side::White, Kind::Pawn)); // c4
            game.board[4][3] = Some(game::Piece::new(Side::Black, Kind::Pawn)); // d5
            game.turn = Side::White;
            game.log.push("демо: обязательное взятие".into());
        }
        Some("mate") => {
            // Мат ладьёй по линии h: ферзевый фланг держат ладья g1 и конь f6,
            // поэтому у короля h8 нет ни одного поля для отхода.
            game.board = [[None; 8]; 8];
            game.phase = Phase::Chess;
            game.board[7][7] = Some(Piece::new(Side::Black, Kind::King)); // h8
            game.board[0][7] = Some(Piece::new(Side::White, Kind::Rook)); // h1
            game.board[0][6] = Some(Piece::new(Side::White, Kind::Rook)); // g1
            game.board[5][5] = Some(Piece::new(Side::White, Kind::Knight)); // f6
            game.board[0][4] = Some(Piece::new(Side::White, Kind::King)); // e1
            game.turn = Side::Black;
            game.status = game::Status::Win {
                side: Side::White,
                reason: "мат".into(),
            };
            game.log.push("демо: мат с окружением".into());
        }
        Some("pause") => {
            game.paused = true;
            game.log.push("демо: пауза".into());
        }
        Some("review") => {
            for mv in [
                game::Move {
                    from: (4, 1),
                    to: (4, 3),
                    captured: None,
                },
                game::Move {
                    from: (4, 6),
                    to: (4, 4),
                    captured: None,
                },
                game::Move {
                    from: (6, 0),
                    to: (5, 2),
                    captured: None,
                },
                game::Move {
                    from: (1, 7),
                    to: (2, 5),
                    captured: None,
                },
                game::Move {
                    from: (5, 0),
                    to: (1, 4),
                    captured: None,
                },
                game::Move {
                    from: (0, 6),
                    to: (0, 5),
                    captured: None,
                },
            ] {
                game.apply(mv);
            }
            game.log.push("демо: просмотр партии".into());
        }
        Some("flip_promo") => {
            game.phase = Phase::Checkers;
            game.board = [[None; 8]; 8];
            game.board[0][4] = Some(Piece::new(Side::White, Kind::King)); // e1
            game.board[0][7] = Some(Piece::new(Side::Black, Kind::King)); // a8
            let mut e8 = Piece::new(Side::White, Kind::Pawn);
            e8.crowned = true;
            let mut h8 = Piece::new(Side::White, Kind::Pawn);
            h8.crowned = true;
            game.board[7][4] = Some(e8); // e8
            game.board[7][7] = Some(h8); // h8
            game.turn = Side::White;
            game.trans = 0.9995;
            game.promotion_choice = true;
            game.log.push("демо: две пешки на последнем ряду".into());
        }
        Some("chess_capture") => {
            game.board = [[None; 8]; 8];
            game.board[0][4] = Some(Piece::new(Side::White, Kind::King)); // e1
            game.board[7][7] = Some(Piece::new(Side::Black, Kind::King)); // h8
            game.board[6][5] = Some(Piece::new(Side::White, Kind::Knight)); // f7
            game.turn = Side::White;
            game.apply(Move {
                from: (5, 6),
                to: (7, 7),
                captured: Some((7, 7)),
            });
            game.log.push("демо: взятие короля в шахматах".into());
        }
        Some("win") => {
            game.phase = Phase::Checkers;
            game.board = [[None; 8]; 8];
            let mut damka = Piece::new(Side::White, Kind::Rook);
            damka.crowned = true;
            game.board[3][3] = Some(damka); // d4
            game.board[4][4] = Some(Piece::new(Side::Black, Kind::King)); // e5
            game.turn = Side::White;
            game.apply(Move {
                from: (3, 3),
                to: (5, 5),
                captured: Some((4, 4)),
            });
            game.log.push("демо: победа по королю".into());
        }
        _ => {}
    }
}

fn bell(t: f32) -> f32 {
    (std::f32::consts::PI * t).sin()
}

fn luminance(c: Color32) -> f32 {
    0.2126 * c.r() as f32 / 255.0 + 0.7152 * c.g() as f32 / 255.0 + 0.0722 * c.b() as f32 / 255.0
}

fn radial(painter: &egui::Painter, center: Pos2, radius: f32, color: Color32, strength: f32) {
    let steps = 14;
    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let r = radius * (1.0 - t * 0.72);
        let a = strength * (1.0 - t) * 0.45;
        painter.circle_filled(center, r, alpha(color, a));
    }
}

fn divider(painter: &egui::Painter, x: f32, y: f32, w: f32, c: Color32) {
    painter.line_segment(
        [Pos2::new(x, y), Pos2::new(x + w, y)],
        Stroke::new(1.0_f32, alpha(c, 0.14)),
    );
}

fn button(
    ui: &mut egui::Ui,
    rect: Rect,
    label: &str,
    accent: bool,
    pal: &Palette,
    font: f32,
) -> bool {
    let id = ui.id().with(("button", label, rect.left() as i32, rect.top() as i32));
    let resp = ui.interact(rect, id, Sense::click());
    let hovered = resp.hovered();
    let (bg, fg, stroke) = if accent {
        (
            if hovered {
                mix(pal.accent, Color32::WHITE, 0.14)
            } else {
                pal.accent
            },
            Color32::from_rgb(22, 18, 26),
            Stroke::NONE,
        )
    } else {
        (
            alpha(Color32::WHITE, if hovered { 0.13 } else { 0.07 }),
            pal.text,
            Stroke::new(1.0_f32, alpha(Color32::WHITE, if hovered { 0.16 } else { 0.13 })),
        )
    };
    rr(ui.painter(), rect, 10.0, bg, stroke);
    text(ui.painter(), rect.center(), Align2::CENTER_CENTER, label, font, fg);
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// Маленькая векторная шестерёнка для кнопки «Настройки».
fn draw_gear(painter: &egui::Painter, center: Pos2, r: f32, color: Color32) {
    let stroke = Stroke::new(1.4_f32, color);
    painter.circle_stroke(center, r * 0.60, stroke);
    painter.circle_stroke(center, r * 0.23, stroke);
    for i in 0..6 {
        let a = std::f32::consts::TAU * i as f32 / 6.0;
        let dir = Vec2::new(a.cos(), a.sin());
        painter.line_segment(
            [center + dir * r * 0.58, center + dir * r],
            Stroke::new(2.0_f32, color),
        );
    }
}

/// Кнопка с шестерёнкой слева от подписи: значок и текст центрируются
/// вместе, как в обычных настройках.
fn button_gear(
    ui: &mut egui::Ui,
    rect: Rect,
    label: &str,
    accent: bool,
    pal: &Palette,
    font: f32,
) -> bool {
    let id = ui
        .id()
        .with(("button_gear", label, rect.left() as i32, rect.top() as i32));
    let resp = ui.interact(rect, id, Sense::click());
    let hovered = resp.hovered();
    let (bg, fg, stroke) = if accent {
        (
            if hovered {
                mix(pal.accent, Color32::WHITE, 0.14)
            } else {
                pal.accent
            },
            Color32::from_rgb(22, 18, 26),
            Stroke::NONE,
        )
    } else {
        (
            alpha(Color32::WHITE, if hovered { 0.13 } else { 0.07 }),
            pal.text,
            Stroke::new(1.0_f32, alpha(Color32::WHITE, if hovered { 0.16 } else { 0.13 })),
        )
    };
    rr(ui.painter(), rect, 10.0, bg, stroke);
    let label = i18n::tr_owned(label.to_string());
    let galley = ui
        .painter()
        .layout_no_wrap(label, FontId::proportional(font), fg);
    let icon_r = 7.0;
    let gap = 8.0;
    let total = icon_r * 2.0 + gap + galley.size().x;
    let start = rect.center().x - total * 0.5;
    let cy = rect.center().y;
    draw_gear(ui.painter(), Pos2::new(start + icon_r, cy), icon_r, fg);
    ui.painter().galley(
        Pos2::new(start + icon_r * 2.0 + gap, cy - galley.size().y * 0.5),
        galley,
        fg,
    );
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.clicked()
}

fn time_slider(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    x: f32,
    y: f32,
    w: f32,
    current: f32,
    pal: &Palette,
    id: &str,
    min: f32,
    max: f32,
) -> Option<f32> {
    let track = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w - 72.0, 6.0));
    let resp = ui.interact(
        track.expand2(Vec2::new(0.0, 12.0)),
        ui.id().with(("time_slider", id)),
        Sense::click_and_drag(),
    );
    let mut picked = None;
    if resp.dragged() || resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            let t = ((p.x - track.left()) / track.width()).clamp(0.0, 1.0);
            picked = Some((min + t * (max - min)).round());
        }
    }
    let value = picked.unwrap_or(current.clamp(min, max));
    let frac = ((value - min) / (max - min).max(0.001)).clamp(0.0, 1.0);
    painter.rect_filled(track, 3.0, alpha(Color32::WHITE, 0.10));
    painter.rect_filled(
        Rect::from_min_size(track.min, Vec2::new(track.width() * frac, 6.0)),
        3.0,
        pal.accent,
    );
    painter.circle_filled(
        Pos2::new(track.left() + track.width() * frac, track.center().y),
        8.0,
        pal.accent,
    );
    text(
        painter,
        Pos2::new(track.right() + 10.0, track.center().y - 8.0),
        Align2::LEFT_TOP,
        format!("{:.0} с", value),
        11.5,
        alpha(pal.text, 0.95),
    );
    picked
}

fn chip(ui: &mut egui::Ui, rect: Rect, label: &str, selected: bool, pal: &Palette) -> bool {
    let id = ui.id().with(("chip", label, rect.left() as i32));
    let resp = ui.interact(rect, id, Sense::click());
    let hovered = resp.hovered();
    let (bg, fg, stroke) = if selected {
        (alpha(pal.accent, 0.16), pal.accent, Stroke::new(1.0_f32, alpha(pal.accent, 0.6)))
    } else if hovered {
        (alpha(Color32::WHITE, 0.08), pal.text, Stroke::new(1.0_f32, alpha(Color32::WHITE, 0.10)))
    } else {
        (alpha(Color32::WHITE, 0.03), alpha(pal.text_dim, 0.9), Stroke::new(1.0_f32, alpha(Color32::WHITE, 0.06)))
    };
    rr(ui.painter(), rect, 8.0, bg, stroke);
    text(ui.painter(), rect.center(), Align2::CENTER_CENTER, label, 12.0, fg);
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.clicked()
}

fn badge(painter: &egui::Painter, right_top: Pos2, label: &str, color: Color32, active: bool, size: f32) {
    let label = i18n::tr_owned(label.to_string());
    let galley = painter.layout_no_wrap(label.clone(), FontId::proportional(size), Color32::WHITE);
    let pad = Vec2::new(9.0, 4.0);
    let rect = Rect::from_min_size(
        Pos2::new(right_top.x - galley.size().x - pad.x * 2.0, right_top.y),
        galley.size() + pad * 2.0,
    );
    if active {
        rr(painter, rect, 7.0, alpha(color, 0.18), Stroke::new(1.0_f32, alpha(color, 0.55)));
        text(
            painter,
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            size,
            color,
        );
    } else {
        rr(painter, rect, 7.0, alpha(Color32::WHITE, 0.05), Stroke::NONE);
        text(
            painter,
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            size,
            alpha(color, 0.5),
        );
    }
}
