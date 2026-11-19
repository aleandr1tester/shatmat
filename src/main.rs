#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod art;
mod game;
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Hotseat,
    Computer,
}

// ---------------------------------------------------------------------------
// Точка входа
// ---------------------------------------------------------------------------

fn main() -> eframe::Result<()> {
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
        .with_title("Сумерки — гибрид шахмат и шашек")
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
}

impl PieceTextures {
    fn empty() -> Self {
        Self {
            white: std::array::from_fn(|_| None),
            black: std::array::from_fn(|_| None),
        }
    }

    fn get(&self, side: Side, kind: Kind) -> Option<&PieceArt> {
        let arr = match side {
            Side::White => &self.white,
            Side::Black => &self.black,
        };
        arr[kind_index(kind)].as_ref()
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

fn load_piece_textures(ctx: &egui::Context) -> PieceTextures {
    let mut out = PieceTextures::empty();
    let dir = pieces_dir();
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
            let path = dir.join(format!("{}_{}.png", prefix, kind_file(kind)));
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let Ok(img) = image::load_from_memory(&bytes) else {
                continue;
            };
            let rgba = img.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
            let color_tex = ctx.load_texture(
                format!("piece_{}_{}", prefix, kind_file(kind)),
                color,
                egui::TextureOptions::LINEAR,
            );
            let mut mask_rgba = rgba.as_raw().clone();
            for px in mask_rgba.chunks_exact_mut(4) {
                px[0] = 255;
                px[1] = 255;
                px[2] = 255;
            }
            let mask = egui::ColorImage::from_rgba_unmultiplied(size, &mask_rgba);
            let mask_tex = ctx.load_texture(
                format!("piece_{}_mask_{}", prefix, kind_file(kind)),
                mask,
                egui::TextureOptions::LINEAR,
            );
            let arr = match side {
                Side::White => &mut out.white,
                Side::Black => &mut out.black,
            };
            arr[kind_index(kind)] = Some(PieceArt {
                color: color_tex,
                mask: mask_tex,
            });
        }
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
    shot: Option<Shot>,
    frames: u32,
    twilight_sounded: bool,
    replay: f32,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, shot: Option<PathBuf>, demo: Option<String>) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        style.animation_time = 0.16;
        style.visuals = egui::Visuals::dark();
        cc.egui_ctx.set_style(style);

        let mut game = initial_game();
        apply_demo(&mut game, demo.as_deref());
        let rules_open = demo.as_deref() == Some("rules");
        let settings_open = demo.as_deref() == Some("settings");
        let piece_tex = load_piece_textures(&cc.egui_ctx);
        sound::prepare();
        if music::available() {
            music::set_enabled(true);
        }

        App {
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
            shot: shot.map(|path| Shot {
                path,
                requested: false,
                saved: false,
            }),
            frames: 0,
            twilight_sounded: false,
            replay: 0.0,
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        let (space, r, h, esc) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Space),
                i.key_pressed(egui::Key::R),
                i.key_pressed(egui::Key::H) || i.key_pressed(egui::Key::Questionmark),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if esc {
            if self.settings {
                self.settings = false;
            } else if self.rules {
                self.rules = false;
            } else {
                self.game.deselect();
            }
        }
        if h && !self.settings {
            self.rules = !self.rules;
        }
        if space && !self.rules && !self.settings {
            self.game.paused = !self.game.paused;
        }
        if r && !self.rules && !self.settings {
            self.new_game();
        }
    }

    fn new_game(&mut self) {
        let smooth = self.game.move_anim_secs;
        self.game = Game::new(self.game.phase_len);
        self.game.move_anim_secs = smooth;
        self.ai_wait = 0.6;
        self.twilight_sounded = false;
        self.replay = 0.0;
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

    /// Компьютер играет чёрными: думает полсекунды и делает ход.
    fn update_ai(&mut self, dt: f32) {
        if self.mode != Mode::Computer {
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
            self.ai_wait = 0.6;
            return;
        }
        self.ai_wait -= dt;
        if self.ai_wait > 0.0 {
            return;
        }
        if let Some(mv) = ai::best_move(&self.game, self.difficulty, &mut self.ai_seed) {
            let mark = self.sound_mark();
            self.game.apply(mv);
            if self.game.pending_promotion.is_some() {
                self.game.choose_promotion(Kind::Queen);
            }
            self.play_sound_since(mark);
        }
        self.ai_wait = 0.55;
    }

    fn handle_pointer(&mut self, ctx: &egui::Context, layout: &Layout) {
        let clicked = ctx.input(|i| i.pointer.primary_clicked());
        if !clicked {
            return;
        }
        let pos = match ctx.input(|i| i.pointer.interact_pos()) {
            Some(p) => p,
            None => return,
        };
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
            "СУМЕРКИ",
            30.0,
            pal.text,
            5.0,
        );
        text(
            painter,
            Pos2::new(x + 1.0, layout.header.top() + 57.0),
            Align2::LEFT_TOP,
            "гибрид шахмат и шашек · фигуры дышат и меняют природу",
            12.0,
            alpha(pal.text_dim, 0.95),
        );

        let hint = "клик — ход · Space — пауза · R — новая партия · H — правила";
        text(
            painter,
            Pos2::new(layout.panel.left() - 24.0, layout.header.top() + 34.0),
            Align2::RIGHT_CENTER,
            hint,
            11.5,
            alpha(pal.text_dim, 1.0),
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
        let pulse_amp = 0.028 + 0.05 * bell(g.trans);
        let anim = self.active_move_anim(g);
        for r in 0..8usize {
            for f in 0..8usize {
                let sq = (f, r);
                let seed = f as f32 * 0.83 + r as f32 * 1.71;
                let pulse = 1.0 + pulse_amp * (self.time * 2.0 + seed).sin();
                let base_size = layout.cell * 1.08 * pulse;
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
                } else if let Some((cap, victim)) = g.last_capture {
                    if let Some((_, _, t)) = anim {
                        if sq == cap {
                            let center = layout.square_rect(sq).center();
                            let ghost = (1.0 - t).clamp(0.0, 1.0);
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
                                base_size * (1.0 - 0.25 * t),
                                victim.kind,
                                victim.is_damka(),
                                victim.side,
                                &pal,
                                ghost * 0.85,
                                phase == Phase::Checkers,
                            );
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
            let pill = Rect::from_center_size(layout.board.center(), Vec2::new(320.0, 104.0));
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
                "Space — продолжить партию",
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
            draw_checker(painter, center, size, damka, kind == Kind::King, side, pal, a);
        } else {
            draw_chess_piece(painter, center, size, kind, side, pal, a);
        }
    }

    // ------------------------- панель -------------------------

    fn paint_panel(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, layout: &Layout) {
        let g = &self.game;
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

        text(
            painter,
            Pos2::new(x, y),
            Align2::LEFT_TOP,
            "ПРОТОТИП · СУМЕРКИ",
            9.5,
            alpha(cur.text_dim, 0.9),
        );
        y += 18.0;

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
        } else {
            let secs = g.phase_left.max(0.0).ceil() as i32;
            format!("{}:{:02}", secs / 60, secs % 60)
        };
        let urgent = g.trans == 0.0
            && g.phase_left <= 5.0
            && !g.is_over()
            && !g.paused;
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
            44.0,
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
            } else if g.twilight_pending() {
                "переход ждёт конца взятия".to_string()
            } else if g.passes >= 2 {
                "нет ходов — сумерки выводят из тупика".to_string()
            } else if self.mode == Mode::Computer && g.turn == Side::Black {
                "компьютер думает…".to_string()
            } else {
                format!("до превращения в {}", g.phase.other().one_ru())
            },
            11.5,
            alpha(cur.text_dim, 0.95),
        );
        y += 72.0;

        let frac = if g.trans > 0.0 {
            1.0 - g.trans
        } else {
            (g.phase_left / g.phase_len).clamp(0.0, 1.0)
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
            format!("Ход {}", g.turn.gen_ru()),
            19.0,
            cur.text,
        );
        if let game::Status::Check(_) = &g.status {
            badge(painter, Pos2::new(x + w, y - 2.0), "ШАХ", cur.danger, true, 11.0);
        } else if g.chain.is_some() {
            badge(painter, Pos2::new(x + w, y - 2.0), "ВЗЯТИЕ", cur.accent, true, 11.0);
        }
        y += 34.0;

        // материал
        let diff = g.material_diff();
        let material = match diff {
            0 => "материал равный".to_string(),
            d if d > 0 => format!("белые впереди на {}", d),
            d => format!("чёрные впереди на {}", -d),
        };
        text(
            painter,
            Pos2::new(x, y),
            Align2::LEFT_TOP,
            format!(
                "{} · взято {} : {}",
                material, g.captured_by_white, g.captured_by_black
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
            alpha(cur.text_dim, 0.8),
        );
        y += 18.0;

        // нижние элементы
        let new_btn = Rect::from_min_size(Pos2::new(x, p.bottom() - 46.0), Vec2::new(w, 46.0));
        let row_y = new_btn.top() - 12.0 - 36.0;
        let gap = 8.0;
        let third = (w - gap * 2.0) / 3.0;
        let rules_btn = Rect::from_min_size(Pos2::new(x, row_y), Vec2::new(third, 36.0));
        let pause_btn = Rect::from_min_size(
            Pos2::new(x + third + gap, row_y),
            Vec2::new(third, 36.0),
        );
        let settings_btn = Rect::from_min_size(
            Pos2::new(x + (third + gap) * 2.0, row_y),
            Vec2::new(third, 36.0),
        );
        let chips_y = row_y - 30.0 - 30.0;
        let chips = [
            (15.0, "15 с"),
            (25.0, "25 с"),
            (40.0, "40 с"),
        ];
        let chip_w = (w - 16.0) / 3.0;
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
                12.0,
                alpha(cur.text_dim, 0.7),
            );
            let tip = "Совет: в шашечной фазе король превращается в дамку — съешьте его и победите.";
            let galley = painter.layout(
                tip.to_string(),
                FontId::proportional(11.0),
                alpha(cur.text_dim, 0.55),
                w,
            );
            painter.galley(Pos2::new(x, log_top + 30.0), galley, Color32::WHITE);
        }

        // ритм
        text(
            painter,
            Pos2::new(x, chips_y - 16.0),
            Align2::LEFT_TOP,
            "РИТМ ПРЕВРАЩЕНИЯ",
            8.5,
            alpha(cur.text_dim, 0.75),
        );

        // --- взаимодействие ---
        if self.game.is_over() {
            return;
        }
        if self.settings || self.rules {
            return;
        }
        if self.game.pending_promotion.is_some() {
            return;
        }
        if button(ui, new_btn, "Новая партия", true, &cur, 15.0) {
            self.new_game();
        }
        if button(ui, rules_btn, "Правила", false, &cur, 12.5) {
            self.rules = true;
        }
        let pause_label = if self.game.paused { "Дальше" } else { "Пауза" };
        if button(ui, pause_btn, pause_label, false, &cur, 12.5) {
            self.game.paused = !self.game.paused;
        }
        if button(ui, settings_btn, "Настройки", false, &cur, 10.0) {
            self.settings = true;
            self.rules = false;
        }
        for (i, (secs, label)) in chips.iter().enumerate() {
            let selected = (self.game.phase_len - secs).abs() < 0.5;
            let clicked = chip(ui, chip_rects[i], label, selected, &cur);
            if clicked && self.game.trans == 0.0 && !self.game.is_over() {
                self.game.phase_len = *secs;
                self.game.phase_left = *secs;
            }
        }
    }

    // ------------------------- настройки -------------------------

    fn paint_settings(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, layout: &Layout) {
        painter.rect_filled(layout.full, 0.0, Color32::from_black_alpha(165));
        let card = Rect::from_center_size(layout.full.center(), Vec2::new(560.0, 500.0));
        self.settings_rect = card;
        let pal = Palette::for_phase(self.game.phase);
        rr(
            painter,
            card,
            24.0,
            Color32::from_rgb(22, 20, 30),
            Stroke::new(1.0_f32, alpha(pal.accent, 0.35)),
        );
        let pad = 30.0;
        let x = card.left() + pad;
        let w = card.width() - pad * 2.0;
        let mut y = card.top() + 30.0;

        spaced_text(painter, Pos2::new(x, y), "НАСТРОЙКИ", 22.0, pal.text, 2.4);
        y += 46.0;

        text(
            painter,
            Pos2::new(x, y),
            Align2::LEFT_TOP,
            "ЗВУКИ",
            9.5,
            alpha(pal.text_dim, 0.8),
        );
        y += 18.0;
        let sound_on = sound::enabled();
        let sound_btn = Rect::from_min_size(Pos2::new(x, y), Vec2::new(230.0, 38.0));
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
        y += 48.0;

        text(
            painter,
            Pos2::new(x, y),
            Align2::LEFT_TOP,
            "ИСТОЧНИК",
            9.0,
            alpha(pal.text_dim, 0.75),
        );
        y += 18.0;
        let chip_w = (w - 8.0) * 0.5;
        let builtin = Rect::from_min_size(Pos2::new(x, y), Vec2::new(chip_w, 30.0));
        let custom = Rect::from_min_size(Pos2::new(x + chip_w + 8.0, y), Vec2::new(chip_w, 30.0));
        if chip(ui, builtin, "Встроенные", !sound::use_files(), &pal) {
            sound::set_use_files(false);
        }
        if chip(ui, custom, "Из папки sounds", sound::use_files(), &pal) {
            sound::set_use_files(true);
        }
        y += 46.0;

        let music_avail = music::available();
        let music_on = music::enabled();
        let music_btn = Rect::from_min_size(Pos2::new(x, y), Vec2::new(310.0, 38.0));
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
        y += 44.0;
        text(
            painter,
            Pos2::new(x, y),
            Align2::LEFT_TOP,
            "Фоновая музыка не прерывается звуками ходов.",
            10.5,
            alpha(pal.text_dim, 0.8),
        );
        y += 34.0;

        text(
            painter,
            Pos2::new(x, y),
            Align2::LEFT_TOP,
            "ПЛАВНОСТЬ ХОДА",
            9.5,
            alpha(pal.text_dim, 0.8),
        );
        y += 20.0;
        let track = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w - 96.0, 6.0));
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
            Pos2::new(track.right() + 14.0, track.center().y - 8.0),
            Align2::LEFT_TOP,
            format!("{:.2} с", self.game.move_anim_secs),
            12.0,
            alpha(pal.text, 0.95),
        );
        y += 42.0;

        text(
            painter,
            Pos2::new(x, y),
            Align2::LEFT_TOP,
            "Свои звуки — в папке sounds, музыка — music/theme.wav,",
            10.5,
            alpha(pal.text_dim, 0.75),
        );
        text(
            painter,
            Pos2::new(x, y + 16.0),
            Align2::LEFT_TOP,
            "картинки фигур — в папке pieces (PNG с прозрачным фоном).",
            10.5,
            alpha(pal.text_dim, 0.75),
        );

        let close = Rect::from_min_size(
            Pos2::new(card.right() - pad - 160.0, card.bottom() - 72.0),
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
        let card_w = (layout.full.width() * 0.62).clamp(680.0, 860.0);
        let card_h = (layout.full.height() * 0.88).clamp(640.0, 740.0);
        let card = Rect::from_center_size(layout.full.center(), Vec2::new(card_w, card_h));
        self.rules_rect = card;
        rr(
            painter,
            card,
            26.0,
            Color32::from_rgb(22, 20, 30),
            Stroke::new(1.0_f32, alpha(Color32::WHITE, 0.10)),
        );

        let pad = 34.0;
        let x = card.left() + pad;
        let w = card.width() - pad * 2.0;
        let mut y = card.top() + 30.0;

        text(
            painter,
            Pos2::new(x, y),
            Align2::LEFT_TOP,
            "Правила Сумерек",
            26.0,
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
        y += 72.0;

        let col_w = (w - 28.0) * 0.5;
        let chess_x = x;
        let check_x = x + col_w + 28.0;

        text(
            painter,
            Pos2::new(chess_x, y),
            Align2::LEFT_TOP,
            "КОГДА ФИГУРЫ — ШАХМАТЫ",
            10.5,
            Color32::from_rgb(232, 179, 106),
        );
        text(
            painter,
            Pos2::new(check_x, y),
            Align2::LEFT_TOP,
            "КОГДА ФИГУРЫ — ШАШКИ",
            10.5,
            Color32::from_rgb(111, 217, 229),
        );
        y += 26.0;

        let chess_rules = [
            "Ходы классические: пешка, конь, слон, ладья, ферзь, король.",
            "Без рокировки и взятия на проходе — только чистые ходы.",
            "Пешка на последнем ряду превращается в выбранную фигуру.",
            "Шах и мат работают: короля нельзя оставить под боем.",
            "Пат — ничья.",
        ];
        let checkers_rules = [
            "Простые шашки ходят по диагонали вперёд и назад.",
            "Шашка остаётся на клетках своего цвета.",
            "Можно подставляться под удар, если нет обязательного взятия.",
            "Король в шашечной фазе помечен точкой.",
            "Дамки летят по диагонали на любое расстояние.",
            "Бить обязательно; серия взятий продолжается, пока есть цель.",
            "Съеденная фигура-король — тоже дамка, её можно взять.",
            "Если застряли оба — сумерки сразу выводят в шахматы.",
        ];
        let mut yy = y;
        for line in chess_rules {
            dot_line(painter, chess_x, yy, col_w, line, Color32::from_rgb(232, 179, 106));
            yy += 36.0;
        }
        let mut yy2 = y;
        for line in checkers_rules {
            dot_line(painter, check_x, yy2, col_w, line, Color32::from_rgb(111, 217, 229));
            yy2 += 36.0;
        }

        y = yy.max(yy2) + 6.0;
        painter.line_segment(
            [Pos2::new(x, y), Pos2::new(x + w, y)],
            Stroke::new(1.0_f32, alpha(Color32::WHITE, 0.08)),
        );
        y += 18.0;

        let shared = [
            "Сумерки длятся 1,7 секунды: ходы замирают, фигуры перетекают.",
            "Переход ждёт, пока игрок завершит серию взятий.",
            "Победа: мат, съеденный король или истребление фигур.",
            "Темп превращений: 15, 25 или 40 секунд — в панели справа.",
            "С компьютером вы играете белыми; сложность меняет глубину.",
        ];
        for line in shared {
            dot_line(painter, x, y, w, line, Color32::from_rgb(150, 142, 162));
            y += 26.0;
        }

        let close = Rect::from_min_size(
            Pos2::new(card.right() - pad - 150.0, card.bottom() - pad - 42.0),
            Vec2::new(150.0, 42.0),
        );
        let pal = Palette::for_phase(self.game.phase);
        if button(ui, close, "Понятно", true, &pal, 14.0) {
            self.rules = false;
        }
        let _ = ctx;
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
                format!("ПОБЕДА {}", side.name_ru().to_uppercase()),
                reason.clone(),
                pal.accent,
            ),
            game::Status::Draw(reason) => (
                "НИЧЬЯ".to_string(),
                reason.clone(),
                Color32::from_rgb(200, 190, 210),
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
                    "шашки"
                } else {
                    "шахматы"
                };
                let side_ru = |s: Side| if s == Side::White { "белые" } else { "чёрные" };
                let (att, vic) = if g.phase == Phase::Checkers {
                    (
                        if attacker.is_damka() { "дамка" } else { "шашка" },
                        if victim.kind == Kind::King {
                            "король"
                        } else {
                            "шашка"
                        },
                    )
                } else {
                    (piece_ru(attacker.kind), piece_ru(victim.kind))
                };
                format!(
                    "Повтор: {} — {}, {} — {} · {}",
                    side_ru(attacker.side),
                    att,
                    side_ru(victim.side),
                    vic,
                    phase
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
            format!("Ходов в журнале: {}", g.log.len()),
            11.0,
            alpha(pal.text_dim, 0.7),
        );

        let again = Rect::from_min_size(
            Pos2::new(x, card.bottom() - 104.0),
            Vec2::new(w, 42.0),
        );
        let rules = Rect::from_min_size(
            Pos2::new(x, card.bottom() - 54.0),
            Vec2::new(w, 38.0),
        );
        if button(ui, again, "Новая партия", true, &pal, 14.0) {
            self.new_game();
        }
        if button(ui, rules, "Правила", false, &pal, 13.0) {
            self.rules = true;
        }
        let _ = ctx;
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
        if !self.rules && !self.settings {
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
        if self.game.is_over() {
            self.replay += dt;
        } else {
            self.replay = 0.0;
        }
        self.update_ai(dt);
        ctx.request_repaint();

        let layout = Layout::new(ctx);
        self.handle_keys(ctx);
        self.handle_pointer(ctx, &layout);
        self.draw(ctx, &layout);
        self.handle_shot(ctx);
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

fn piece_ru(kind: Kind) -> &'static str {
    match kind {
        Kind::Pawn => "пешка",
        Kind::Knight => "конь",
        Kind::Bishop => "слон",
        Kind::Rook => "ладья",
        Kind::Queen => "ферзь",
        Kind::King => "король",
    }
}

/// Партия начинается в шахматной фазе.
fn initial_game() -> Game {
    Game::new(25.0)
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

fn dot_line(painter: &egui::Painter, x: f32, y: f32, w: f32, line: &str, color: Color32) {
    painter.circle_filled(Pos2::new(x + 3.0, y + 7.0), 2.0, alpha(color, 0.8));
    let galley = painter.layout(
        line.to_string(),
        FontId::proportional(12.5),
        alpha(Color32::from_rgb(214, 206, 220), 0.95),
        w - 16.0,
    );
    painter.galley(Pos2::new(x + 14.0, y), galley, Color32::WHITE);
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
    let galley = painter.layout_no_wrap(label.to_string(), FontId::proportional(size), Color32::WHITE);
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
