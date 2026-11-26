#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod arena;
mod replay;
mod art;
mod fonts;
mod game;
mod i18n;
mod logging;
mod music;
mod net;
mod rulecode;
mod sound;
mod updater;

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
    /// Партия по локальной сети: сетевой сеанс активен.
    Network,
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
    let mut check_update = false;
    // Турнир ИИ против ИИ без окна: --arena.
    let mut arena_mode = false;
    let mut arena_games = 100usize;
    let mut arena_chess: Vec<u8> = vec![4, 6, 8];
    let mut arena_checkers: Vec<u8> = vec![2, 4, 6, 8];
    let mut arena_difficulty = "medium".to_string();
    let mut arena_difficulty_black: Option<String> = None;
    let mut arena_holds = 2u8;
    let mut arena_draw_plies = 20usize;
    let mut arena_out = PathBuf::from("arena.csv");
    let mut replay_path: Option<PathBuf> = None;
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
        if args[i] == "--check-update" {
            check_update = true;
        }
        if args[i] == "--replay" {
            if let Some(p) = args.get(i + 1) {
                replay_path = Some(PathBuf::from(p));
                i += 1;
            }
        }
        if args[i] == "--arena" {
            arena_mode = true;
        }
        if args[i] == "--games" {
            if let Some(p) = args.get(i + 1) {
                arena_games = p.parse().unwrap_or(arena_games);
                i += 1;
            }
        }
        if args[i] == "--chess" {
            if let Some(p) = args.get(i + 1) {
                arena_chess = p.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                i += 1;
            }
        }
        if args[i] == "--checkers" {
            if let Some(p) = args.get(i + 1) {
                arena_checkers = p.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                i += 1;
            }
        }
        if args[i] == "--draw-plies" {
            if let Some(p) = args.get(i + 1) {
                arena_draw_plies = p.parse().unwrap_or(arena_draw_plies);
                i += 1;
            }
        }
        if args[i] == "--difficulty-black" {
            if let Some(p) = args.get(i + 1) {
                arena_difficulty_black = Some(p.clone());
                i += 1;
            }
        }
        if args[i] == "--holds" {
            if let Some(p) = args.get(i + 1) {
                arena_holds = p.parse().unwrap_or(arena_holds);
                i += 1;
            }
        }
        if args[i] == "--difficulty" {
            if let Some(p) = args.get(i + 1) {
                arena_difficulty = p.clone();
                i += 1;
            }
        }
        if args[i] == "--out" {
            if let Some(p) = args.get(i + 1) {
                arena_out = PathBuf::from(p);
                i += 1;
            }
        }
        i += 1;
    }
    if let Some(path) = replay_path {
        replay::run(&path);
        return Ok(());
    }
    if arena_mode {
        let parse_diff = |s: &str| match s {
            "easy" => Difficulty::Easy,
            "hard" => Difficulty::Hard,
            _ => Difficulty::Medium,
        };
        let difficulty = parse_diff(&arena_difficulty);
        let difficulty_black = arena_difficulty_black.as_deref().map(parse_diff);
        arena::run(&arena::Config {
            games: arena_games.max(1),
            chess: arena_chess,
            checkers: arena_checkers,
            difficulty,
            difficulty_black,
            holds: arena_holds,
            draw_plies: arena_draw_plies,
            out: arena_out,
        });
        return Ok(());
    }
    if check_update {
        // Диагностика проверки обновлений без запуска окна.
        match updater::check_latest() {
            Ok(v) => println!("последний релиз: {v}; текущая версия: {}", updater::VERSION),
            Err(e) => println!("ошибка проверки: {e}"),
        }
        return Ok(());
    }
    if let Some(dir) = write_sounds {
        sound::write_defaults(&dir);
        return Ok(());
    }
    let shot_mode = shot_path.is_some();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("ShatMat — фигуры меняют природу")
        .with_inner_size([1240.0, 820.0])
        .with_min_inner_size([1040.0, 700.0]);

    if shot_mode {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }

    // Иконка окна: та же картинка, что вшита в exe (assets/icon-256.png).
    let icon = {
        let bytes: &[u8] = include_bytes!("../assets/icon-256.png");
        image::load_from_memory(bytes).ok().map(|img| {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            egui::IconData {
                rgba: rgba.into_raw(),
                width: w,
                height: h,
            }
        })
    };
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
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

/// Демонстрационная партия: компьютер играет сам с собой на стартовом экране.
fn lobby_demo_game() -> Game {
    let mut g = Game::new(20.0);
    g.set_twilight_by_moves(true);
    g.set_phase_len(Phase::Chess, 6);
    g.set_phase_len(Phase::Checkers, 4);
    g.mandatory_capture = true;
    g.crown_non_pawns = true;
    g.max_chain = true;
    g.king_escape = true;
    g.promotion_choice = false;
    g
}

/// Случайное зерно: каждая демонстрация складывается по-своему.
fn lobby_demo_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
}

/// Позиция «в разгаре» для снимка стартового экрана: несколько ходов подряд.
fn lobby_demo_preset(plies: usize) -> Game {
    let mut g = lobby_demo_game();
    let mut seed = 0x1234_5678_9ABC_DEF0u64;
    for _ in 0..plies {
        let moves = g.all_moves();
        if moves.is_empty() || g.is_over() {
            break;
        }
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let i = ((seed >> 33) as usize) % moves.len();
        g.apply(moves[i]);
        g.settle_twilight();
    }
    g
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

/// Читает картинку фигуры в RGBA: сначала SVG (вектор), затем PNG.
fn read_piece_rgba(
    dir: &std::path::Path,
    name: &str,
    target: u32,
) -> Option<(Vec<u8>, usize, usize)> {
    let svg_path = dir.join(format!("{name}.svg"));
    if svg_path.is_file() {
        if let Ok(bytes) = std::fs::read(&svg_path) {
            if let Some(out) = rasterize_svg(&bytes, target) {
                return Some(out);
            }
        }
    }
    let png_path = dir.join(format!("{name}.png"));
    let bytes = std::fs::read(&png_path).ok()?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    Some((img.into_raw(), w, h))
}

/// Затемняет светлую картинку фигуры для чёрной стороны.
///
/// Набору достаточно нарисовать только белый комплект: чёрные фигуры
/// получаются из них затемнением. Средние тона темнеют сильнее, тёмный
/// контур и яркие блики — умереннее, поэтому силуэт и внутренний рисунок
/// остаются читаемыми.
fn darken_rgba(rgba: &mut [u8]) {
    for px in rgba.chunks_exact_mut(4) {
        let lum = (px[0] as u32 * 30 + px[1] as u32 * 59 + px[2] as u32 * 11) / 100;
        let k = if lum > 200 {
            0.50
        } else if lum > 90 {
            0.34
        } else {
            0.55
        };
        px[0] = (px[0] as f32 * k) as u8;
        px[1] = (px[1] as f32 * k) as u8;
        px[2] = (px[2] as f32 * k) as u8;
    }
}

/// Загружает картинку фигуры: сначала пробует `.svg` (вектор), затем `.png`.
fn load_piece_textures(
    ctx: &egui::Context,
    dir: &std::path::Path,
    tag: &str,
    target: u32,
) -> PieceTextures {
    let mut out = PieceTextures::empty();
    // Текстура из готовых RGBA: цвет и белая маска для подсветки.
    let make = |name: &str, rgba: Vec<u8>, w: usize, h: usize| -> PieceArt {
        let color = egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba);
        let color_tex = ctx.load_texture(
            format!("piece_{tag}_{name}"),
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
            format!("piece_{tag}_{name}_mask"),
            mask,
            egui::TextureOptions::LINEAR,
        );
        PieceArt {
            color: color_tex,
            mask: mask_tex,
        }
    };
    // Фигура своего цвета; если для чёрных файла нет — берём белую и
    // затемняем, чтобы паку хватило нарисовать только белый комплект.
    let load = |prefix: &str, side: Side, base: &str| -> Option<PieceArt> {
        let name = format!("{prefix}_{base}");
        if let Some((rgba, w, h)) = read_piece_rgba(dir, &name, target) {
            return Some(make(&name, rgba, w, h));
        }
        if side == Side::Black {
            if let Some((mut rgba, w, h)) =
                read_piece_rgba(dir, &format!("white_{base}"), target)
            {
                darken_rgba(&mut rgba);
                return Some(make(&format!("black_{base}_auto"), rgba, w, h));
            }
        }
        None
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
            if let Some(art) = load(prefix, side, kind_file(kind)) {
                let arr = match side {
                    Side::White => &mut out.white,
                    Side::Black => &mut out.black,
                };
                arr[kind_index(kind)] = Some(art);
            }
        }
        let i = PieceTextures::side_index(side);
        out.checker[i] = load(prefix, side, "checker");
        out.damka[i] = load(prefix, side, "damka");
        out.royal[i] = load(prefix, side, "royal");
    }
    out
}

/// Папка паков рядом с игрой: `packs/<имя>/{pieces,sounds,music}`.
fn packs_dir() -> PathBuf {
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
    {
        let p = dir.join("packs");
        if p.is_dir() {
            return p;
        }
    }
    PathBuf::from("packs")
}

/// Пак: папка в packs/ с фигурами, звуками и/или музыкой.
#[derive(Clone)]
struct PackInfo {
    name: String,
    label: String,
    dir: PathBuf,
}

impl PackInfo {
    fn pieces(&self) -> Option<PathBuf> {
        let p = self.dir.join("pieces");
        p.is_dir().then_some(p)
    }
    fn sounds(&self) -> Option<PathBuf> {
        let p = self.dir.join("sounds");
        p.is_dir().then_some(p)
    }
    fn music(&self) -> Option<PathBuf> {
        let p = self.dir.join("music");
        p.is_dir().then_some(p)
    }
}

fn discover_packs() -> Vec<PackInfo> {
    let dir = packs_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<PackInfo> = Vec::new();
    for e in entries.flatten() {
        let path = e.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
        else {
            continue;
        };
        let info = PackInfo {
            label: pack_label(&path, &name),
            name,
            dir: path,
        };
        if info.pieces().is_none() && info.sounds().is_none() && info.music().is_none() {
            continue;
        }
        out.push(info);
    }
    out.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
    out
}

/// Подпись пака: `pack.txt` (строки `name.<язык>=…` или `name=…`),
/// `name.txt` (одна строка), иначе имя папки.
fn pack_label(dir: &std::path::Path, fallback: &str) -> String {
    let lang = i18n::current().code().to_string();
    if let Ok(text) = std::fs::read_to_string(dir.join("pack.txt")) {
        let mut plain: Option<String> = None;
        let mut localized: Option<String> = None;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let k = k.trim();
                let v = v.trim();
                if v.is_empty() {
                    continue;
                }
                if k == format!("name.{lang}") {
                    localized = Some(v.to_string());
                } else if k == "name" {
                    plain = Some(v.to_string());
                }
            }
        }
        if let Some(v) = localized.or(plain) {
            return v;
        }
    }
    if let Ok(text) = std::fs::read_to_string(dir.join("name.txt")) {
        if let Some(line) = text.lines().map(|l| l.trim()).find(|l| !l.is_empty()) {
            return line.to_string();
        }
    }
    fallback.to_string()
}

/// Выбор пака по категории: `pieces`, `sounds`, `music` из packs/current.txt.
fn saved_pack(kind: &str) -> Option<String> {
    let text = std::fs::read_to_string(packs_dir().join("current.txt")).ok()?;
    for line in text.lines() {
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == kind {
                let v = v.trim();
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

fn save_pack(kind: &str, name: Option<&str>) {
    let path = packs_dir().join("current.txt");
    let _ = std::fs::create_dir_all(packs_dir());
    let mut lines: Vec<String> = Vec::new();
    if let Ok(text) = std::fs::read_to_string(&path) {
        for line in text.lines() {
            if let Some((k, _)) = line.split_once('=') {
                if k.trim() == kind {
                    continue;
                }
            }
            if !line.trim().is_empty() {
                lines.push(line.to_string());
            }
        }
    }
    if let Some(n) = name {
        lines.push(format!("{kind}={n}"));
    }
    let _ = std::fs::write(&path, lines.join("\n"));
}

/// Каталог фигур набора: сначала пак, потом старая папка в pieces/.
fn preset_dir_for(packs: &[PackInfo], preset: Option<&str>) -> PathBuf {
    if let Some(name) = preset {
        if let Some(p) = packs.iter().find(|p| p.name == name) {
            if let Some(dir) = p.pieces() {
                return dir;
            }
        }
        let legacy = pieces_dir().join(name);
        if legacy.is_dir() {
            return legacy;
        }
    }
    pieces_dir()
}

/// Наборы фигур: подпапки в pieces/, где есть хотя бы король.
/// Возвращает (папка, подпись). Подпись берётся из name.txt, иначе — имя папки.
fn available_presets() -> Vec<(String, String)> {
    let dir = pieces_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<(String, String)> = Vec::new();
    for e in entries.flatten() {
        let path = e.path();
        if !path.is_dir() {
            continue;
        }
        let has = path.join("white_king.svg").is_file()
            || path.join("white_king.png").is_file();
        if !has {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            let label = std::fs::read_to_string(path.join("name.txt"))
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| name.to_string());
            out.push((name.to_string(), label));
        }
    }
    out.sort();
    out
}

/// Все доступные наборы фигур: старые папки в pieces/ плюс паки с фигурами.
fn all_piece_sets(packs: &[PackInfo]) -> Vec<(String, String)> {
    let mut out = available_presets();
    for p in packs {
        if p.pieces().is_some() && !out.iter().any(|(d, _)| *d == p.name) {
            out.push((p.name.clone(), p.label.clone()));
        }
    }
    out.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    out
}

/// Выбранный набор запоминается в pieces/current.txt (пусто — встроенный).
fn saved_preset() -> Option<String> {
    let path = pieces_dir().join("current.txt");
    let text = std::fs::read_to_string(path).ok()?;
    let name = text.trim().to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn save_preset(name: Option<&str>) {
    let path = pieces_dir().join("current.txt");
    let _ = std::fs::write(path, name.unwrap_or(""));
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
    /// Иконки фигур для шашек — всегда из стандартного набора,
    /// чтобы смена пресета не меняла шахматные обозначения на шашках.
    icon_tex: PieceTextures,
    /// Пульсация фигур ходящей стороны (по умолчанию включена).
    pulse_on: bool,
    /// Выбранный набор фигур (папка в pieces/ или пак) и список доступных.
    piece_preset: Option<String>,
    piece_presets: Vec<(String, String)>,
    /// Паки (фигуры/звуки/музыка) и выбранные наборы звука и музыки.
    packs: Vec<PackInfo>,
    sound_pack: Option<String>,
    music_pack: Option<String>,
    /// Звук победы играет один раз за партию.
    win_sound_played: bool,
    mode: Mode,
    difficulty: Difficulty,
    ai_wait: f32,
    ai_seed: u64,
    ai_preview: Option<Move>,
    ai_preview_phase: Option<Phase>,
    /// true — сумерки уже были: компьютер ждёт не меньше 3 с перед ходом.
    ai_after_twilight: bool,
    /// Демонстрационная партия ИИ против ИИ на стартовом экране.
    demo: Game,
    demo_seed: u64,
    demo_wait: f32,
    demo_rx: Option<std::sync::mpsc::Receiver<Option<Move>>>,
    demo_end_pause: f32,
    /// Пауза перед стартом демонстрации: даём развернуть окно.
    demo_delay: f32,
    /// Повтор финального хода демонстрации после конца партии.
    demo_replay: f32,
    /// Время начала демо-партии, её начальное зерно и признак «лог записан».
    demo_start: Option<String>,
    demo_seed0: u64,
    demo_logged: bool,
    shot: Option<Shot>,
    frames: u32,
    twilight_sounded: bool,
    replay: f32,
    logging_allowed: bool,
    log_enabled: bool,
    show_checker_icons: bool,
    mate_lines: bool,
    mate_escape: bool,
    /// Карточка «Правила партии» в панели: сводка раскрыта.
    rules_open: bool,
    /// Сколько строк было в журнале на прошлом кадре: новый ход сворачивает сводку.
    rules_seen_log: usize,
    font_faces: Vec<fonts::FontFace>,
    font_index: usize,
    game_start: Option<String>,
    log_written: bool,
    last_log: Option<PathBuf>,
    journal: bool,
    journal_rect: Rect,
    journal_text: String,
    /// Введённый код правил (поле в настройках).
    rules_code_input: String,
    /// Сообщение после применения кода: «Применено: …» или «Неверный код».
    rules_code_msg: String,
    review: bool,
    review_saved: Option<Game>,
    review_ply: usize,
    /// Окно «Логи партий»: список файлов из папки logs.
    logs_window: bool,
    logs_list: Vec<(PathBuf, String)>,
    logs_scroll: f32,
    logs_rect: Rect,
    /// Партия, к которой вернуться после просмотра чужого лога.
    review_return: Option<Game>,
    /// Сводка по открытому логу: ходы, взятия, смены фаз.
    review_stats: String,
    /// Список ходов для панели просмотра («1.1 e2-e4 · шахматы»).
    review_lines: Vec<String>,
    /// Какие строки — первые ходы новой природы (для подсветки).
    review_switch: Vec<bool>,
    /// Природа каждой строки списка (None — начальная позиция).
    review_phases: Vec<Option<Phase>>,
    /// Показывать в окне логов ещё и демонстрационные партии.
    logs_demo: bool,
    /// Заголовок панели в просмотре: итог партии или название лога.
    review_result: String,
    guide: bool,
    guide_kind: Kind,
    guide_rect: Rect,
    guide_demo: Option<GuideDemo>,
    guide_checkers: bool,
    guide_damka: bool,
    started: bool,
    /// Сетевой контекст: обнаружение, активный сеанс, события.
    net: Option<net::Net>,
    /// Запрос на поднятие сети из интерфейса (выполняется в update()).
    net_open_requested: bool,
    /// Открыто ли окно сети.
    net_window: bool,
    net_rect: Rect,
    /// Ввод адреса или ID соперника.
    net_addr_input: String,
    /// Приветствие — единственное сообщение до начала партии.
    net_greeting: String,
    /// Сообщения чата: (текст, наше, время). Живут 60 секунд, в логи не пишутся.
    net_chat: Vec<(String, bool, std::time::Instant)>,
    net_chat_input: String,
    /// Прокрутка содержимого сетевого окна, когда оно не влезает.
    net_scroll: f32,
    /// Прокрутка и высота содержимого окна настроек (для скроллбара).
    settings_scroll: f32,
    settings_content_h: f32,
    /// Отображаемый код приглашения (выделяемое поле).
    net_code_view: String,
    /// Реконнект: адрес соперника, признак ожидания продолжения и таймер повтора.
    net_last_addr: Option<std::net::SocketAddr>,
    net_resume: bool,
    net_retry: f32,
    /// Проверка обновлений: фоновый запрос и его состояние.
    update_check: Option<std::sync::mpsc::Receiver<updater::CheckResult>>,
    update_state: updater::UpdateState,
    /// Наша сторона в сетевой партии.
    net_side: Option<Side>,
    /// Ход, применённый локально, но ещё не отправленный (ждёт превращения).
    net_pending_move: Option<Move>,
    /// Ход соперника, пришедший во время сумерек: применяется после смены фазы.
    net_pending_apply: Option<(Move, Option<Kind>)>,
    /// Правила хоста, применяемые при старте сетевой партии.
    net_rules: Option<rulecode::Rules>,
    /// Обратный отсчёт после потери связи, с.
    net_wait: f32,
    /// Последнее сообщение о состоянии сети в окне.
    net_status: String,
    /// Хеш соперника, пришедший раньше отложенного сумерками хода.
    net_pending_hash: Option<u64>,
    /// Режим «по сети» включён в настройках — до этого чип скрыт.
    net_enabled: bool,
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
        let packs = discover_packs();
        let piece_presets = all_piece_sets(&packs);
        let piece_preset = saved_preset().filter(|n| piece_presets.iter().any(|(d, _)| d == n));
        let sound_pack = saved_pack("sounds")
            .filter(|n| packs.iter().any(|p| p.name == *n && p.sounds().is_some()));
        let music_pack = saved_pack("music")
            .filter(|n| packs.iter().any(|p| p.name == *n && p.music().is_some()));
        if let Some(dir) = sound_pack
            .as_ref()
            .and_then(|n| packs.iter().find(|p| p.name == *n))
            .and_then(|p| p.sounds())
        {
            sound::set_sounds_root(Some(dir));
        }
        if let Some(dir) = music_pack
            .as_ref()
            .and_then(|n| packs.iter().find(|p| p.name == *n))
            .and_then(|p| p.music())
        {
            music::set_music_root(Some(dir));
        }
        let piece_tex = load_piece_textures(
            &cc.egui_ctx,
            &preset_dir_for(&packs, piece_preset.as_deref()),
            piece_preset.as_deref().unwrap_or("builtin"),
            512,
        );
        // Иконки на шашках берём из корня pieces/ (стандартный набор), 128 px достаточно.
        let icon_tex = load_piece_textures(&cc.egui_ctx, &pieces_dir(), "stdicons", 128);
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
            icon_tex,
            pulse_on: true,
            piece_preset,
            piece_presets,
            packs,
            sound_pack,
            music_pack,
            win_sound_played: false,
            // По умолчанию играем против компьютера; демо-сценарии
            // остаются за двумя людьми, чтобы не мешать показам.
            mode: if demo.is_some() {
                Mode::Hotseat
            } else {
                Mode::Computer
            },
            difficulty: Difficulty::Medium,
            ai_wait: 0.6,
            ai_seed: 0x9E37_79B9_7F4A_7C15,
            ai_preview: None,
            ai_preview_phase: None,
            ai_after_twilight: false,
            demo: if demo.as_deref() == Some("lobby") {
                lobby_demo_preset(16)
            } else {
                lobby_demo_game()
            },
            demo_seed: lobby_demo_seed(),
            demo_wait: 0.7,
            demo_rx: None,
            demo_end_pause: 0.0,
            demo_delay: 3.0,
            demo_replay: 0.0,
            demo_start: None,
            demo_seed0: 0,
            demo_logged: false,
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
            rules_open: true,
            rules_seen_log: 0,
            font_faces,
            font_index,
            game_start,
            log_written: false,
            last_log: None,
            journal: false,
            journal_rect: Rect::NOTHING,
            journal_text: String::new(),
            rules_code_input: String::new(),
            rules_code_msg: String::new(),
            review: false,
            review_saved: None,
            review_ply: 0,
            logs_window: false,
            logs_list: Vec::new(),
            logs_scroll: 0.0,
            logs_rect: Rect::NOTHING,
            review_return: None,
            review_stats: String::new(),
            review_lines: Vec::new(),
            review_switch: Vec::new(),
            review_phases: Vec::new(),
            logs_demo: false,
            review_result: String::new(),
            guide: false,
            guide_kind: Kind::Pawn,
            guide_rect: Rect::NOTHING,
            guide_demo: None,
            guide_checkers: false,
            guide_damka: false,
            started: demo.is_some()
                && !demo
                    .as_deref()
                    .map(|d| d.starts_with("lobby"))
                    .unwrap_or(false),
            // --- Сеть ---
            // Снимок окна сети требует живого сокета: в демо поднимаем сразу.
    net: if demo.as_deref() == Some("net") {
        net::Net::new()
    } else {
        None
    },
    net_open_requested: false,
            net_window: demo.as_deref() == Some("net"),
            net_rect: Rect::NOTHING,
            net_addr_input: String::new(),
            net_greeting: String::new(),
            net_chat: Vec::new(),
            net_chat_input: String::new(),
    net_scroll: 0.0,
    settings_scroll: 0.0,
    settings_content_h: 0.0,
    net_code_view: String::new(),
    net_last_addr: None,
    net_resume: false,
    net_retry: 0.0,
    update_check: None,
    update_state: updater::UpdateState::Idle,
            net_side: None,
            net_pending_move: None,
            net_pending_apply: None,
            net_rules: None,
            net_wait: 0.0,
            net_status: String::new(),
            net_pending_hash: None,
            net_enabled: matches!(demo.as_deref(), Some("net") | Some("lobby_net")),
        };
        // Демонстрация: запоминаем стартовое зерно — по нему партия
        // воспроизводится один в один.
        app.demo_seed0 = app.demo_seed;
        app.demo_start = Some(logging::now_string());
        if demo.as_deref() == Some("journal") {
            app.journal_text = app.build_journal_text();
            app.journal = true;
        }
        if demo.as_deref() == Some("lobby_end") {
            // Демо-финал: проверка логов демонстрации и вида законченной партии.
            app.demo.status = game::Status::Win {
                side: Side::White,
                reason: "мат".into(),
            };
            app.demo_delay = 0.0;
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
            // Стрелки листают партию: влево/вверх — назад, вправо/вниз —
            // вперёд; Home/End — к началу и к концу, Esc — закрыть.
            let back = ctx.input(|i| {
                i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::ArrowUp)
            });
            let forward = ctx.input(|i| {
                i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::ArrowDown)
            });
            let home = ctx.input(|i| i.key_pressed(egui::Key::Home));
            let end = ctx.input(|i| i.key_pressed(egui::Key::End));
            let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
            let max_ply = self
                .review_saved
                .as_ref()
                .map(|g| g.snapshots.len().saturating_sub(1))
                .unwrap_or(0);
            if esc {
                self.exit_review();
                return;
            }
            if home && self.review_ply > 0 {
                self.set_review_ply(0);
            }
            if end && self.review_ply < max_ply {
                self.set_review_ply(max_ply);
            }
            if back && self.review_ply > 0 {
                self.set_review_ply(self.review_ply - 1);
            }
            if forward && self.review_ply < max_ply {
                self.set_review_ply(self.review_ply + 1);
            }
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
                } else if self.net_window {
                    self.net_window = false;
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
            } else if self.net_window {
                self.net_window = false;
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
        if space && !self.rules && !self.settings && !self.journal && !self.net_window {
            self.net_set_pause(!self.game.paused);
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
                game::Status::Draw { reason } => i18n::logf(
                    "ничья — {reason}",
                    &[("reason", &i18n::log_owned(reason.clone()))],
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
            Mode::Network => i18n::log("по сети").to_string(),
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
        self.win_sound_played = false;
        self.started = true;
        self.game.paused = false;
        // В лобби памятка раскрыта, а в партии по умолчанию свёрнута:
        // журнал ходов важнее, правила можно раскрыть ссылкой.
        self.rules_open = false;
        if self.logging_allowed && self.log_enabled && self.game_start.is_none() {
            self.game_start = Some(logging::now_string());
            self.log_written = false;
        }
    }

    fn new_game(&mut self) {
        // сетевая партия синхронна — пересоздавать её локально нельзя
        if self.net_active() {
            return;
        }
        self.finalize_log("прервана (новая партия)");
        self.win_sound_played = false;
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
        self.rules_open = !self.started;
        self.ai_preview = None;
        self.ai_preview_phase = None;
        self.ai_after_twilight = false;
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

    /// Демонстрация на стартовом экране: ИИ играет сам с собой, без звука.
    /// Мусор не считаем, пока окно не активно или открыт диалог.
    fn update_demo(&mut self, dt: f32, ctx: &egui::Context) {
        if self.started || self.review || self.guide {
            return;
        }
        // Первые секунды после запуска демонстрацию не крутим: окно часто
        // открывается не на весь экран, и его успевают развернуть.
        if self.demo_delay > 0.0 {
            self.demo_delay = (self.demo_delay - dt).max(0.0);
            return;
        }
        if self.settings || self.rules || self.net_window || self.journal {
            return;
        }
        if !ctx.input(|i| i.focused) && self.shot.is_none() {
            return;
        }
        self.demo.tick(dt);
        if self.demo.is_over() {
            // Отдельный лог демо-партии: разбирать такие партии так же
            // удобно, как настоящие.
            if !self.demo_logged {
                self.demo_logged = true;
                let code = rulecode::pretty(&rulecode::encode(&rulecode::Rules::from_game(&self.demo)));
                let start = self
                    .demo_start
                    .clone()
                    .unwrap_or_else(logging::now_string);
                logging::write_demo_log(&self.demo, &start, &code, self.demo_seed0);
            }
            // Концовку показываем подробнее: последний ход проигрывается
            // дважды (цикл 4 с), и только потом начинается новая партия.
            self.demo_end_pause += dt;
            self.demo_replay += dt;
            if self.demo_end_pause > 8.8 {
                self.demo = lobby_demo_game();
                self.demo_seed = lobby_demo_seed();
                self.demo_seed0 = self.demo_seed;
                self.demo_start = Some(logging::now_string());
                self.demo_logged = false;
                self.demo_wait = 0.5;
                self.demo_rx = None;
                self.demo_end_pause = 0.0;
                self.demo_replay = 0.0;
            }
            return;
        }
        if let Some(rx) = self.demo_rx.take() {
            match rx.try_recv() {
                Ok(mv) => {
                    if let Some(mv) = mv {
                        self.demo.apply(mv);
                    }
                    // Концовку (мало фигур) показываем быстрее, чтобы демо
                    // не вязло в мёртвых позициях; шахматная часть при полной
                    // доске — медленнее: позицию надо успеть разглядеть.
                    let pieces = self
                        .demo
                        .board
                        .iter()
                        .flatten()
                        .filter(|c| c.is_some())
                        .count();
                    self.demo_wait = if pieces <= 6 {
                        0.7
                    } else if self.demo.phase == Phase::Chess {
                        2.4
                    } else {
                        1.1
                    };
                    self.demo_end_pause = 0.0;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    self.demo_rx = Some(rx);
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.demo_wait = 0.4;
                }
            }
        }
        self.demo_wait -= dt;
        if self.demo_wait <= 0.0 && !self.demo.is_locked() && self.demo_rx.is_none() {
            let mut g = self.demo.clone();
            g.trim_for_search();
            let mut seed = self.demo_seed;
            self.demo_seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let mv = ai::best_move(&g, Difficulty::Medium, &mut seed);
                let _ = tx.send(mv);
            });
            self.demo_rx = Some(rx);
        }
    }

    /// Компьютер играет чёрными: 2–3 секунды показывает цель, затем ходит.
    /// После сумерек пауза не меньше 3 секунд.
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
        if self.game.trans > 0.0 {
            // Сумерки: прежний выбор компьютера больше не годится,
            // а на ход после смены игры ему нужно не меньше 3 секунд.
            self.ai_after_twilight = true;
            self.ai_preview = None;
            self.ai_preview_phase = None;
            self.ai_wait = 0.0;
        }
        if self.rules
            || self.settings
            || self.game.paused
            || self.game.is_over()
            || self.game.trans > 0.0
            || self.game.twilight_pause()
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
                self.ai_wait = if self.ai_after_twilight {
                    // После сумерек — пауза не меньше 3 секунд.
                    3.0 + frac
                } else {
                    2.0 + frac
                };
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
                self.ai_after_twilight = false;
            } else {
                self.ai_wait = 0.3;
            }
        } else {
            self.ai_wait = 0.6;
        }
    }

    // ------------------------- сеть -------------------------

    fn net_session(&self) -> Option<net::SessionInfo> {
        self.net.as_ref().and_then(|n| n.session())
    }

    fn net_active(&self) -> bool {
        self.net_session().is_some()
    }

    fn net_send(&self, msg: net::OutMsg) {
        if let Some(n) = &self.net {
            n.send(msg);
        }
    }

    /// Применить ход соперника: сумерки откладывают, превращение доводит.
    fn net_apply_move(&mut self, from: Square, to: Square, captured: Option<Square>, promo: Option<Kind>) {
        let mv = Move { from, to, captured };
        if self.game.trans > 0.0 {
            self.net_pending_apply = Some((mv, promo));
            return;
        }
        let mark = self.sound_mark();
        self.game.apply(mv);
        if let Some(kind) = promo {
            if self.game.pending_promotion.is_some() {
                self.game.choose_promotion(kind);
            }
        }
        self.play_sound_since(mark);
    }

    /// Пауза в сетевой партии: свой флаг и сообщение сопернику.
    fn net_set_pause(&mut self, on: bool) {
        self.game.paused = on;
        if self.net_active() {
            self.net_send(net::OutMsg::Pause(on));
        }
    }

    /// Сверить позицию с соперником; расхождение останавливает партию.
    fn net_check_hash(&mut self, hash: u64) {
        let my_hash = net::position_hash(&self.game);
        if my_hash != hash {
            self.net_status =
                i18n::tr("Позиции разошлись — партия остановлена").to_string();
            self.game.paused = true;
            self.net_close("позиции разошлись");
            self.finalize_log("прервана (позиции разошлись)");
        }
    }

    /// Отправить ход и хеш позиции после него.
    fn net_send_move(&mut self, mv: Move, promo: Option<Kind>) {
        if !self.net_active() {
            return;
        }
        self.net_send(net::OutMsg::Move {
            from: mv.from,
            to: mv.to,
            captured: mv.captured,
            promo,
        });
        let hash = net::position_hash(&self.game);
        self.net_send(net::OutMsg::Hash { ply: self.game.ply, hash });
    }

    /// После локального клика: если применён ход — отправить его.
    /// `before` — позиция last_move до клика, чтобы не отправить старый ход.
    fn net_after_local_move(&mut self, before: Option<(Square, Square)>) {
        if !self.net_active() {
            return;
        }
        if self.game.pending_promotion.is_some() {
            // превращение ждёт выбора фигуры — ход уйдёт после выбора
            if let Some((from, to)) = self.game.last_move {
                if before != Some((from, to)) {
                    let captured = self.game.last_capture.map(|(sq, _)| sq);
                    self.net_pending_move = Some(Move { from, to, captured });
                }
            }
        } else if self.game.last_move != before {
            if let Some((from, to)) = self.game.last_move {
                let captured = self.game.last_capture.map(|(sq, _)| sq);
                // пешка дошла до последней горизонтали без диалога — это ферзь
                let promo = self.game.history.last().and_then(|r| {
                    if r.from == from && r.to == to && r.kind == Kind::Pawn {
                        Some(Kind::Queen)
                    } else {
                        None
                    }
                });
                self.net_send_move(Move { from, to, captured }, promo);
            }
        }
    }

    /// Новая сетевая партия: правила хоста + seed.
    fn net_start_game(&mut self, seed: u64) {
        let smooth = self.game.move_anim_secs;
        let mut g = Game::new(self.game.chess_len);
        if let Some(rules) = self.net_rules.take() {
            rules.apply(&mut g);
        }
        g.seed_rng(seed);
        g.refresh_phase_timer();
        g.move_anim_secs = smooth;
        self.game = g;
        self.started = true;
        self.game.paused = false;
        self.net_pending_move = None;
        self.net_pending_apply = None;
        self.twilight_sounded = false;
        self.replay = 0.0;
        self.net_status = i18n::tr("Партия началась").to_string();
    }

    /// Закрыть сеанс: прощание, сброс сетевого состояния, возврат в «вдвоём».
    fn net_close(&mut self, reason: &str) {
        if let Some(net) = &mut self.net {
            net.close_session(reason);
        }
        self.net_side = None;
        self.net_pending_move = None;
        self.net_pending_apply = None;
        self.net_pending_hash = None;
        self.net_rules = None;
        self.net_wait = 0.0;
        self.mode = Mode::Hotseat;
        self.game.paused = false;
    }

    /// События сети за кадр.
    fn net_poll(&mut self) {
        let Some(net) = &self.net else { return };
        let events = net.poll();
        for ev in events {
            match ev {
                net::NetEvent::Connected { role, peer_id, rules, text } => {
                    self.net_status = i18n::trf(
                        "Соединение установлено: {id} · правила {rules}",
                        &[
                            ("id", &net::display_id(&peer_id)),
                            ("rules", &rulecode::encode_compact(&rules)),
                        ],
                    );
                    self.net_side = Some(role.side());
                    self.mode = Mode::Network;
                    self.started = true;
                    self.game.paused = false;
                    self.net_rules = Some(rules);
                    if !text.is_empty() {
                        self.net_chat.push((text, false, std::time::Instant::now()));
                    }
                    if role == net::Role::Host {
                        if self.net_resume {
                            // Реконнект: хост пересылает состояние — партия
                            // продолжается с того же места.
                            self.net_resume = false;
                            let data = self.game.export_state();
                            self.net_send(net::OutMsg::Sync(data));
                            self.game.paused = false;
                            self.net_wait = 0.0;
                            self.net_status =
                                i18n::tr("Соперник вернулся — партия продолжается").to_string();
                        } else {
                            let seed = random_seed();
                            self.net_send(net::OutMsg::Seed(seed));
                            self.net_start_game(seed);
                        }
                    } else if self.net_resume {
                        // Гость ждёт состояние от хоста; обычный старт
                        // придёт сообщением Seed.
                        self.game.paused = true;
                    }
                }
                net::NetEvent::Seed(seed) => {
                    if self.mode == Mode::Network {
                        self.net_resume = false;
                        self.net_wait = 0.0;
                        self.net_start_game(seed);
                    }
                }
                net::NetEvent::Sync(data) => {
                    if self.mode == Mode::Network {
                        if self.game.import_state(&data) {
                            self.net_resume = false;
                            self.net_wait = 0.0;
                            self.game.paused = false;
                            self.twilight_sounded = false;
                            self.net_status =
                                i18n::tr("Соперник вернулся — партия продолжается").to_string();
                        } else {
                            self.net_status =
                                i18n::tr("Не удалось продолжить партию").to_string();
                        }
                    }
                }
                net::NetEvent::Ping => {
                    // внутренний пинг: живость связи проверяет session_loop
                }
                net::NetEvent::Move { from, to, captured, promo } => {
                    if self.mode == Mode::Network {
                        self.net_apply_move(from, to, captured, promo);
                    }
                }
                net::NetEvent::Hash { ply: _, hash } => {
                    if self.mode == Mode::Network {
                        if self.net_pending_apply.is_some() {
                            // ход ещё в очереди сумерек — сравним после его применения
                            self.net_pending_hash = Some(hash);
                        } else {
                            self.net_check_hash(hash);
                        }
                    }
                }
                net::NetEvent::Pause(on) => {
                    if self.mode == Mode::Network {
                        self.game.paused = on;
                    }
                }
                net::NetEvent::Chat(text) => {
                    if self.mode == Mode::Network {
                        self.net_chat.push((text, false, std::time::Instant::now()));
                    }
                }
                net::NetEvent::Bye(reason) => {
                    if self.mode == Mode::Network {
                        self.net_status = i18n::trf(
                            "Соединение закрыто: {reason}",
                            &[("reason", &reason)],
                        );
                        self.net_close("");
                        self.finalize_log("прервана (соперник закрыл соединение)");
                    }
                }
                net::NetEvent::ChainEnd => {
                    if self.mode == Mode::Network {
                        self.game.end_chain();
                    }
                }
                net::NetEvent::PromoFree { sq, kind } => {
                    if self.mode == Mode::Network && self.game.pending_promotion == Some(sq) {
                        self.game.choose_promotion(kind);
                    }
                }
                net::NetEvent::Disconnected(reason) => {
                    if self.mode == Mode::Network {
                        self.net_last_addr = self.net_session().map(|s| s.peer_addr);
                        if let Some(n) = self.net.as_mut() {
                            n.mark_dropped();
                        }
                        self.net_status = i18n::trf(
                            "Связь потеряна: {reason}",
                            &[("reason", &reason)],
                        );
                        self.net_resume = true;
                        self.net_retry = 3.0;
                        self.net_wait = 60.0;
                        self.game.paused = true;
                        self.finalize_log("прервана (связь потеряна)");
                    }
                }
                net::NetEvent::Failed(msg) => {
                    self.net_status = msg;
                }
            }
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
        let (lines, switches, phases) = Self::build_review_lines(&saved);
        self.review_lines = lines;
        self.review_switch = switches;
        self.review_phases = phases;
        self.review_result = match &saved.status {
            game::Status::Win { side, reason } => i18n::trf(
                "ПОБЕДА {side}",
                &[("side", &side.name_ru().to_uppercase())],
            ),
            game::Status::Draw { .. } => i18n::tr("НИЧЬЯ").to_string(),
            _ => i18n::tr("ПРОСМОТР ПАРТИИ").to_string(),
        };
        self.review_saved = Some(saved);
        self.review_stats.clear();
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
        self.review_saved.take();
        if let Some(back) = self.review_return.take() {
            // Просматривали чужой лог — возвращаемся к своей партии.
            self.game = back;
        }
        self.review = false;
        self.review_ply = 0;
    }

    /// Открыть список логов из папки logs.
    fn open_logs(&mut self) {
        self.logs_list = replay::list_logs(self.logs_demo);
        self.logs_window = true;
        self.logs_scroll = 0.0;
    }

    /// Открыть лог в режиме просмотра — как разбор своей партии.
    fn open_log_file(&mut self, path: PathBuf) {
        let Some(mut view) = replay::load_game(&path) else {
            return;
        };
        if self.review {
            self.exit_review();
        }
        self.review_return = Some(self.game.clone());
        view.status = game::Status::Playing;
        view.selected = None;
        view.trans = 0.0;
        view.paused = false;
        let mut chess_plies = 0usize;
        let mut check_plies = 0usize;
        let mut caps_chess = 0usize;
        let mut caps_check = 0usize;
        let mut flips = 0usize;
        let mut prev: Option<Phase> = None;
        for r in &view.history {
            if r.phase == Phase::Chess {
                chess_plies += 1;
            } else {
                check_plies += 1;
            }
            if r.captured.is_some() {
                if r.phase == Phase::Chess {
                    caps_chess += 1;
                } else {
                    caps_check += 1;
                }
            }
            if prev.is_some() && prev != Some(r.phase) {
                flips += 1;
            }
            prev = Some(r.phase);
        }
        let diff = view.material_diff();
        self.review_stats = i18n::trf(
            "ходов {n} · шахматы {c}, шашки {k} · взятия {cc}/{ck} · смен {f} · материал {m}",
            &[
                ("n", &view.history.len().to_string()),
                ("c", &chess_plies.to_string()),
                ("k", &check_plies.to_string()),
                ("cc", &caps_chess.to_string()),
                ("ck", &caps_check.to_string()),
                ("f", &flips.to_string()),
                ("m", &format!("{:+}", diff)),
            ],
        );
        let (lines, switches, phases) = Self::build_review_lines(&view);
        self.review_lines = lines;
        self.review_switch = switches;
        self.review_phases = phases;
        self.review_result = i18n::tr("ПРОСМОТР ЛОГА").to_string();
        self.review_saved = Some(view.clone());
        self.review = true;
        self.game = view;
        self.review_ply = 0;
        self.logs_window = false;
        // Лог открывается в просмотре — окна настроек и журнала больше
        // не нужны, иначе они закрывают область разбора.
        self.settings = false;
        self.rules = false;
        self.journal = false;
        self.net_window = false;
        self.ai_preview = None;
        self.set_review_ply(0);
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

    /// Максимальный полуход в просмотре.
    fn review_max(&self) -> usize {
        self.review_saved
            .as_ref()
            .map(|g| g.snapshots.len().saturating_sub(1))
            .unwrap_or(0)
    }

    /// Строки ходов для панели просмотра: «полуход.ход» впереди, чтобы
    /// различать ход и полуход, и природа хода — как в файле лога:
    /// «1.1 e2-e4 · шахматы», «7.4 h2-g3 · шашки». Ход, сменивший природу,
    /// помечается стрелкой.
    fn build_review_lines(saved: &Game) -> (Vec<String>, Vec<bool>, Vec<Option<Phase>>) {
        let mut out = Vec::with_capacity(saved.history.len() + 1);
        let mut switches = Vec::with_capacity(saved.history.len() + 1);
        let mut phases = Vec::with_capacity(saved.history.len() + 1);
        out.push(i18n::tr("0. Начальная позиция").to_string());
        switches.push(false);
        phases.push(None);
        let mut full_move = 0usize;
        let mut prev_phase: Option<Phase> = None;
        for (i, r) in saved.history.iter().enumerate() {
            let sep = if r.captured.is_some() { "x" } else { "-" };
            let mv = format!(
                "{}{}{}",
                game::sq_name(r.from),
                sep,
                game::sq_name(r.to)
            );
            if r.side == Side::White {
                full_move += 1;
            }
            let ply = i + 1;
            let switched = prev_phase.is_some() && prev_phase != Some(r.phase);
            out.push(format!("{ply}.{full_move} {mv}"));
            switches.push(switched);
            phases.push(Some(r.phase));
            prev_phase = Some(r.phase);
        }
        (out, switches, phases)
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
        if self.logs_window {
            if !self.logs_rect.contains(pos) {
                self.logs_window = false;
            }
            return;
        }
        if self.settings {
            if !self.settings_rect.contains(pos) {
                self.settings = false;
            }
            return;
        }
        if self.net_window {
            if !self.net_rect.contains(pos) {
                self.net_window = false;
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
            // «Партия ещё не начата»: клик по полю — тот же старт, что «Играть».
            if layout.board.contains(pos) {
                self.start_game();
            }
            return;
        }
        if self.game.paused {
            let pill = Rect::from_center_size(layout.board.center(), Vec2::new(470.0, 104.0));
            if pill.contains(pos) {
                self.net_set_pause(false);
            }
            return;
        }
        if self.game.is_over() {
            return;
        }
        if self.mode == Mode::Computer && self.game.turn == Side::Black {
            return;
        }
        // в сетевой партии ходит только своя сторона
        if self.mode == Mode::Network && self.net_side != Some(self.game.turn) {
            return;
        }
        if let Some(sq) = layout.square_at(pos) {
            let before = self.game.last_move;
            let mark = self.sound_mark();
            self.game.click(sq);
            self.play_sound_since(mark);
            self.net_after_local_move(before);
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
                if !self.started && !self.review {
                    self.paint_start_frost(ui, &painter, layout);
                }
                self.paint_panel(ui, &painter, layout);
                self.paint_promotion(ui, &painter, layout);
                if self.settings {
                    self.paint_settings(ui, &painter, layout);
                }
                if self.rules && !self.settings {
                    self.paint_rules(ui, ctx, &painter, layout);
                }
                if self.net_window {
                    self.paint_net(ui, &painter, layout);
                }
                if self.journal {
                    self.paint_journal(ui, &painter, layout);
                }
                if self.logs_window {
                    self.paint_logs(ui, &painter, layout);
                }
                if self.guide {
                    self.paint_guide(ui, &painter, layout);
                }
            });
    }

    // ------------------------- фон -------------------------

    fn paint_background(&self, ctx: &egui::Context, painter: &egui::Painter, layout: &Layout) {
        // Фон подстраивается под то, что на доске: до старта — под демонстрацию.
        let (phase, trans) = if self.started || self.review {
            (self.game.phase, self.game.trans)
        } else {
            (self.demo.phase, self.demo.trans)
        };
        let pal = Palette::for_phase(phase);
        let next = Palette::for_phase(phase.other());
        let m = smooth(trans);
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
            "ShatMat",
            28.0,
            pal.text,
            3.5,
        );
        text(
            painter,
            Pos2::new(x + 1.0, layout.header.top() + 57.0),
            Align2::LEFT_TOP,
            "фигуры меняют природу",
            12.0,
            alpha(pal.text_dim, 0.95),
        );
    }

    // ------------------------- доска -------------------------

    /// Активная анимация хода: (откуда, куда, прогресс 0..1).
    fn active_move_anim(&self, g: &Game) -> Option<(Square, Square, f32)> {
        let (from, to) = g.last_move?;
        if g.is_over() {
            let t = if self.started { self.replay } else { self.demo_replay };
            let p = ((t % 4.0) / 2.8).clamp(0.0, 1.0);
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
        // До старта на доске идёт демонстрационная партия ИИ против ИИ.
        // В просмотре показываем разбираемую партию, а не демонстрацию.
        let g = if self.started || self.review {
            &self.game
        } else {
            &self.demo
        };
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
            if !g.is_locked() && !g.twilight_pause() && !self.rules {
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

        // сумеречная дымка — под фигурами; без надписи, переход виден по анимации
        if g.trans > 0.0 {
            let a = bell(g.trans);
            painter.rect_filled(
                layout.board,
                8.0,
                alpha(Color32::from_rgb(48, 30, 62), 0.10 * a),
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
                    if self.pulse_on
                        && piece.side == g.turn
                        && g.trans == 0.0
                        && !g.is_over()
                        && g.pending_promotion.is_none()
                        && !g.moves_for(sq).is_empty()
                    {
                        let pulse = 0.5 + 0.5 * (self.time * 3.6 + seed * 0.35).sin();
                        if phase == Phase::Checkers {
                            let glow = if piece.side == Side::White {
                                Color32::from_rgb(255, 248, 236)
                            } else {
                                Color32::from_rgb(104, 108, 118)
                            };
                            art::draw_piece_glow(
                                painter,
                                center,
                                size,
                                piece.kind,
                                piece.side,
                                true,
                                glow,
                                0.18 + 0.28 * pulse,
                            );
                        } else if let Some(art) = self.piece_tex.get(piece.side, piece.kind) {
                            // «Дым» вокруг фигуры того, кто ходит: маска крупнее
                            // силуэта, привязанная к низу фигуры. Без привязки
                            // к низу ореол вылезает под фигурой светлым пятном.
                            let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
                            let base = center.y + size * 0.5;
                            let (col, a1, a2) = if piece.side == Side::White {
                                (
                                    Color32::from_rgb(255, 249, 238),
                                    0.14 + 0.22 * pulse,
                                    0.07 + 0.12 * pulse,
                                )
                            } else {
                                (
                                    Color32::from_rgb(74, 78, 90),
                                    0.16 + 0.22 * pulse,
                                    0.08 + 0.12 * pulse,
                                )
                            };
                            // Тень в пределах клетки: чуть больше силуэта, не больше.
                            for (scale, al) in [(1.05_f32, a2), (1.015_f32, a1)] {
                                let h = size * scale;
                                let r = Rect::from_min_max(
                                    Pos2::new(center.x - h * 0.5, base - h),
                                    Pos2::new(center.x + h * 0.5, base),
                                );
                                painter.image(art.mask.id(), r, uv, alpha(col, al));
                            }
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
                            None,
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
                            None,
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
                            None,
                        );
                    }

                    // Король в шашках: красное пульсирующее кольцо поверх шашки —
                    // его можно взять, и это сразу поражение.
                    if piece.kind == Kind::King && !g.is_over() && g.trans == 0.0 {
                        if phase == Phase::Checkers {
                            // Тревожное кольцо — только когда короля реально
                            // бьют следующим ходом; иначе едва заметное.
                            let a = if g.king_under_attack(piece.side) {
                                1.0
                            } else {
                                0.30
                            };
                            art::draw_king_ring(painter, center, size, a, self.time);
                        } else if g.king_danger_after_flip(piece.side) {
                            // Шахматы, но смена фаз вот-вот: король окажется
                            // под боем — предупреждаем заранее.
                            art::draw_king_ring(painter, center, size, 0.9, self.time);
                        }
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
                None,
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
                            // Отступаем от центра к краю силуэта: линия выходит
                            // из-под фигуры, а не протыкает её насквозь.
                            let start_dir = (kc - fc).normalized();
                            let fs = fc + start_dir * layout.cell * 0.30;
                            // Конец — у края короля: пунктир не должен просвечивать
                            // сквозь его силуэт.
                            let ke = kc - start_dir * layout.cell * 0.30;
                            let path: Vec<Pos2> = if kind == Kind::Knight {
                                let corner = if (from.0 as i32 - king.0 as i32).abs() == 1 {
                                    (king.0, from.1)
                                } else {
                                    (from.0, king.1)
                                };
                                vec![fs, layout.square_rect(corner).center(), ke]
                            } else {
                                vec![fs, ke]
                            };
                            for pair in path.windows(2) {
                                // Пунктир: тёмная подложка для контраста и цветной штрих.
                                let seg = [pair[0], pair[1]];
                                painter.extend(egui::Shape::dashed_line(
                                    &seg,
                                    Stroke::new(6.0_f32, Color32::from_black_alpha(70)),
                                    9.0,
                                    6.0,
                                ));
                                painter.extend(egui::Shape::dashed_line(
                                    &seg,
                                    Stroke::new(2.6_f32, alpha(pal.danger, 0.40 + 0.25 * pulse)),
                                    9.0,
                                    6.0,
                                ));
                            }
                            painter.circle_stroke(
                                fc,
                                layout.cell * 0.36,
                                Stroke::new(1.6_f32, alpha(pal.danger, 0.70)),
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
                            let dir = (sc - fc).normalized();
                            let fs = fc + dir * layout.cell * 0.28;
                            let seg = [fs, sc];
                            painter.extend(egui::Shape::dashed_line(
                                &seg,
                                Stroke::new(5.0_f32, Color32::from_black_alpha(60)),
                                7.0,
                                5.0,
                            ));
                            painter.extend(egui::Shape::dashed_line(
                                &seg,
                                Stroke::new(2.0_f32, alpha(yellow, 0.55 + 0.25 * pulse)),
                                7.0,
                                5.0,
                            ));
                            painter.circle_stroke(
                                sc,
                                layout.cell * 0.18,
                                Stroke::new(1.4_f32, alpha(yellow, 0.65)),
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
        _hole: Option<Color32>,
    ) {
        if a <= 0.01 {
            return;
        }
        if !checkers_form {
            if let Some(art) = self.piece_tex.get(side, kind) {
                // Пропорции фигур заложены в самих SVG (пешка меньше),
                // здесь — общий масштаб, чтобы фигуры не упирались в клетку.
                let s = size * 0.82;
                let rect = Rect::from_center_size(center, Vec2::splat(s));
                let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
                painter.image(art.color.id(), rect, uv, alpha(Color32::WHITE, a));
                return;
            }
        }
        if checkers_form {
            let show_icon = self.show_checker_icons && kind != Kind::Pawn;
            let royal = kind == Kind::King && !show_icon;
            // Иконка-силуэт фигуры: маска из того же SVG — сплошная заливка,
            // ровно по центру шашки, меньше диаметра, без артефактов.
            let draw_icon = |painter: &egui::Painter| {
                if !show_icon {
                    return;
                }
                if let Some(ico) = self.icon_tex.get(side, kind) {
                    let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
                    let tint = if side == Side::White {
                        alpha(Color32::from_rgb(46, 34, 22), a * 0.92)
                    } else {
                        alpha(Color32::from_rgb(240, 231, 209), a * 0.92)
                    };
                    painter.image(
                        ico.mask.id(),
                        Rect::from_center_size(center, Vec2::splat(size * 0.34)),
                        uv,
                        tint,
                    );
                } else {
                    art::draw_checker_icon(painter, center, size, kind, side, a);
                }
            };
            if let Some(art) = self.piece_tex.checker_art(side, damka, royal) {
                let rect = Rect::from_center_size(center, Vec2::splat(size));
                let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
                painter.image(art.color.id(), rect, uv, alpha(Color32::WHITE, a));
            } else {
                // Корона и точка рисуются отдельно, поверх иконки фигуры.
                draw_checker(painter, center, size, false, false, side, pal, a);
            }
            // Иконка фигуры — вписывается в круг дамки.
            draw_icon(painter);
            // Корона дамки — поверх фигуры.
            if damka && !self.piece_tex.has_damka(side) {
                art::draw_damka_crown(painter, center, size, side, a);
            }
            if royal && !self.piece_tex.has_royal(side) {
                art::draw_royal_dot(painter, center, size, side, a);
            }
            return;
        } else {
            draw_chess_piece(painter, center, size, kind, side, pal, a);
        }
    }

    // ------------------------- панель -------------------------

    fn paint_start_frost(
        &self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        layout: &Layout,
    ) {
        let b = layout.board;
        let frost = painter.with_clip_rect(b);
        // Сильное размытие: плотная матовая вуаль в два слоя.
        frost.rect_filled(
            b,
            10.0,
            Color32::from_rgba_unmultiplied(8, 10, 18, 116),
        );
        frost.rect_filled(
            b,
            10.0,
            Color32::from_rgba_unmultiplied(16, 18, 30, 18),
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
        // Плашка подгоняется под текст: ширина — по самой длинной строке,
        // поэтому она не «висит» шире содержимого.
        let title_font = FontId::proportional(22.0);
        let hint_font = FontId::proportional(12.0);
        let caption_font = FontId::proportional(11.0);
        let caption_text = i18n::tr("Демонстрация: компьютер играет сам с собой");
        // В конце демо-партии плашка на несколько секунд показывает её
        // результат — иначе концовку демонстрации не с чем соотнести.
        let demo_over = !self.started && self.demo.is_over();
        let (title_text, hint_text) = if demo_over {
            match &self.demo.status {
                game::Status::Win { side, reason } => (
                    i18n::trf("ПОБЕДА {side}", &[("side", &side.name_ru().to_uppercase())]),
                    i18n::tr_owned(reason.clone()),
                ),
                game::Status::Draw { reason } => (
                    i18n::tr("НИЧЬЯ").to_string(),
                    i18n::tr_owned(reason.clone()),
                ),
                _ => (
                    i18n::tr("ПАРТИЯ ЕЩЁ НЕ НАЧАТА").to_string(),
                    String::new(),
                ),
            }
        } else {
            // Инструкцию «нажмите…» не пишем: плашка ведёт себя как ссылка —
            // курсор-рука и подсказка при наведении.
            (
                i18n::tr("ПАРТИЯ ЕЩЁ НЕ НАЧАТА").to_string(),
                String::new(),
            )
        };
        let text_w = |text: &str, font: FontId| {
            painter
                .layout_no_wrap(text.to_string(), font, Color32::WHITE)
                .size()
                .x
        };
        let widest = text_w(&title_text, title_font.clone())
            .max(text_w(&hint_text, hint_font.clone()))
            .max(text_w(caption_text, caption_font.clone()));
        let has_hint = !hint_text.is_empty();
        let label = Rect::from_center_size(
            b.center(),
            Vec2::new(
                (widest + 72.0).clamp(300.0, b.width() * 0.78),
                if has_hint { 104.0 } else { 74.0 },
            ),
        );
        // Полупрозрачная тонированная плашка: доска с демонстрацией видна сквозь неё.
        let pal = Palette::for_phase(if self.started {
            self.game.phase
        } else {
            self.demo.phase
        });
        painter.rect_filled(
            label.translate(Vec2::new(0.0, 10.0)),
            20.0,
            Color32::from_black_alpha(48),
        );
        rr(
            painter,
            label,
            20.0,
            alpha(mix(Color32::from_rgb(10, 12, 20), pal.accent, 0.18), 0.55),
            Stroke::new(1.0_f32, alpha(pal.accent, 0.34)),
        );
        // Тени под текстом: плашка полупрозрачная, буквы должны читаться
        // и на светлых клетках доски.
        let (title_pos, hint_pos, caption_pos) = if has_hint {
            (
                label.center() + Vec2::new(0.0, -20.0),
                label.center() + Vec2::new(0.0, 10.0),
                label.center() + Vec2::new(0.0, 32.0),
            )
        } else {
            (
                label.center() + Vec2::new(0.0, -12.0),
                label.center() + Vec2::new(0.0, 10.0),
                label.center() + Vec2::new(0.0, 16.0),
            )
        };
        let shadow = Color32::from_black_alpha(150);
        for (pos, text, font) in [
            (title_pos, title_text.as_str(), title_font.clone()),
            (hint_pos, hint_text.as_str(), hint_font.clone()),
            (caption_pos, caption_text, caption_font.clone()),
        ] {
            painter.text(
                pos + Vec2::new(0.0, 1.4),
                Align2::CENTER_CENTER,
                text,
                font,
                shadow,
            );
        }
        painter.text(
            title_pos,
            Align2::CENTER_CENTER,
            title_text.as_str(),
            title_font,
            Color32::from_rgb(246, 243, 252),
        );
        painter.text(
            hint_pos,
            Align2::CENTER_CENTER,
            hint_text.as_str(),
            hint_font,
            Color32::from_rgb(226, 222, 240),
        );
        painter.text(
            caption_pos,
            Align2::CENTER_CENTER,
            caption_text,
            caption_font,
            Color32::from_rgb(196, 192, 212),
        );
        // Плашка ведёт себя как ссылка: курсор-рука и подсказка при наведении.
        // Подсказку не показываем, когда на плашке результат демонстрации.
        let resp = ui.interact(label, ui.id().with("start_plate"), egui::Sense::click());
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if !demo_over {
            // Подсказка у курсора, как у ссылки в браузере: плашка широкая,
            // и привязка к её краю уводила текст далеко от мыши.
            resp.on_hover_text_at_pointer(i18n::tr("Запуск игры"));
        }
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

        if self.review {
            self.paint_panel_review(ui, painter, layout);
            return;
        }

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

        let over = g.is_over();
        if over {
            // Итог партии — наверху панели, как заголовок: кто победил,
            // почему и что было последним ходом.
            let (title, reason, color) = match &g.status {
                game::Status::Win { side, reason } => (
                    i18n::trf("ПОБЕДА {side}", &[("side", &side.name_ru().to_uppercase())]),
                    i18n::tr_owned(reason.clone()),
                    cur.accent,
                ),
                game::Status::Draw { reason } => (
                    i18n::tr("НИЧЬЯ").to_string(),
                    i18n::tr_owned(reason.clone()),
                    cur.text,
                ),
                _ => ("ПАРТИЯ".to_string(), String::new(), cur.text),
            };
            spaced_text(painter, Pos2::new(x, y), &title, 23.0, color, 2.2);
            y += 34.0;
            if !reason.is_empty() {
                text_fit(
                    painter,
                    Pos2::new(x, y),
                    &reason,
                    w,
                    13.0,
                    alpha(cur.text_dim, 0.95),
                );
                y += 26.0;
            }
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
                    alpha(cur.text, 0.9),
                    w,
                );
                let h = galley.size().y;
                painter.galley(Pos2::new(x, y), galley, Color32::WHITE);
                y += h + 6.0;
            }
            // Матующие фигуры — если победа матом.
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
                                let line =
                                    i18n::trf("Мат ставят: {names}", &[("names", &names)]);
                                let galley = painter.layout(
                                    line,
                                    FontId::proportional(10.5),
                                    alpha(cur.danger, 0.95),
                                    w,
                                );
                                let h = galley.size().y;
                                painter.galley(Pos2::new(x, y), galley, Color32::WHITE);
                                y += h + 6.0;
                            }
                        }
                    }
                }
            }
            text_fit(
                painter,
                Pos2::new(x, y),
                &i18n::trf("Ходов: {n}", &[("n", &g.history.len().to_string())]),
                w,
                11.0,
                alpha(cur.text_dim, 0.72),
            );
            y += 24.0;
        } else {
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
        // Фиксированная длина фазы: рядом с часами виден срок до смены природы.
        if g.twilight_by_moves && !g.is_over() && g.trans == 0.0 {
            let left = g.full_moves_to_switch();
            let label = i18n::trf("смена через {n} х.", &[("n", &left.to_string())]);
            text(
                painter,
                Pos2::new(x + w, y + 8.0),
                Align2::RIGHT_TOP,
                &label,
                11.5,
                alpha(cur.accent, 0.95),
            );
        }
        let sub = if g.trans > 0.0 {
            // Во время сумерек подпись не нужна: переход виден по анимации.
            String::new()
        } else if g.paused {
            "пауза".to_string()
        } else if g.grace_hold() {
            // Последние 5 секунд: фигура в руке — таймер ждёт её хода.
            i18n::tr("таймер ждёт вашего хода").to_string()
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
        };
        text_fit(
            painter,
            Pos2::new(x + 4.0, y + 50.0),
            &sub,
            w - 4.0,
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

        // предпросмотр фаз — высота как у нижнего ряда кнопок
        let pill_h = 36.0;
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
                8.0,
                alpha(col, if active { 0.16 } else { 0.04 }),
                Stroke::new(1.0_f32, alpha(col, if active { 0.55 } else { 0.12 })),
            );
            text(
                painter,
                pill.center(),
                Align2::CENTER_CENTER,
                ph.one_ru(),
                12.5,
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
            Pos2::new(x, y + 12.0),
            Align2::LEFT_TOP,
            "СОПЕРНИК",
            8.5,
            alpha(cur.text_dim, 0.75),
        );
        let label_w = 66.0;
        let chip_h = 36.0;
        let gap = 8.0;
        let chips_x = x + label_w;
        let chips_w = w - label_w;
        // «по сети» появляется только после включения в настройках
        let net_count = if self.net_enabled { 3 } else { 2 };
        let mut mode_labels: Vec<&str> = vec!["вдвоём", "с компьютером"];
        if self.net_enabled {
            mode_labels.push("по сети");
        }
        // Ширина кнопок считается по тексту: «с компьютером» длиннее соседей
        // и в немецком («gegen den Computer») не влезает в равную треть.
        // Свободное место делим поровну, чтобы кнопки не «прыгали» между
        // языками, а тесное — ужимаем пропорционально.
        let mut widths: Vec<f32> = mode_labels
            .iter()
            .map(|label| {
                let galley = painter.layout_no_wrap(
                    i18n::tr_owned(label.to_string()),
                    FontId::proportional(12.0),
                    cur.text,
                );
                galley.size().x + 16.0
            })
            .collect();
        let total = (chips_w - gap * (net_count - 1) as f32).max(1.0);
        let sum: f32 = widths.iter().sum();
        if sum < total {
            let extra = (total - sum) / net_count as f32;
            for w in widths.iter_mut() {
                *w += extra;
            }
        } else {
            let k = total / sum;
            for w in widths.iter_mut() {
                *w *= k;
            }
        }
        let hot = Rect::from_min_size(Pos2::new(chips_x, y), Vec2::new(widths[0], chip_h));
        let comp = Rect::from_min_size(
            Pos2::new(chips_x + widths[0] + gap, y),
            Vec2::new(widths[1], chip_h),
        );
        if chip(ui, hot, "вдвоём", self.mode == Mode::Hotseat, &cur) {
            self.mode = Mode::Hotseat;
        }
        if chip(ui, comp, "с компьютером", self.mode == Mode::Computer, &cur) {
            self.mode = Mode::Computer;
            self.ai_wait = 0.5;
        }
        if self.net_enabled {
            let net_chip = Rect::from_min_size(
                Pos2::new(chips_x + widths[0] + widths[1] + gap * 2.0, y),
                Vec2::new(widths[2], chip_h),
            );
            if chip(ui, net_chip, "по сети", self.mode == Mode::Network, &cur) {
                // Сеть поднимается в update(): сокеты не открываются до этого
                // момента, поэтому ОС не спрашивает доступ к сети при запуске.
                self.net_open_requested = true;
                self.mode = Mode::Network;
                if !self.net_active() {
                    self.net_window = true;
                }
            }
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
        // Индикатор стороны: диаметр по высоте строчных букв (14 px при
        // кегле 19), между кружком и текстом — заметный отступ.
        let dot_r = 7.0;
        let dot = Pos2::new(x + dot_r, y + 12.0);
        let side_color = match g.turn {
            Side::White => Color32::from_rgb(244, 232, 210),
            Side::Black => Color32::from_rgb(70, 82, 110),
        };
        painter.circle_filled(dot, dot_r, side_color);
        painter.circle_stroke(dot, dot_r, Stroke::new(1.0_f32, alpha(cur.text_dim, 0.6)));
        let text_x = x + dot_r * 2.0 + 10.0;
        text_fit(
            painter,
            Pos2::new(text_x, y),
            &i18n::trf("Ход {side}", &[("side", g.turn.gen_ru())]),
            w - (text_x - x),
            19.0,
            cur.text,
        );
        let king_danger = if g.phase == Phase::Checkers {
            g.king_under_attack(g.turn)
        } else {
            g.king_danger_after_flip(g.turn)
        };
        if king_danger {
            // Король под боем — важнее любых других подсказок.
            let label = if g.phase == Phase::Checkers {
                "КОРОЛЬ ПОД БОЕМ"
            } else {
                "КОРОЛЬ ПОД УДАРОМ"
            };
            badge(painter, Pos2::new(x + w, y - 2.0), label, cur.danger, true, 11.0);
        } else if let game::Status::Check(_) = &g.status {
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
            0 => i18n::tr("счёт равный").to_string(),
            d if d > 0 => i18n::trf("белые впереди на {n}", &[("n", &d.to_string())]),
            d => i18n::trf("чёрные впереди на {n}", &[("n", &(-d).to_string())]),
        };
        text_fit(
            painter,
            Pos2::new(x, y),
            &i18n::trf(
                "{material} · взято {white} : {black}",
                &[
                    ("material", &material),
                    ("white", &g.captured_by_white.to_string()),
                    ("black", &g.captured_by_black.to_string()),
                ],
            ),
            w,
            11.5,
            alpha(cur.text_dim, 0.95),
        );
        y += 24.0;
        }

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
        // Карточка смены фаз: поля совпадают с общими отступами панели.
        let setup_pad = 12.0;
        let setup_w = w - setup_pad * 2.0;
        // Высота по содержимому: в «по ходам» — памятка правил (сворачивается
        // ссылкой), в остальных режимах — ряд чипов/вторая строка. Низ карточки
        // держим в 10 px над кнопками, журнал подрезаем по её верхнему краю.
        let rules_open = self.rules_open;
        let setup_h = if g.twilight_by_moves {
            if rules_open {
                152.0
            } else {
                38.0
            }
        } else {
            88.0
        };
        let setup_top = row_y - 10.0 - setup_h;
        let mut toggle_rules = false;
        let chips = [
            (10.0, "10 с"),
            (15.0, "15 с"),
            (20.0, "20 с"),
            (40.0, "40 с"),
        ];
        let chip_w = (setup_w - 24.0) / 4.0;
        let mut chip_rects = Vec::new();
        for (i, _) in chips.iter().enumerate() {
            chip_rects.push(Rect::from_min_size(
                Pos2::new(x + setup_pad + i as f32 * (chip_w + 8.0), setup_top + 30.0),
                Vec2::new(chip_w, 36.0),
            ));
        }
        // журнал ходов (последние строки)
        let log_top = y;
        let log_bottom = setup_top - 10.0;
        let line_h = 17.0;
        let max_lines = (((log_bottom - log_top) / line_h).floor() as usize).max(0);
        if max_lines > 0 && !g.log.is_empty() {
            let start = g.log.len().saturating_sub(max_lines);
            let mut ly = log_top;
            for (idx, line) in g.log[start..].iter().enumerate() {
                let is_last = start + idx + 1 == g.log.len();
                text_fit(
                    painter,
                    Pos2::new(x, ly),
                    line,
                    w,
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
            // Подсказки не рисуем, если под них нет места: раскрытая сводка
            // правил съедает высоту журнала, и строки наезжали друг на друга.
            let hint_h = 17.0;
            if log_top + hint_h <= log_bottom {
                text_fit(
                    painter,
                    Pos2::new(x, log_top),
                    "пока тихо — сделайте первый ход",
                    w,
                    12.5,
                    alpha(cur.text_dim, 0.78),
                );
            }
            let tip = i18n::tr_owned(
                "Совет: в шашечной фазе король превращается в дамку — съешьте его и победите."
                    .to_string(),
            );
            let galley = painter.layout(
                tip,
                FontId::proportional(11.5),
                alpha(cur.text_dim, 0.72),
                w,
            );
            let tip_y = log_top + hint_h + 6.0;
            if tip_y + galley.size().y <= log_bottom {
                painter.galley(Pos2::new(x, tip_y), galley, Color32::WHITE);
            }
        }

        // Предварительные настройки игры: компактная карточка с единым
        // стилем, чтобы блок читался так же, как остальные строки панели.
        let setup_bg = Rect::from_min_size(
            Pos2::new(x, setup_top),
            Vec2::new(w, setup_h),
        );
        // Непрозрачная подложка: иначе строки журнала просвечивают сквозь
        // карточку и наезжают на текст правил. Цвет берём из той же палитры,
        // что и фон окна: в лобби это демонстрация, в партии — текущая фаза,
        // иначе карточка выбивается из окружения на смене природы.
        let (scr_phase, scr_trans) = if self.started {
            (g.phase, g.trans)
        } else {
            (self.demo.phase, self.demo.trans)
        };
        let scr_pal = blend(
            Palette::for_phase(scr_phase),
            Palette::for_phase(scr_phase.other()),
            smooth(scr_trans) * 0.7,
        );
        let ground = {
            let t = ((setup_bg.center().y - layout.full.top())
                / layout.full.height().max(1.0))
                .clamp(0.0, 1.0);
            mix(scr_pal.bg_top, scr_pal.bg_bottom, t)
        };
        rr(
            painter,
            setup_bg,
            12.0,
            mix(ground, Color32::WHITE, 0.08),
            Stroke::new(1.0_f32, alpha(Color32::WHITE, 0.07)),
        );
        let head_dy = if rules_open { 12.0 } else { 13.0 };
        text(
            painter,
            Pos2::new(x + setup_pad, setup_top + head_dy),
            Align2::LEFT_TOP,
            if g.twilight_by_moves {
                "ПРАВИЛА ПАРТИИ"
            } else if g.auto_phase {
                "АВТОСМЕНА СУМЕРЕК"
            } else {
                "РИТМ ТЕКУЩЕЙ ФАЗЫ"
            },
            9.5,
            alpha(cur.text_dim, 0.9),
        );
        if g.twilight_by_moves {
            // Ссылка «скрыть/показать» рядом с заголовком карточки.
            let link_label = if rules_open { "скрыть" } else { "показать" };
            let link_txt = i18n::tr_owned(link_label.to_string());
            let header_w = painter
                .layout_no_wrap(
                    i18n::tr_owned("ПРАВИЛА ПАРТИИ".to_string()),
                    FontId::proportional(9.5),
                    Color32::WHITE,
                )
                .size()
                .x;
            let link_w = painter
                .layout_no_wrap(
                    link_txt.clone(),
                    FontId::proportional(9.5),
                    Color32::WHITE,
                )
                .size()
                .x;
            let link_rect = Rect::from_min_size(
                Pos2::new(x + setup_pad + header_w + 10.0, setup_top + head_dy - 1.0),
                Vec2::new(link_w + 2.0, 12.0),
            );
            let link_resp =
                ui.interact(link_rect, ui.id().with("rules_toggle"), Sense::click());
            if link_resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if link_resp.clicked() {
                toggle_rules = true;
            }
            painter.text(
                Pos2::new(link_rect.left(), setup_top + head_dy),
                Align2::LEFT_TOP,
                &link_txt,
                FontId::proportional(9.5),
                if link_resp.hovered() {
                    cur.accent
                } else {
                    alpha(cur.accent, 0.8)
                },
            );
            if rules_open {
                // Что означает фаза и как она меняется в этой партии.
                text_fit(
                    painter,
                    Pos2::new(x + setup_pad, setup_top + 38.0),
                    &i18n::trf(
                        "Полных ходов в фазе: шахматы — {c}, шашки — {s}.",
                        &[
                            ("c", &g.chess_moves.to_string()),
                            ("s", &g.checkers_moves.to_string()),
                        ],
                    ),
                    setup_w,
                    10.5,
                    alpha(cur.text_dim, 0.9),
                );
                text_fit(
                    painter,
                    Pos2::new(x + setup_pad, setup_top + 54.0),
                    "Оба знают, сколько ходов осталось до смены природы.",
                    setup_w,
                    10.5,
                    alpha(cur.text_dim, 0.9),
                );
                divider(painter, x + setup_pad, setup_top + 75.5, setup_w, cur.text_dim);
                // Подробная памятка: то же, что блок пояснений в настройках.
                let rules = [
                    if g.mandatory_capture {
                        "Бить обязательно; серию нужно продолжать."
                    } else {
                        "Можно без взятия; серию можно закончить."
                    },
                    if g.max_chain {
                        "Брать можно только самую длинную серию взятий."
                    } else {
                        "Достаточно любого взятия, длина серии не важна."
                    },
                    if g.crown_non_pawns {
                        "Дамками становятся все фигуры, а не только пешки."
                    } else {
                        "Дамками становятся только пешки."
                    },
                    if g.king_escape {
                        "Король под боем может уйти вместо обязательного взятия."
                    } else {
                        "Под боем король бьёт наравне со всеми — без исключений."
                    },
                ];
                for (i, line) in rules.iter().enumerate() {
                    text_fit(
                        painter,
                        Pos2::new(x + setup_pad, setup_top + 82.0 + i as f32 * 16.0),
                        line,
                        setup_w,
                        10.5,
                        alpha(cur.text_dim, 0.9),
                    );
                }
            }
        } else if g.auto_phase {
            text_fit(
                painter,
                Pos2::new(x + setup_pad, setup_top + 30.0),
                &i18n::trf("Случайно: 10–{max} с", &[("max", &format!("{:.0}", g.auto_max))]),
                setup_w,
                12.5,
                cur.accent,
            );
            text_fit(
                painter,
                Pos2::new(x + setup_pad, setup_top + 48.0),
                "Следующий интервал будет другим",
                setup_w,
                11.0,
                alpha(cur.text_dim, 0.85),
            );
        }
        if toggle_rules {
            self.rules_open = !self.rules_open;
        }
        // Идёт партия и в журнал записан новый ход — сводка снова сворачивается,
        // чтобы не занимать место у журнала. Раскрыть можно ссылкой.
        if self.started && self.rules_open && g.log.len() > self.rules_seen_log {
            self.rules_open = false;
        }
        self.rules_seen_log = g.log.len();
        // --- взаимодействие ---
        if self.settings || self.rules || self.guide || self.journal {
            return;
        }
        if g.is_over() {
            // Финальные кнопки стоят там же, где игровые: меняется только
            // содержимое — добавлены журнал и просмотр партии.
            if button(ui, rules_btn, "Журнал", false, &cur, 12.0) {
                self.journal_text = self.build_journal_text();
                self.journal = true;
            }
            if button_gear(ui, settings_btn, "Настройки", false, &cur, 11.5) {
                self.settings = true;
                self.rules = false;
            }
            if button(ui, play_btn, "Новая партия", true, &cur, 14.5) {
                self.new_game();
            }
            if button(ui, new_btn, "Просмотр", false, &cur, 14.5) {
                self.start_review();
            }
            return;
        }
        if self.game.pending_promotion.is_some() {
            return;
        }
        if button_resp(ui, rules_btn, "Правила", false, &cur, 12.0, i18n::tr("H — правила")).clicked() {
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
        if button_resp(
            ui,
            play_btn,
            pause_label,
            true,
            &cur,
            14.5,
            i18n::tr("Space — игра/пауза"),
        ).clicked() {
            if !self.started {
                self.start_game();
            } else {
                self.net_set_pause(!self.game.paused);
            }
        }
        if button_resp(
            ui,
            new_btn,
            "Новая партия",
            false,
            &cur,
            14.5,
            i18n::tr("R — новая партия"),
        ).clicked() {
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
            if self.net_active() {
                self.net_send(net::OutMsg::ChainEnd);
            }
        }
    }

    /// Поднять сетевой контекст при первом обращении к сети.
    /// Сокеты (UDP-обнаружение и приём TCP) открываются только здесь, поэтому
    /// запрос доступа брандмауэра Windows появляется лишь при включении режима.
    fn net_ensure(&mut self) -> bool {
        if self.net.is_none() {
            self.net = net::Net::new();
        }
        self.net.is_some()
    }

    /// Запустить проверку обновлений в фоне.
    fn update_check_start(&mut self) {
        if self.update_check.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel::<updater::CheckResult>();
        self.update_check = Some(rx);
        self.update_state = updater::UpdateState::Checking;
        std::thread::spawn(move || {
            let _ = tx.send(updater::check_latest());
        });
    }

    /// Забрать результат фоновой проверки, если он уже готов.
    fn update_check_poll(&mut self) {
        let Some(rx) = &self.update_check else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(ver)) => {
                self.update_state = if updater::is_newer(&ver, updater::VERSION) {
                    updater::UpdateState::Available(ver)
                } else {
                    updater::UpdateState::UpToDate
                };
                self.update_check = None;
            }
            Ok(Err(reason)) => {
                // Причина уходит в журнал — по нему видно, что именно не так
                // (нет сети, блокировка, прокси).
                self.game.log.push(format!(
                    "{}: {reason}",
                    i18n::tr("Проверка обновлений не удалась")
                ));
                if self.game.log.len() > 240 {
                    self.game.log.remove(0);
                }
                self.update_state = updater::UpdateState::Failed;
                self.update_check = None;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.update_state = updater::UpdateState::Failed;
                self.update_check = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }

    // ------------------------- настройки -------------------------

    fn paint_settings(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, layout: &Layout) {
        painter.rect_filled(layout.full, 0.0, Color32::from_black_alpha(165));
        // Чуть шире, чем раньше: длинные переводы (es/de/fr/tr) в правой
        // колонке должны помещаться целиком, без обрезки.
        // Высота — по окну: с группами настроек контент стал выше,
        // фиксированные 660 px обрезали нижние блоки.
        let card_h = (layout.full.height() - 40.0).clamp(560.0, 962.0);
        let card = Rect::from_center_size(layout.full.center(), Vec2::new(760.0, card_h));
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
        // Колонки прокручиваются, если содержимое не влезает: сдвиг,
        // обрезка и полупрозрачный скроллбар справа — как в окне сети.
        // Низ списка — выше подвала со сводкой и кнопками, иначе
        // прокручиваемое содержимое рисуется под текстом сводки.
        let bottom_limit = card.bottom() - 140.0;
        let view_h = (bottom_limit - top).max(120.0);
        let max_off = (self.settings_content_h - view_h).max(0.0);
        let wheel = ui.input(|i| i.raw_scroll_delta.y);
        if let Some(m) = ui.input(|i| i.pointer.latest_pos()) {
            let zone = Rect::from_min_max(
                Pos2::new(card.left() + 10.0, top),
                Pos2::new(card.right() - 10.0, bottom_limit),
            );
            if zone.contains(m) && max_off > 0.5 {
                self.settings_scroll = (self.settings_scroll - wheel * 0.5).clamp(0.0, max_off);
            }
        }
        self.settings_scroll = self.settings_scroll.clamp(0.0, max_off);
        let off = self.settings_scroll;
        let clip_area = Rect::from_min_max(
            Pos2::new(card.left() + 8.0, top),
            Pos2::new(card.right() - 8.0, bottom_limit),
        );
        let mut columns_end = top;
        let _ = ui.scope_builder(egui::UiBuilder::new().max_rect(clip_area), |ui| {
            ui.set_clip_rect(clip_area);
            let painter = &ui.painter().clone();
            let mut ly = top - off;
        group_title(painter, x, ly, "Звук и журнал", pal.accent);
        ly += 17.0;
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
        ly += 4.0;

        // наборы звука и музыки: одна компактная строка с двумя кнопками-циклами
        let sound_items: Vec<(Option<String>, String)> = std::iter::once((
            None,
            i18n::tr("Встроенные").to_string(),
        ))
        .chain(
            self.packs
                .iter()
                .filter(|p| p.sounds().is_some())
                .map(|p| (Some(p.name.clone()), p.label.clone())),
        )
        .collect();
        let music_items: Vec<(Option<String>, String)> = std::iter::once((
            None,
            i18n::tr("Встроенная").to_string(),
        ))
        .chain(
            self.packs
                .iter()
                .filter(|p| p.music().is_some())
                .map(|p| (Some(p.name.clone()), p.label.clone())),
        )
        .collect();
        if sound_items.len() > 1 || music_items.len() > 1 {
            ly += 4.0;
            let cw = (left_w - 8.0) * 0.5;
            let cur_sound = sound_items
                .iter()
                .find(|(n, _)| n.as_deref() == self.sound_pack.as_deref())
                .map(|(_, l)| l.clone())
                .unwrap_or_else(|| i18n::tr("Встроенные").to_string());
            let cur_music = music_items
                .iter()
                .find(|(n, _)| n.as_deref() == self.music_pack.as_deref())
                .map(|(_, l)| l.clone())
                .unwrap_or_else(|| i18n::tr("Встроенная").to_string());
            let sound_btn = Rect::from_min_size(Pos2::new(x, ly), Vec2::new(cw, 28.0));
            let music_btn = Rect::from_min_size(
                Pos2::new(x + cw + 8.0, ly),
                Vec2::new(cw, 28.0),
            );
            let sound_label = i18n::trf("Звуки: {name}", &[("name", &cur_sound)]);
            if chip(ui, sound_btn, &sound_label, false, &pal)
                && sound_items.len() > 1
            {
                let pos = sound_items
                    .iter()
                    .position(|(n, _)| n.as_deref() == self.sound_pack.as_deref())
                    .unwrap_or(0);
                let (next_name, _) = &sound_items[(pos + 1) % sound_items.len()];
                save_pack("sounds", next_name.as_deref());
                self.sound_pack = next_name.clone();
                let dir = next_name
                    .as_ref()
                    .and_then(|n| self.packs.iter().find(|p| p.name == *n))
                    .and_then(|p| p.sounds());
                sound::set_sounds_root(dir);
            }
            let music_label = i18n::trf("Музыка: {name}", &[("name", &cur_music)]);
            if chip(ui, music_btn, &music_label, false, &pal)
                && music_items.len() > 1
            {
                let pos = music_items
                    .iter()
                    .position(|(n, _)| n.as_deref() == self.music_pack.as_deref())
                    .unwrap_or(0);
                let (next_name, _) = &music_items[(pos + 1) % music_items.len()];
                save_pack("music", next_name.as_deref());
                self.music_pack = next_name.clone();
                let dir = next_name
                    .as_ref()
                    .and_then(|n| self.packs.iter().find(|p| p.name == *n))
                    .and_then(|p| p.music());
                music::set_music_root(dir);
            }
            ly += 34.0;
        }

        

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
        text_fit(
            painter,
            Pos2::new(x, ly),
            "Файлы — в папке logs рядом с игрой.",
            left_w,
            11.0,
            alpha(pal.text_dim, 0.95),
        );
        ly += 20.0;
        let open_logs_btn = Rect::from_min_size(Pos2::new(x, ly), Vec2::new(left_w, 32.0));
        if button(ui, open_logs_btn, "Открыть логи", false, &pal, 11.5) {
            self.open_logs();
        }
        ly += 38.0;

        ly += 6.0;
        divider(painter, x, ly, left_w, pal.text_dim);
        ly += 16.0;
        group_title(painter, x, ly, "Оформление", pal.accent);
        // Переключатель пульсации — справа на той же строке, места хватает.
        let pulse_chip = Rect::from_min_size(
            Pos2::new(x + left_w - 112.0, ly - 5.0),
            Vec2::new(112.0, 20.0),
        );
        if chip(
            ui,
            pulse_chip,
            if self.pulse_on {
                "Пульсация: да"
            } else {
                "Пульсация: нет"
            },
            self.pulse_on,
            &pal,
        ) {
            self.pulse_on = !self.pulse_on;
        }
        ly += 22.0;
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
        text_fit(
            painter,
            Pos2::new(x, ly),
            "Свои .ttf/.otf — в папке fonts рядом с игрой.",
            left_w,
            11.0,
            alpha(pal.text_dim, 0.92),
        );
        ly += 20.0;

        // код правил: текущий код + ввод чужого кода
        // набор фигур: встроенный + папки-пресеты из pieces/
        text(
            painter,
            Pos2::new(x, ly),
            Align2::LEFT_TOP,
            "НАБОР ФИГУР",
            9.0,
            alpha(pal.text_dim, 0.75),
        );
        ly += 15.0;
        {
            // (папка, подпись): у встроенного папки нет.
            let names: Vec<(Option<String>, String)> = std::iter::once((
                None,
                i18n::tr("Встроенный").to_string(),
            ))
            .chain(
                self.piece_presets
                    .iter()
                    .map(|(dir, label)| (Some(dir.clone()), label.clone())),
            )
            .collect();
            let chip_w = (left_w - 8.0) * 0.5;
            for (i, name) in names.iter().enumerate() {
                let row = (i / 2) as f32;
                let col = (i % 2) as f32;
                let r = Rect::from_min_size(
                    Pos2::new(x + (chip_w + 8.0) * col, ly + 32.0 * row),
                    Vec2::new(chip_w, 28.0),
                );
                let (folder, label) = name;
                let on = self.piece_preset.as_deref() == folder.as_deref();
                if chip(ui, r, label, on, &pal) && !on {
                    save_preset(folder.as_deref());
                    self.piece_preset = folder.clone();
                    self.piece_tex = load_piece_textures(
                        ui.ctx(),
                        &preset_dir_for(&self.packs, folder.as_deref()),
                        folder.as_deref().unwrap_or("builtin"),
                        512,
                    );
                }
            }
            let rows = ((names.len() + 1) / 2) as f32;
            ly += 32.0 * rows + 6.0;
        }
        ly += 6.0;

        {
        ly += 6.0;
        divider(painter, x, ly, left_w, pal.text_dim);
        ly += 16.0;
        group_title(painter, x, ly, "Вид", pal.accent);
        ly += 16.0;
        text(
            painter,
            Pos2::new(x, ly),
            Align2::LEFT_TOP,
            "ПЛАВНОСТЬ ХОДА",
            9.0,
            alpha(pal.text_dim, 0.75),
        );
        ly += 16.0;
        let track = Rect::from_min_size(Pos2::new(x, ly), Vec2::new(left_w - 72.0, 6.0));
        let slider = ui.interact(
            track.expand2(Vec2::new(0.0, 14.0)),
            ui.id().with("smooth_slider"),
            Sense::click_and_drag(),
        );
        if slider.dragged() || slider.clicked() {
            if let Some(p) = slider.interact_pointer_pos() {
                let t = ((p.x - track.left()) / track.width()).clamp(0.0, 1.0);
                self.game.move_anim_secs = 0.05 + t * 0.85;
            }
        }
        let frac = ((self.game.move_anim_secs - 0.05) / 0.85).clamp(0.0, 1.0);
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
        ly += 30.0;

        text(
            painter,
            Pos2::new(x, ly),
            Align2::LEFT_TOP,
            "МАТ · ТРАЕКТОРИИ",
            9.0,
            alpha(pal.text_dim, 0.75),
        );
        ly += 15.0;
        let half = (left_w - 8.0) * 0.5;
        let mate_chip = Rect::from_min_size(Pos2::new(x, ly), Vec2::new(half, 28.0));
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
            Pos2::new(x + half + 8.0, ly),
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
        ly += 32.0;
        }
        {
        // Сеть: режим «по сети» скрыт, пока не включён здесь. Блок стоит
        // в левой колонке — рядом с остальными настройками программы.
        divider(painter, x, ly, left_w, pal.text_dim);
        ly += 16.0;
        group_title(painter, x, ly, "Сеть", pal.accent);
        ly += 18.0;
        let net_btn = Rect::from_min_size(Pos2::new(x, ly), Vec2::new(left_w, 40.0));
        let net_label = if self.net_enabled {
            "Сетевая игра: включена"
        } else {
            "Сетевая игра: выключена"
        };
        if button(ui, net_btn, net_label, self.net_enabled, &pal, 12.5) {
            self.net_enabled = !self.net_enabled;
            if self.net_enabled {
                // Сеть поднимается по требованию — здесь же ОС спросит
                // разрешение на доступ к частным сетям.
                if !self.net_ensure() {
                    self.net_enabled = false;
                    self.net_status = i18n::tr("Сеть недоступна").to_string();
                }
            } else {
                if self.net_active() {
                    self.net_close("сетевой режим выключен");
                }
                if let Some(mut n) = self.net.take() {
                    n.shutdown();
                }
                self.mode = Mode::Hotseat;
            }
        }
        ly += 46.0;
        text_fit(
            painter,
            Pos2::new(x, ly),
            "Соперники рядом — без сервера. Включите, чтобы появился режим «по сети».",
            left_w,
            11.0,
            alpha(pal.text_dim, 0.95),
        );
        ly += 26.0;
        }


        // правая колонка: правила партии и их код для обмена
        let mut ry = top - off;
        group_title(painter, right_x, ry, "Правила партии", pal.accent);
        ry += 17.0;
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
            text_fit(
                painter,
                Pos2::new(right_x, ry),
                "Фаза меняется после полных ходов — по ходу на каждого.",
                right_w,
                11.0,
                alpha(pal.text, 0.95),
            );
            ry += 15.0;
            text_fit(
                painter,
                Pos2::new(right_x, ry),
                "Оба знают, сколько ходов осталось до смены природы.",
                right_w,
                11.0,
                alpha(pal.text_dim, 0.95),
            );
            ry += 22.0;
            text(
                painter,
                Pos2::new(right_x, ry),
                Align2::LEFT_TOP,
                "ДЛИНА ФАЗЫ · ПОЛНЫХ ХОДОВ",
                9.0,
                alpha(pal.text_dim, 0.75),
            );
            ry += 15.0;
            // Длина фазы — отдельно для шахмат и шашек: на короткой фазе
            // не успеваешь перестроить фигуры под следующий режим.
            for (phase_label, phase) in [("шахматы", Phase::Chess), ("шашки", Phase::Checkers)] {
                text_fit(
                    painter,
                    Pos2::new(right_x, ry),
                    phase_label,
                    right_w,
                    10.0,
                    alpha(pal.text, 0.9),
                );
                let fw = (right_w - 6.0 * 6.0) / 7.0;
                let current = self.game.phase_len(phase);
                for (k, n) in [2u8, 3, 4, 5, 6, 7, 8].iter().enumerate() {
                    let chip_rect = Rect::from_min_size(
                        Pos2::new(right_x + (fw + 6.0) * k as f32, ry + 15.0),
                        Vec2::new(fw, 26.0),
                    );
                    let label = format!("{n}");
                    if chip(ui, chip_rect, &label, current == *n, &pal) {
                        self.game.set_phase_len(phase, *n);
                    }
                }
                ry += 15.0 + 26.0 + 8.0;
            }
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
            text_fit(
                painter,
                Pos2::new(right_x, ry),
                "По истечении ход переходит противнику.",
                right_w,
                11.0,
                alpha(pal.text_dim, 0.95),
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
                text_fit(
                    painter,
                    Pos2::new(right_x, ry),
                    "Каждая смена случайна; следующий интервал отличается.",
                    right_w,
                    10.0,
                    alpha(pal.text_dim, 0.88),
                );
                ry += 25.0;
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
        let chain_chip = Rect::from_min_size(
            Pos2::new(right_x, ry + 34.0),
            Vec2::new((right_w - 8.0) * 0.5, 28.0),
        );
        let escape_chip = Rect::from_min_size(
            Pos2::new(right_x + (right_w - 8.0) * 0.5 + 8.0, ry + 34.0),
            Vec2::new((right_w - 8.0) * 0.5, 28.0),
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
        if chip(
            ui,
            chain_chip,
            if self.game.max_chain {
                "Цепочка: максимум"
            } else {
                "Цепочка: любая"
            },
            self.game.max_chain,
            &pal,
        ) {
            self.game.max_chain = !self.game.max_chain;
        }
        if chip(
            ui,
            escape_chip,
            if self.game.king_escape {
                "Король уходит: да"
            } else {
                "Король уходит: нет"
            },
            self.game.king_escape,
            &pal,
        ) {
            self.game.king_escape = !self.game.king_escape;
        }
        ry += 70.0;
        text_fit(
            painter,
            Pos2::new(right_x, ry),
            if mandatory {
                "Бить обязательно; серию нужно продолжать."
            } else {
                "Можно без взятия; серию можно закончить."
            },
            right_w,
            11.0,
            alpha(pal.text_dim, 0.95),
        );
        ry += 15.0;
        text_fit(
            painter,
            Pos2::new(right_x, ry),
            if self.game.max_chain {
                "Брать можно только самую длинную серию взятий."
            } else {
                "Достаточно любого взятия, длина серии не важна."
            },
            right_w,
            11.0,
            alpha(pal.text_dim, 0.95),
        );
        ry += 15.0;
        text_fit(
            painter,
            Pos2::new(right_x, ry),
            "Иконки — кроме пешек; дамки — все или только пешки.",
            right_w,
            11.0,
            alpha(pal.text_dim, 0.95),
        );
        ry += 15.0;
        text_fit(
            painter,
            Pos2::new(right_x, ry),
            if self.game.king_escape {
                "Король под боем может уйти вместо обязательного взятия."
            } else {
                "Под боем король бьёт наравне со всеми — без исключений."
            },
            right_w,
            11.0,
            alpha(pal.text_dim, 0.95),
        );
        ry += 15.0;

        {
        ry += 6.0;
        divider(painter, right_x, ry, right_w, pal.text_dim);
        ry += 16.0;
        group_title(painter, right_x, ry, "Код правил партии", pal.accent);
        ry += 18.0;
        // Код правил — один блок из 4 символов: тот же, что третий блок
        // в сетевом коде приглашения (ID+ID+правила).
        let code = rulecode::encode_compact(&rulecode::Rules::from_game(&self.game));
        let code_row_h = 30.0;
        // Код — по вертикальному центру строки, кнопка — у правого края колонки.
        text(
            painter,
            Pos2::new(right_x, ry + code_row_h * 0.5),
            Align2::LEFT_CENTER,
            &code,
            15.0,
            pal.accent,
        );
        // Компактная иконка вместо широкой кнопки: место для кода, подсказка при наведении.
        let copy_btn = Rect::from_min_size(
            Pos2::new(right_x + right_w - 26.0, ry + (code_row_h - 26.0) * 0.5),
            Vec2::new(26.0, 26.0),
        );
        let copy_resp = ui.interact(
            copy_btn,
            ui.id().with(("copy_code", copy_btn.top() as i32)),
            Sense::click(),
        );
        draw_copy_icon(ui, copy_btn, &pal, copy_resp.hovered());
        if copy_resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if copy_resp.on_hover_text(i18n::tr("Копировать")).clicked() {
            ui.ctx().copy_text(code);
            self.rules_code_msg = i18n::tr("Код скопирован").to_string();
        }
        ry += code_row_h + 8.0;

        let input_w = right_w - 120.0;
        let input_rect = Rect::from_min_size(Pos2::new(right_x, ry), Vec2::new(input_w, 28.0));
        rr(
            painter,
            input_rect,
            8.0,
            alpha(pal.text, 0.06),
            Stroke::new(1.0_f32, alpha(pal.text, 0.18)),
        );
        let code_field = egui::TextEdit::singleline(&mut self.rules_code_input)
            .font(FontId::proportional(13.0))
            .text_color(pal.text)
            .frame(false)
            .hint_text("XXXX");
        ui.put(input_rect, code_field);
        let apply_btn = Rect::from_min_size(
            Pos2::new(right_x + right_w - 112.0, ry),
            Vec2::new(124.0, 28.0),
        );
        if button(ui, apply_btn, "Применить", false, &pal, 11.5) {
            match rulecode::decode_compact(&self.rules_code_input) {
                Some(rules) => {
                    rules.apply(&mut self.game);
                    self.rules_code_msg = rules.summary();
                }
                None => {
                    self.rules_code_msg = i18n::tr("Неверный код").to_string();
                }
            }
        }
        ry += 30.0;
        if !self.rules_code_msg.is_empty() {
            text_fit(
                painter,
                Pos2::new(right_x, ry),
                &self.rules_code_msg,
                right_w,
                11.0,
                alpha(pal.text_dim, 0.95),
            );
            ry += 16.0;
        }
        }


            columns_end = ly.max(ry);
        });
        // Реальная высота содержимого — для бегунка со следующего кадра.
        let content_h = (columns_end - top + off).max(0.0);
        self.settings_content_h = content_h;
        let actual_max = (content_h - view_h).max(0.0);
        self.settings_scroll = self.settings_scroll.clamp(0.0, actual_max);
        let summary_y = if actual_max > 0.5 {
            card.bottom() - 132.0
        } else {
            (columns_end + 16.0).max(card.bottom() - 132.0)
        };
        let summary = if self.game.twilight_by_moves {
            i18n::trf(
                "Сейчас: полных ходов в фазе — шахматы {c}, шашки {s}; время хода {secs} с",
                &[
                    ("c", &self.game.chess_moves.to_string()),
                    ("s", &self.game.checkers_moves.to_string()),
                    ("secs", &format!("{:.0}", self.game.move_time_limit)),
                ],
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
        text_fit(
            painter,
            Pos2::new(x, summary_y),
            &summary,
            w,
            12.0,
            alpha(pal.text, 0.92),
        );
        text_fit(
            painter,
            Pos2::new(x, summary_y + 26.0),
            "Свои звуки — sounds (WAV/MP3), музыка — music/theme.wav, фигуры — pieces (PNG).",
            w,
            11.0,
            alpha(pal.text_dim, 0.95),
        );

        // кнопка обновлений: проверка версии на GitHub, при наличии —
        // открытие страницы релизов (скачивание делает сам пользователь)
        self.update_check_poll();
        let upd_state = self.update_state.clone();
        let upd_label: String = match &upd_state {
            updater::UpdateState::Idle => i18n::tr("Обновления").to_string(),
            updater::UpdateState::Checking => i18n::tr("Проверка…").to_string(),
            updater::UpdateState::UpToDate => i18n::tr("Обновлений нет").to_string(),
            updater::UpdateState::Available(ver) => {
                i18n::trf("Скачать {ver}", &[("ver", ver)])
            }
            updater::UpdateState::Failed => i18n::tr("Сеть недоступна").to_string(),
        };
        let upd_btn = Rect::from_min_size(
            Pos2::new(
                card.right() - pad - 160.0 - 8.0 - 168.0,
                card.bottom() - pad - 42.0,
            ),
            Vec2::new(168.0, 42.0),
        );
        if button(
            ui,
            upd_btn,
            &upd_label,
            matches!(upd_state, updater::UpdateState::Available(_)),
            &pal,
            12.5,
        ) {
            match self.update_state {
                updater::UpdateState::Available(_) => updater::open_downloads(),
                updater::UpdateState::Checking => {}
                _ => self.update_check_start(),
            }
        }

        let close = Rect::from_min_size(
            Pos2::new(card.right() - pad - 160.0, card.bottom() - pad - 42.0),
            Vec2::new(160.0, 42.0),
        );
        if button(ui, close, "Готово", true, &pal, 14.0) {
            self.settings = false;
        }

        // скроллбар настроек: полупрозрачная дорожка и бегунок
        if actual_max > 0.5 {
            let track = Rect::from_min_max(
                Pos2::new(card.right() - 12.0, top + 2.0),
                Pos2::new(card.right() - 6.0, bottom_limit - 2.0),
            );
            painter.rect_filled(track, 3.0, alpha(Color32::WHITE, 0.08));
            let thumb_h = (view_h / content_h * track.height()).clamp(28.0, track.height());
            let thumb_y = track.top()
                + self.settings_scroll / actual_max * (track.height() - thumb_h).max(0.0);
            let thumb = Rect::from_min_size(
                Pos2::new(track.left(), thumb_y),
                Vec2::new(track.width(), thumb_h),
            );
            painter.rect_filled(thumb, 3.0, alpha(Color32::WHITE, 0.30));
            let resp = ui.interact(thumb, ui.id().with("settings_scrollbar"), Sense::drag());
            if resp.dragged() {
                let factor = actual_max / (track.height() - thumb_h).max(1.0);
                self.settings_scroll =
                    (self.settings_scroll + resp.drag_delta().y * factor).clamp(0.0, actual_max);
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::Grab);
            }
        }
    }

    // ------------------------- сетевое окно -------------------------

    fn net_connect_addr(&mut self, addr: std::net::SocketAddr) {
        if let Some(net) = self.net.as_mut() {
            net.connect(addr);
            self.net_status = i18n::trf(
                "Подключение к {addr}…",
                &[("addr", &addr.to_string())],
            );
        }
    }

    /// Разобрать поле подключения: адрес, ID или код приглашения.
    fn net_route_connect(&mut self) {
        let raw = self.net_addr_input.trim().to_string();
        if raw.is_empty() {
            return;
        }
        let Some(code) = normalize_code(&raw) else {
            // Точка/двоеточие или иные символы — это адрес.
            self.net_connect_input();
            return;
        };
        let (id8, rules_part) = match code.chars().count() {
            // Приглашение: 8 символов ID + 4 символа правил.
            12 => (Some(code[..8].to_string()), Some(code[8..].to_string())),
            8 => (Some(code), None),
            _ => {
                self.net_status = i18n::tr("Неверный код").to_string();
                return;
            }
        };
        let id8 = id8.expect("длина проверена выше");
        let id = format!("{}-{}", &id8[..4], &id8[4..]);
        if !net::valid_id(&id) {
            self.net_status = i18n::tr("Неверный код").to_string();
            return;
        }
        let rules_summary = rules_part
            .and_then(|rc| rulecode::decode_compact(&rc))
            .map(|r| r.summary());
        // 1) Соперник уже виден в локальной сети — прямое подключение.
        let peer = self.net.as_ref().and_then(|n| {
            n.peers()
                .iter()
                .find(|p| strip_code(&p.id) == id8)
                .cloned()
        });
        if let Some(p) = peer {
            self.net_connect_addr(p.addr);
            if let Some(s) = rules_summary {
                self.net_status = i18n::trf(
                    "Подключение к {addr}: {summary}",
                    &[("addr", &p.addr.to_string()), ("summary", &s)],
                );
            }
            return;
        }
        // 2) Соперника нет рядом: сейчас игра соединяется только в одной сети.
        if let Some(s) = rules_summary {
            self.net_status = i18n::trf(
                "Правила: {summary} — соперник не найден",
                &[("summary", &s)],
            );
        } else {
            self.net_status = i18n::tr("Соперник не найден в этой сети").to_string();
        }
    }

    /// Разобрать адрес из поля: «192.168.1.5» или «192.168.1.5:47771».
    fn net_connect_input(&mut self) {
        let raw = self.net_addr_input.trim().to_string();
        if raw.is_empty() {
            return;
        }
        let (ip, port) = match raw.rsplit_once(':') {
            Some((ip, port)) => (ip.to_string(), port.to_string()),
            None => (raw.clone(), net::TCP_PORT.to_string()),
        };
        match format!("{}:{}", ip, port).parse::<std::net::SocketAddr>() {
            Ok(addr) => self.net_connect_addr(addr),
            Err(_) => {
                self.net_status = i18n::tr("Неверный адрес").to_string();
            }
        }
    }

    fn net_chat_send(&mut self) {
        let text = self.net_chat_input.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.net_chat_input.clear();
        self.net_chat.push((text.clone(), true, std::time::Instant::now()));
        self.net_send(net::OutMsg::Chat(text));
    }

    fn paint_net(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, layout: &Layout) {
        let pal = Palette::for_phase(self.game.phase);
        let next = Palette::for_phase(self.game.phase.other());
        let cur = blend(pal, next, smooth(self.game.trans));
        painter.rect_filled(layout.full, 0.0, Color32::from_black_alpha(175));

        // Единый ритм окна: поле 22 по кругу, заголовок 26, зазор 12,
        // внизу справа текстовая кнопка «Закрыть» (как «Понятно» в правилах).
        // Высота карточки подгоняется под содержимое, лишних пустот нет;
        // если содержимое не влезает — справа полупрозрачный скроллбар.
        let pad = 22.0;
        let title_h = 26.0;
        let gap = 12.0;
        let btn_h = 38.0;
        let card_w = 560.0;
        let session = self.net_active();
        let Some(net) = self.net.as_ref() else {
            let card = Rect::from_center_size(layout.board.center(), Vec2::new(card_w, 150.0));
            self.net_rect = card;
            rr(
                painter,
                card,
                22.0,
                Color32::from_rgb(22, 20, 30),
                Stroke::new(1.0_f32, alpha(cur.accent, 0.25)),
            );
            text_fit(
                painter,
                Pos2::new(card.left() + pad, card.center().y - 8.0),
                "Сеть недоступна в этой сборке.",
                card_w - pad * 2.0,
                13.0,
                cur.text_dim,
            );
            let close = Rect::from_min_size(
                Pos2::new(card.right() - pad - 150.0, card.bottom() - pad - btn_h),
                Vec2::new(150.0, btn_h),
            );
            if button(ui, close, "Закрыть", false, &cur, 13.5) {
                self.net_window = false;
            }
            return;
        };
        let id = net.my_id();
        let hosting = net.hosting();
        let peers = net.peers();
        let peers_n = peers.len().min(5);
        let content_h = if session {
            26.0 + 28.0 + 20.0 + 160.0 + 42.0 + 42.0
        } else {
            let mut h = 16.0 + 44.0 + 16.0 + 74.0 + 16.0;
            if peers_n == 0 {
                h += 30.0;
            } else {
                h += peers_n as f32 * 36.0 + 8.0;
            }
            h += 16.0 + 42.0 + if self.net_status.is_empty() { 0.0 } else { 20.0 };
            h
        };
        let view_max = (layout.full.height() - 200.0).max(200.0);
        let view_h = content_h.min(view_max);
        let max_off = (content_h - view_h).max(0.0);
        let card_h = pad + title_h + gap + view_h + gap + btn_h + pad;
        let half = card_h * 0.5;
        let mut cy = layout.board.center().y;
        let lo = layout.full.top() + 20.0 + half;
        let hi = layout.full.bottom() - 20.0 - half;
        cy = if lo <= hi {
            cy.clamp(lo, hi)
        } else {
            layout.full.center().y
        };
        let card = Rect::from_center_size(Pos2::new(layout.board.center().x, cy), Vec2::new(card_w, card_h));
        self.net_rect = card;
        rr(
            painter,
            card,
            22.0,
            Color32::from_rgb(22, 20, 30),
            Stroke::new(1.0_f32, alpha(cur.accent, 0.25)),
        );
        text(
            painter,
            Pos2::new(card.left() + pad, card.top() + pad - 2.0),
            Align2::LEFT_TOP,
            "СЕТЕВАЯ ИГРА",
            15.0,
            cur.accent,
        );
        // Закрытие — текстовой кнопкой внизу справа, как «Понятно» в правилах.
        let close = Rect::from_min_size(
            Pos2::new(card.right() - pad - 150.0, card.bottom() - pad - btn_h),
            Vec2::new(150.0, btn_h),
        );
        if button(ui, close, "Закрыть", false, &cur, 13.5) {
            self.net_window = false;
        }

        let inner = Rect::from_min_max(
            Pos2::new(card.left() + pad, card.top() + pad + title_h + gap),
            Pos2::new(card.right() - pad, card.bottom() - pad - btn_h - gap),
        );
        let sb_w = if max_off > 0.5 { 12.0 } else { 0.0 };
        let view = Rect::from_min_max(
            inner.left_top(),
            Pos2::new(inner.right() - sb_w, inner.bottom()),
        );
        let w = view.width();

        let wheel = ui.input(|i| i.raw_scroll_delta.y);
        if let Some(m) = ui.input(|i| i.pointer.latest_pos()) {
            if inner.contains(m) && max_off > 0.5 {
                self.net_scroll = (self.net_scroll - wheel * 0.5).clamp(0.0, max_off);
            }
        }
        self.net_scroll = self.net_scroll.clamp(0.0, max_off);
        let off = self.net_scroll;

        let _ = ui.scope_builder(egui::UiBuilder::new().max_rect(view), |ui| {
            ui.set_clip_rect(view);
            let cp = &ui.painter().clone();
            let vy = |yy: f32| view.top() + yy - off;
            let mut y = 0.0;

            if session {
                // --- активный сеанс: соперник, чат, закрытие ---
                let sess = self.net_session().unwrap();
                let side_label = match self.net_side {
                    Some(Side::White) => i18n::tr("вы играете белыми").to_string(),
                    Some(Side::Black) => i18n::tr("вы играете чёрными").to_string(),
                    None => String::new(),
                };
                text_fit(
                    cp,
                    Pos2::new(card.left() + pad, vy(y)),
                    &format!(
                        "{} · {} · {}",
                        i18n::trf("Соперник {id}", &[("id", &net::display_id(&sess.peer_id))]),
                        side_label,
                        sess.peer_addr
                    ),
                    w,
                    13.0,
                    cur.text,
                );
                y += 18.0 + 8.0;
                text_fit(
                    cp,
                    Pos2::new(card.left() + pad, vy(y)),
                    &self.net_status,
                    w,
                    11.5,
                    cur.accent,
                );
                y += 16.0 + 12.0;

                // чат: сообщения живут 60 секунд, в логи не пишутся
                text(
                    cp,
                    Pos2::new(card.left() + pad, vy(y)),
                    Align2::LEFT_TOP,
                    "ЧАТ",
                    9.0,
                    alpha(cur.text_dim, 0.75),
                );
                y += 16.0 + 4.0;
                let chat_top = y;
                let chat_bottom = y + 150.0;
                let mut cyy = chat_bottom - 16.0;
                for (msg, mine, _) in self.net_chat.iter().rev() {
                    if cyy < chat_top {
                        break;
                    }
                    let prefix = if *mine {
                        i18n::tr("вы").to_string()
                    } else {
                        i18n::tr("соперник").to_string()
                    };
                    let line = format!("{}: {}", prefix, msg);
                    let color = if *mine { cur.accent } else { cur.text };
                    text_fit(cp, Pos2::new(card.left() + pad, vy(cyy)), &line, w, 12.0, color);
                    cyy -= 17.0;
                }
                y = chat_bottom + 10.0;

                // ввод чата: поле слева, кнопка — у правого края
                let input_w = w - 100.0;
                let input_rect =
                    Rect::from_min_size(Pos2::new(card.left() + pad, vy(y)), Vec2::new(input_w, 30.0));
                rr(
                    cp,
                    input_rect,
                    8.0,
                    alpha(Color32::WHITE, 0.05),
                    Stroke::new(1.0_f32, alpha(Color32::WHITE, 0.10)),
                );
                let field = egui::TextEdit::singleline(&mut self.net_chat_input)
                    .font(FontId::proportional(12.5))
                    .text_color(cur.text)
                    .frame(false)
                    .hint_text(i18n::tr("сообщение…"));
                let resp = ui.put(input_rect.shrink(6.0), field);
                let send = Rect::from_min_size(
                    Pos2::new(view.right() - 92.0, vy(y) + 1.0),
                    Vec2::new(92.0, 28.0),
                );
                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter)) && resp.has_focus();
                if button(ui, send, "Отправить", false, &cur, 11.5) || enter {
                    self.net_chat_send();
                }
                y += 30.0 + 12.0;

                let close_btn =
                    Rect::from_min_size(Pos2::new(card.left() + pad, vy(y)), Vec2::new(w, 38.0));
                if button(ui, close_btn, "Закрыть сеанс", true, &cur, 12.5) {
                    self.net_close("закрыт вами");
                }
            } else {
                // --- поиск соперника ---
                // Код приглашения: сетевой ID + компактный блок настроек
                // (4 символа). Поле выделяемое мышью — код можно скопировать
                // и без кнопки.
                let invite = format!(
                    "{}-{}",
                    id,
                    rulecode::encode_compact(&rulecode::Rules::from_game(&self.game))
                );
                text(
                    cp,
                    Pos2::new(card.left() + pad, vy(y)),
                    Align2::LEFT_TOP,
                    "МОЙ КОД",
                    9.0,
                    alpha(cur.text_dim, 0.75),
                );
                y += 16.0;
                self.net_code_view = invite.clone();
                let code_rect = Rect::from_min_size(
                    Pos2::new(card.left() + pad, vy(y)),
                    Vec2::new(w - 36.0, 30.0),
                );
                let code_field = egui::TextEdit::singleline(&mut self.net_code_view)
                    .font(FontId::proportional(14.0))
                    .text_color(cur.text)
                    .frame(false);
                ui.put(code_rect, code_field);
                // Та же компактная иконка, что в настройках: код + подсказка.
                let copy = Rect::from_min_size(
                    Pos2::new(view.right() - 26.0, vy(y) + 2.0),
                    Vec2::new(26.0, 26.0),
                );
                let copy_resp = ui.interact(
                    copy,
                    ui.id().with(("net_copy", copy.top() as i32)),
                    Sense::click(),
                );
                draw_copy_icon(ui, copy, &cur, copy_resp.hovered());
                if copy_resp.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                if copy_resp.on_hover_text(i18n::tr("Копировать")).clicked() {
                    ui.ctx().copy_text(invite);
                    self.net_status = i18n::tr("Код скопирован").to_string();
                }
                y += 30.0 + 14.0;

                // приветствие — единственное сообщение до начала партии
                text(
                    cp,
                    Pos2::new(card.left() + pad, vy(y)),
                    Align2::LEFT_TOP,
                    "ПРИВЕТСТВИЕ",
                    9.0,
                    alpha(cur.text_dim, 0.75),
                );
                y += 16.0;
                let greet_rect =
                    Rect::from_min_size(Pos2::new(card.left() + pad, vy(y)), Vec2::new(w, 60.0));
                rr(
                    cp,
                    greet_rect,
                    8.0,
                    alpha(Color32::WHITE, 0.05),
                    Stroke::new(1.0_f32, alpha(Color32::WHITE, 0.10)),
                );
                let greet = egui::TextEdit::singleline(&mut self.net_greeting)
                    .font(FontId::proportional(12.5))
                    .text_color(cur.text)
                    .frame(false)
                    .hint_text(i18n::tr("короткое сообщение сопернику"));
                ui.put(greet_rect.shrink(6.0), greet);
                y += 60.0 + 14.0;

                // игроки рядом
                text(
                    cp,
                    Pos2::new(card.left() + pad, vy(y)),
                    Align2::LEFT_TOP,
                    "ИГРОКИ РЯДОМ",
                    9.0,
                    alpha(cur.text_dim, 0.75),
                );
                y += 16.0;
                if peers.is_empty() {
                    text_fit(
                        cp,
                        Pos2::new(card.left() + pad, vy(y)),
                        &i18n::tr("Пока никого. Откройте игру на другом компьютере.").to_string(),
                        w,
                        11.5,
                        cur.text_dim,
                    );
                    y += 18.0 + 12.0;
                } else {
                    for peer in peers.iter().take(5) {
                        let row =
                            Rect::from_min_size(Pos2::new(card.left() + pad, vy(y)), Vec2::new(w, 32.0));
                        rr(cp, row, 8.0, alpha(Color32::WHITE, 0.04), Stroke::NONE);
                        text_fit(
                            cp,
                            Pos2::new(card.left() + pad + 10.0, vy(y) + 16.0),
                            &format!("{} · {}", net::display_id(&peer.id), peer.addr),
                            w - 130.0,
                            12.0,
                            cur.text,
                        );
                        let btn = Rect::from_min_size(
                            Pos2::new(view.right() - 116.0, vy(y) + 2.0),
                            Vec2::new(110.0, 28.0),
                        );
                        if button(ui, btn, "Подключиться", false, &cur, 11.0) {
                            self.net_connect_addr(peer.addr);
                        }
                        y += 36.0;
                    }
                    y += 8.0;
                }

                // подключение: адрес, ID или код приглашения — и ожидание
                // соперника в одном ряду, кнопки — у правого края
                text(
                    cp,
                    Pos2::new(card.left() + pad, vy(y)),
                    Align2::LEFT_TOP,
                    "ПОДКЛЮЧЕНИЕ",
                    9.0,
                    alpha(cur.text_dim, 0.75),
                );
                y += 16.0;
                let wait_w = 150.0;
                let go_w = 116.0;
                let input_w = (w - go_w - wait_w - 16.0).max(160.0);
                let addr_rect = Rect::from_min_size(
                    Pos2::new(card.left() + pad, vy(y)),
                    Vec2::new(input_w, 30.0),
                );
                rr(
                    cp,
                    addr_rect,
                    8.0,
                    alpha(Color32::WHITE, 0.05),
                    Stroke::new(1.0_f32, alpha(Color32::WHITE, 0.10)),
                );
                let addr_field = egui::TextEdit::singleline(&mut self.net_addr_input)
                    .font(FontId::proportional(12.5))
                    .text_color(cur.text)
                    .frame(false)
                    .hint_text(i18n::tr("IP или код"));
                ui.put(addr_rect.shrink(6.0), addr_field);
                let go = Rect::from_min_size(
                    Pos2::new(card.left() + pad + input_w + 8.0, vy(y) + 1.0),
                    Vec2::new(go_w, 28.0),
                );
                if button(ui, go, "Подключить", false, &cur, 11.5) {
                    self.net_route_connect();
                }
                let wait_btn = Rect::from_min_size(
                    Pos2::new(card.left() + pad + input_w + go_w + 16.0, vy(y) + 1.0),
                    Vec2::new(wait_w, 28.0),
                );
                if hosting {
                    if button(ui, wait_btn, "Отменить ожидание", true, &cur, 11.5) {
                        if let Some(n) = self.net.as_mut() {
                            n.cancel_hosting();
                        }
                    }
                } else if button(ui, wait_btn, "Ждать соперника", true, &cur, 11.5) {
                    if let Some(n) = self.net.as_mut() {
                        n.host();
                    }
                }
                y += 30.0 + 12.0;

                if !self.net_status.is_empty() {
                    text_fit(
                        cp,
                        Pos2::new(card.left() + pad, vy(y)),
                        &self.net_status,
                        w,
                        11.5,
                        cur.accent,
                    );
                }
            }
        });

        // скроллбар: полупрозрачная дорожка и бегунок, колесо и перетаскивание
        if max_off > 0.5 {
            let track = Rect::from_min_max(
                Pos2::new(inner.right() - 9.0, inner.top() + 2.0),
                Pos2::new(inner.right() - 3.0, inner.bottom() - 2.0),
            );
            painter.rect_filled(track, 3.0, alpha(Color32::WHITE, 0.08));
            let thumb_h = (view_h / content_h * track.height()).clamp(28.0, track.height());
            let thumb_y =
                track.top() + off / max_off * (track.height() - thumb_h).max(0.0);
            let thumb = Rect::from_min_size(
                Pos2::new(track.left(), thumb_y),
                Vec2::new(track.width(), thumb_h),
            );
            painter.rect_filled(thumb, 3.0, alpha(Color32::WHITE, 0.30));
            let resp = ui.interact(thumb, ui.id().with("net_scrollbar"), Sense::drag());
            if resp.dragged() {
                let factor = max_off / (track.height() - thumb_h).max(1.0);
                self.net_scroll =
                    (self.net_scroll + resp.drag_delta().y * factor).clamp(0.0, max_off);
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::Grab);
            }
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
            self.paint_piece(painter, icon, 56.0, *kind, false, self.game.turn, &cur, 1.0, false, None);
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
                let kind = *kind;
                let free = self.game.promotion_free;
                let promo_sq = self.game.pending_promotion;
                self.game.choose_promotion(kind);
                if self.net_active() {
                    if free {
                        // свободное превращение после сумерек — без хода
                        if let Some(sq) = promo_sq {
                            self.net_send(net::OutMsg::PromoFree { sq, kind });
                        }
                    } else if let Some(mv) = self.net_pending_move.take() {
                        self.net_send_move(mv, Some(kind));
                    }
                }
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
            "Правила ShatMat",
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
            "Король в шашках выглядит как шашка с меткой, но ходит и бьёт как дамка. Его можно съесть — это сразу поражение.",
            "Дамки летят по диагонали на любое расстояние.",
            "Бить обязательно; серия взятий продолжается, пока есть цель.",
            "Обязательное взятие можно отключить в настройках.",
            "Иконки на шашках показывают вид фигуры; пешки — без иконок.",
            "По умолчанию дамкой может стать любая фигура; «Дамки: пешки» — только пешки.",
            "Шашка на последнем ряду становится дамкой и продолжает серию как дамка.",
            "Короля можно съесть и победить.",
            "Обязательный бой действует для всех шашек, кроме короля: он бьёт только по желанию.",
            "Король под боем может уйти вместо обязательного взятия — настройка «Король уходит».",
            "Если застряли оба — сумерки сразу выводят в шахматы.",
        ];
        let shared = [
            "Сумерки — это смена игрового режима: игра идёт по правилам шашек, природа фигур и правила меняются вместе с ними.",
            "Сумерки длятся 1,7 секунды: ходы замирают, фигуры перетекают.",
            "Переход ждёт, пока игрок завершит серию взятий.",
            "Смена фаз не начинается, пока в текущем режиме не решена ясная ситуация: в шашках — если можно съесть короля, в шахматах — если не отыгран шах. Сумерки ждут следующий ход.",
            "Держать сумерки дольше двух ходов подряд нельзя: если смена фаз всё-таки наступила, а короли стоят вплотную, тот, чей ход, забирает чужого короля.",
            "Победа: мат, съеденный король или истребление фигур.",
            "Время шахмат и шашек задаётся отдельно в настройках.",
            "Смена фаз: по времени или по полным ходам — по ходу на каждого.",
            "Длина фазы задаётся отдельно для шахмат и шашек — от 2 до 8 полных ходов; счётчик виден в панели, обе стороны знают срок.",
            "В режиме по ходам у каждого хода есть время 10–180 с; по истечении ход переходит противнику.",
            "В шашках король ходит как дамка, но его можно взять — берегите короля до возврата в шахматы.",
            "«Цепочка: максимум» — брать разрешено только самую длинную серию взятий.",
            "Последние 5 секунд таймер ждёт, пока вы держите фигуру: ход успевает завершиться до смены игры.",
            "Компьютер думает 2–3 с и подсвечивает, какую фигуру берёт.",
            "После партии «Просмотр» показывает ходы по шагам с анимацией.",
            "При мате видны красные линии атак на короля, жёлтые линии фигур, перекрывающих выходы, и список матующих фигур.",
            "С компьютером вы играете белыми; сложность меняет глубину.",
        ];

        let draw_rules = [
            "Король против короля: после пустого полухода каждого — ничья.",
            "Одинокий король против короля и одной сильной фигуры: после 20 полуходов без взятий — ничья, позицию не доиграть.",
        ];

        let chess_col = Color32::from_rgb(232, 179, 106);
        let check_col = Color32::from_rgb(111, 217, 229);
        let shared_col = Color32::from_rgb(150, 142, 162);
        let draw_col = Color32::from_rgb(178, 166, 208);
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
                    // Чуть больше воздуха между строками — длинные правила
                    // читаются легче, особенно на испанском и немецком.
                    line_height: Some(size * 1.5),
                    ..Default::default()
                },
            );
        }
        push_line(
            &mut job,
            &i18n::tr_owned("КОГДА ФИГУРЫ — ШАХМАТЫ\n".to_string()),
            14.5,
            chess_col,
        );
        for line in chess_rules {
            push_line(
                &mut job,
                &format!("• {}\n", i18n::tr_owned(line.to_string())),
                13.5,
                body,
            );
        }
        push_line(&mut job, "\n\n", 13.5, body);
        push_line(
            &mut job,
            &i18n::tr_owned("КОГДА ФИГУРЫ — ШАШКИ\n".to_string()),
            14.5,
            check_col,
        );
        for line in checkers_rules {
            push_line(
                &mut job,
                &format!("• {}\n", i18n::tr_owned(line.to_string())),
                13.5,
                body,
            );
        }
        push_line(&mut job, "\n\n", 13.5, body);
        push_line(
            &mut job,
            &i18n::tr_owned("СУМЕРКИ\n".to_string()),
            14.5,
            shared_col,
        );
        for line in shared {
            push_line(
                &mut job,
                &format!("• {}\n", i18n::tr_owned(line.to_string())),
                13.5,
                body,
            );
        }
        push_line(&mut job, "\n\n", 13.5, body);
        push_line(
            &mut job,
            &i18n::tr_owned("НИЧЬИ\n".to_string()),
            14.5,
            draw_col,
        );
        for line in draw_rules {
            push_line(
                &mut job,
                &format!("• {}\n", i18n::tr_owned(line.to_string())),
                13.5,
                body,
            );
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

    fn paint_logs(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, layout: &Layout) {
        painter.rect_filled(layout.full, 0.0, Color32::from_black_alpha(170));
        let card = Rect::from_center_size(layout.full.center(), Vec2::new(760.0, 580.0));
        self.logs_rect = card;
        let pal = Palette::for_phase(self.game.phase);
        rr(
            painter,
            card,
            24.0,
            Color32::from_rgb(22, 20, 30),
            Stroke::new(1.0_f32, alpha(pal.accent, 0.40)),
        );
        let pad = 26.0;
        text(
            painter,
            Pos2::new(card.left() + pad, card.top() + 20.0),
            Align2::LEFT_TOP,
            "ЛОГИ ПАРТИЙ",
            19.0,
            pal.text,
        );
        text_fit(
            painter,
            Pos2::new(card.left() + pad, card.top() + 48.0),
            "Выберите партию — откроется просмотр с доской и ходами.",
            card.width() - pad * 2.0,
            11.0,
            alpha(pal.text_dim, 0.9),
        );
        let list_top = card.top() + 76.0;
        let list_bottom = card.bottom() - 72.0;
        let track = Rect::from_min_max(
            Pos2::new(card.left() + pad, list_top),
            Pos2::new(card.right() - pad - 8.0, list_bottom),
        );
        let row_h = 34.0;
        let content_h = self.logs_list.len() as f32 * row_h;
        let max_off = (content_h - track.height()).max(0.0);
        if let Some(m) = ui.input(|i| i.pointer.latest_pos()) {
            if track.contains(m) && max_off > 0.5 {
                let wheel = ui.input(|i| i.raw_scroll_delta.y);
                self.logs_scroll = (self.logs_scroll - wheel * 0.5).clamp(0.0, max_off);
            }
        }
        self.logs_scroll = self.logs_scroll.clamp(0.0, max_off);
        let clip = painter.with_clip_rect(track);
        if self.logs_list.is_empty() {
            clip.text(
                track.center(),
                Align2::CENTER_CENTER,
                i18n::tr("Логов пока нет"),
                FontId::proportional(13.0),
                alpha(pal.text_dim, 0.9),
            );
        }
        let mut open: Option<PathBuf> = None;
        for (i, (path, caption)) in self.logs_list.iter().enumerate() {
            let y = list_top - self.logs_scroll + i as f32 * row_h;
            let row = Rect::from_min_size(
                Pos2::new(track.left(), y),
                Vec2::new(track.width(), row_h - 4.0),
            );
            if row.bottom() < list_top || row.top() > list_bottom {
                continue;
            }
            let resp = ui.interact(row, ui.id().with(("log_row", i)), Sense::click());
            if resp.hovered() {
                clip.rect_filled(row, 8.0, alpha(pal.accent, 0.10));
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            clip.text(
                Pos2::new(row.left() + 10.0, row.center().y),
                Align2::LEFT_CENTER,
                caption,
                FontId::proportional(11.5),
                alpha(pal.text, 0.95),
            );
            if resp.clicked() {
                open = Some(path.clone());
            }
        }
        if max_off > 0.5 {
            let bar = Rect::from_min_max(
                Pos2::new(card.right() - pad - 4.0, list_top),
                Pos2::new(card.right() - pad, list_bottom),
            );
            painter.rect_filled(bar, 3.0, alpha(Color32::WHITE, 0.08));
            let thumb_h =
                (track.height() / content_h * bar.height()).clamp(28.0, bar.height());
            let thumb_y =
                bar.top() + self.logs_scroll / max_off * (bar.height() - thumb_h).max(0.0);
            painter.rect_filled(
                Rect::from_min_size(
                    Pos2::new(bar.left(), thumb_y),
                    Vec2::new(bar.width(), thumb_h),
                ),
                3.0,
                alpha(pal.accent, 0.55),
            );
        }
        // Галка «показывать демо-партии» слева от кнопки закрытия.
        let check = Rect::from_min_size(
            Pos2::new(card.left() + pad, card.bottom() - 46.0),
            Vec2::new(18.0, 18.0),
        );
        let check_hit = ui.interact(
            check.expand2(Vec2::new(4.0, 6.0)),
            ui.id().with("logs_demo_check"),
            Sense::click(),
        );
        painter.rect_stroke(
            check,
            4.0,
            Stroke::new(1.2_f32, alpha(pal.text_dim, 0.75)),
            egui::StrokeKind::Inside,
        );
        if self.logs_demo {
            painter.rect_filled(check.shrink(4.0), 2.0, pal.accent);
        }
        if check_hit.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        text(
            painter,
            Pos2::new(check.right() + 10.0, check.center().y),
            Align2::LEFT_CENTER,
            i18n::tr("показывать демо-партии"),
            11.5,
            alpha(pal.text, 0.92),
        );
        if check_hit.clicked() {
            self.logs_demo = !self.logs_demo;
            self.logs_list = replay::list_logs(self.logs_demo);
            self.logs_scroll = 0.0;
        }
        let close = Rect::from_min_size(
            Pos2::new(card.right() - pad - 150.0, card.bottom() - 54.0),
            Vec2::new(150.0, 38.0),
        );
        if button(ui, close, "Закрыть", false, &pal, 12.5) {
            self.logs_window = false;
        }
        if let Some(p) = open {
            self.open_log_file(p);
        }
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
            None,
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
                None,
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
            None,
        );
    }

    /// Панель в просмотре: тот же сайдбар, но вместо ходов партии —
    /// кликабельный список с навигацией по полуходам.
    fn paint_panel_review(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, layout: &Layout) {
        let g = &self.game;
        let pal = Palette::for_phase(g.phase);
        let cur = pal;
        let rect = layout.panel;
        let p = rect.shrink(22.0);
        let x = p.left();
        let w = p.width();
        let mut y = p.top();

        text(
            painter,
            Pos2::new(p.right(), p.top()),
            Align2::RIGHT_TOP,
            format!("v{VERSION}"),
            10.5,
            alpha(cur.text_dim, 0.55),
        );
        text_fit(
            painter,
            Pos2::new(x, y),
            &self.review_result,
            w - 60.0,
            15.0,
            cur.accent,
        );
        y += 24.0;
        let total = self.review_max();
        let full_move = (self.review_ply + 1) / 2;
        let total_full = (total + 1) / 2;
        text(
            painter,
            Pos2::new(x, y),
            Align2::LEFT_TOP,
            i18n::trf(
                "Ход {full} из {full_total} · полуход {ply} из {ply_total}",
                &[
                    ("full", &full_move.to_string()),
                    ("full_total", &total_full.to_string()),
                    ("ply", &self.review_ply.to_string()),
                    ("ply_total", &total.to_string()),
                ],
            ),
            10.5,
            alpha(cur.text_dim, 0.85),
        );
        y += 16.0;
        if !self.review_stats.is_empty() {
            text_fit(
                painter,
                Pos2::new(x, y),
                &self.review_stats,
                w,
                9.5,
                alpha(cur.accent, 0.85),
            );
            y += 14.0;
        }
        y += 4.0;
        divider(painter, x, y, w, cur.text_dim);
        y += 10.0;
        text(
            painter,
            Pos2::new(x, y),
            Align2::LEFT_TOP,
            "ХОДЫ",
            9.5,
            alpha(cur.text_dim, 0.85),
        );
        y += 18.0;

        // Кнопки — внизу панели, как у обычной партии.
        let second_y = p.bottom() - 46.0;
        let first_y = second_y - 46.0;
        let gap = 8.0;
        let half = (w - gap) * 0.5;
        let back_btn = Rect::from_min_size(Pos2::new(x, first_y), Vec2::new(half, 36.0));
        let fwd_btn = Rect::from_min_size(Pos2::new(x + half + gap, first_y), Vec2::new(half, 36.0));
        let start_btn = Rect::from_min_size(Pos2::new(x, second_y), Vec2::new(half, 46.0));
        let close_btn = Rect::from_min_size(Pos2::new(x + half + gap, second_y), Vec2::new(half, 46.0));

        let list_rect = Rect::from_min_max(
            Pos2::new(x, y),
            Pos2::new(p.right(), first_y - 10.0),
        );
        let labels = self.review_lines.clone();
        let current = self.review_ply;
        let mut clicked = None;
        let _ = ui.scope_builder(egui::UiBuilder::new().max_rect(list_rect), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("review_scroll")
                .auto_shrink([false, false])
                .max_height(list_rect.height())
                .show(ui, |ui| {
                    ui.set_width(w);
                    for (i, label) in labels.iter().enumerate() {
                        let selected = i == current;
                        let switched = self
                            .review_switch
                            .get(i)
                            .copied()
                            .unwrap_or(false);
                        let font = FontId::proportional(11.0);
                        let mut job = egui::text::LayoutJob::default();
                        let text = if selected {
                            format!("▶ {label}")
                        } else {
                            label.clone()
                        };
                        job.append(
                            &text,
                            0.0,
                            egui::TextFormat {
                                font_id: font.clone(),
                                color: if selected || switched {
                                    cur.accent
                                } else {
                                    alpha(cur.text, 0.92)
                                },
                                ..Default::default()
                            },
                        );
                        if let Some(ph) = self.review_phases.get(i).copied().flatten() {
                            let ph_pal = Palette::for_phase(ph);
                            job.append(
                                " · ",
                                0.0,
                                egui::TextFormat {
                                    font_id: font.clone(),
                                    color: alpha(cur.text_dim, 0.8),
                                    ..Default::default()
                                },
                            );
                            let word = if ph == Phase::Checkers {
                                i18n::tr("шашки")
                            } else {
                                i18n::tr("шахматы")
                            };
                            let word = if switched {
                                word.to_uppercase()
                            } else {
                                word.to_string()
                            };
                            job.append(
                                &word,
                                0.0,
                                egui::TextFormat {
                                    font_id: font.clone(),
                                    color: ph_pal.accent,
                                    ..Default::default()
                                },
                            );
                        }
                        let resp = ui.add(
                            egui::Button::new(job)
                                .frame(false)
                                .fill(if selected {
                                    alpha(cur.accent, 0.10)
                                } else {
                                    Color32::TRANSPARENT
                                }),
                        );
                        if resp.clicked() {
                            clicked = Some(i);
                        }
                    }
                });
        });
        if let Some(i) = clicked {
            self.set_review_ply(i);
        }
        if button(ui, back_btn, "◀ Назад", false, &cur, 12.0) && self.review_ply > 0 {
            self.set_review_ply(self.review_ply - 1);
        }
        if button(ui, fwd_btn, "Вперёд ▶", false, &cur, 12.0) && self.review_ply < total {
            self.set_review_ply(self.review_ply + 1);
        }
        if button(ui, start_btn, "К началу", false, &cur, 12.5) {
            self.set_review_ply(0);
        }
        if button(ui, close_btn, "Закрыть", true, &cur, 12.5) {
            self.exit_review();
        }
    }

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
        // Фанфара в конце партии — один раз.
        if matches!(self.game.status, game::Status::Win { .. }) && !self.win_sound_played {
            self.win_sound_played = true;
            sound::win();
        }
        }
        // сеть: события, отложенные сумерками ходы, ожидание связи
        if self.net_open_requested {
            self.net_open_requested = false;
            if self.net.is_none() {
                self.net = net::Net::new();
            }
            if self.net.is_none() {
                self.net_enabled = false;
                self.mode = Mode::Hotseat;
                self.net_status = i18n::tr("Сеть недоступна").to_string();
            }
        }
        self.net_poll();
        if self.net_wait > 0.0 {
            self.net_wait -= dt;
            // автоматическое восстановление: гость долбится сам,
            // хост сам снова слушает порт — вручную ничего нажимать не надо
            if self.net_resume && self.net_retry > 0.0 {
                self.net_retry -= dt;
                if self.net_retry <= 0.0 {
                    self.net_retry = 4.0;
                    let joiner = self.net_side == Some(Side::Black);
                    let host = self.net_side == Some(Side::White);
                    let addr = self.net_last_addr;
                    if let Some(net) = self.net.as_mut() {
                        if joiner && net.session().is_none() {
                            if let Some(a) = addr {
                                net.connect(a);
                            }
                        } else if host && !net.hosting() && net.session().is_none() {
                            net.host();
                        }
                    }
                    if joiner && addr.is_some() {
                        self.net_status =
                            i18n::tr("Пытаемся восстановить связь…").to_string();
                    }
                }
            }
            if self.net_wait <= 0.0 {
                self.net_resume = false;
                self.net_last_addr = None;
                self.net_close("связь не восстановилась");
                self.finalize_log("прервана (связь потеряна)");
            }
        }
        if self.mode == Mode::Network && self.game.trans == 0.0 {
            if let Some((mv, promo)) = self.net_pending_apply.take() {
                let mark = self.sound_mark();
                self.game.apply(mv);
                if let Some(kind) = promo {
                    if self.game.pending_promotion.is_some() {
                        self.game.choose_promotion(kind);
                    }
                }
                self.play_sound_since(mark);
                if let Some(hash) = self.net_pending_hash.take() {
                    self.net_check_hash(hash);
                }
            }
        }
        // правила и приветствие всегда отражают текущее состояние
        if let Some(net) = &self.net {
            net.set_rules(rulecode::Rules::from_game(&self.game));
            net.set_greeting(self.net_greeting.clone());
        }
        // чат живёт в памяти 60 секунд
        let now = std::time::Instant::now();
        self.net_chat.retain(|(_, _, at)| {
            now.duration_since(*at).as_secs() < net::CHAT_TTL_SECS
        });
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
        self.update_demo(dt, ctx);
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
        i18n::tr_owned(s.to_string()),
        FontId::proportional(size),
        color,
        width,
    );
    let h = galley.size().y;
    painter.galley(pos, galley, color);
    h
}

/// Текст в одну строку с автоподжатием под ширину: длинные переводы
/// уменьшаются, но не выходят за границу.
fn text_fit(painter: &egui::Painter, pos: Pos2, s: &str, max_w: f32, size: f32, color: Color32) {
    let s = i18n::tr_owned(s.to_string());
    let mut galley = painter.layout_no_wrap(s.clone(), FontId::proportional(size), color);
    if galley.size().x > max_w && max_w > 1.0 {
        let scaled = (size * max_w / galley.size().x).max(8.0);
        galley = painter.layout_no_wrap(s, FontId::proportional(scaled), color);
    }
    painter.galley(pos, galley, color);
}

fn text_fit_right(
    painter: &egui::Painter,
    pos: Pos2,
    s: &str,
    max_w: f32,
    size: f32,
    color: Color32,
) {
    let s = i18n::tr_owned(s.to_string());
    let mut galley = painter.layout_no_wrap(s.clone(), FontId::proportional(size), color);
    if galley.size().x > max_w && max_w > 1.0 {
        let scaled = (size * max_w / galley.size().x).max(8.0);
        galley = painter.layout_no_wrap(s, FontId::proportional(scaled), color);
    }
    let w = galley.size().x;
    painter.galley(pos - Vec2::new(w, 0.0), galley, color);
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
        Some("icons") => {
            // Предпросмотр иконок шашек: по одной фигуре каждого вида
            // с обеих сторон, режим «Иконки: да».
            game.board = [[None; 8]; 8];
            game.phase = Phase::Checkers;
            game.mandatory_capture = false;
            game.turn = Side::White;
            let row_white = [
                (0usize, Kind::Queen),
                (1, Kind::Rook),
                (2, Kind::Bishop),
                (3, Kind::Knight),
                (4, Kind::King),
            ];
            for (f, k) in row_white {
                game.board[4][f] = Some(Piece::new(Side::White, k));
            }
            let row_black = [
                (3usize, Kind::Queen),
                (4, Kind::Rook),
                (5, Kind::Bishop),
                (6, Kind::Knight),
                (7, Kind::King),
            ];
            for (f, k) in row_black {
                game.board[3][f] = Some(Piece::new(Side::Black, k));
            }
            // Пешка-дамка у обеих сторон: корона на пешке. Конь-дамка:
            // метка фигуры вместе с короной.
            let mut white_pawn_d = Piece::new(Side::White, Kind::Pawn);
            white_pawn_d.crowned = true;
            game.board[4][5] = Some(white_pawn_d); // f5
            let mut white_knight_d = Piece::new(Side::White, Kind::Knight);
            white_knight_d.crowned = true;
            game.board[5][2] = Some(white_knight_d); // c6
            let mut black_pawn_d = Piece::new(Side::Black, Kind::Pawn);
            black_pawn_d.crowned = true;
            game.board[2][2] = Some(black_pawn_d); // c7
            game.log.push("демо: иконки шашек".into());
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
        Some("draw") => {
            // Король против короля: оба сходили пусто — ничья.
            game.board = [[None; 8]; 8];
            game.phase = Phase::Chess;
            game.board[0][0] = Some(Piece::new(Side::White, Kind::King)); // a1
            game.board[7][7] = Some(Piece::new(Side::Black, Kind::King)); // h8
            game.turn = Side::White;
            game.apply(game::Move {
                from: (0, 0),
                to: (0, 1),
                captured: None,
            });
            game.apply(game::Move {
                from: (7, 7),
                to: (7, 6),
                captured: None,
            });
            game.log.push("демо: ничья «король против короля»".into());
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

/// Мелкий заголовок группы настроек: разрядка и приглушённый акцент.
fn group_title(painter: &egui::Painter, x: f32, y: f32, text: &str, c: Color32) {
    let s = crate::i18n::tr_owned(text.to_string()).to_uppercase();
    painter.text(
        Pos2::new(x, y),
        Align2::LEFT_TOP,
        s,
        FontId::proportional(8.5),
        alpha(c, 0.62),
    );
}

fn divider(painter: &egui::Painter, x: f32, y: f32, w: f32, c: Color32) {
    painter.line_segment(
        [Pos2::new(x, y), Pos2::new(x + w, y)],
        Stroke::new(1.0_f32, alpha(c, 0.14)),
    );
}

/// Нормализует код: верхний регистр, O→0, I/L→1, дефисы и пробелы вон.
/// None — есть посторонние символы (точка, двоеточие и т.п.): это адрес.
fn normalize_code(raw: &str) -> Option<String> {
    let mut out = String::new();
    for ch in raw.chars() {
        let up = ch.to_ascii_uppercase();
        let up = match up {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        };
        match up {
            '-' | '+' | ' ' | '_' => continue,
            '0'..='9' | 'A'..='Z' => out.push(up),
            _ => return None,
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Убирает дефисы из ID или кода — для сравнения.
fn strip_code(s: &str) -> String {
    s.chars().filter(|c| *c != '-').collect()
}

fn button(
    ui: &mut egui::Ui,
    rect: Rect,
    label: &str,
    accent: bool,
    pal: &Palette,
    font: f32,
) -> bool {
    button_resp(ui, rect, label, accent, pal, font, "").clicked()
}

/// Кнопка с подсказкой при наведении (как у ссылки в браузере).
fn button_resp(
    ui: &mut egui::Ui,
    rect: Rect,
    label: &str,
    accent: bool,
    pal: &Palette,
    font: f32,
    tip: &str,
) -> egui::Response {
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
    // Длинные переводы (немецкий, турецкий) поджимаем, чтобы текст не
    // выходил за кнопку.
    let label = i18n::tr_owned(label.to_string());
    let mut galley = ui
        .painter()
        .layout_no_wrap(label.clone(), FontId::proportional(font), fg);
    let max_w = rect.width() - 14.0;
    if galley.size().x > max_w && max_w > 1.0 {
        let scaled = (font * max_w / galley.size().x).max(8.0);
        galley = ui
            .painter()
            .layout_no_wrap(label, FontId::proportional(scaled), fg);
    }
    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, fg);
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if tip.is_empty() {
        resp
    } else {
        resp.on_hover_text(tip)
    }
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
    let mut font = font;
    let icon_r = 7.0;
    let gap = 8.0;
    let mut galley = ui
        .painter()
        .layout_no_wrap(label.clone(), FontId::proportional(font), fg);
    let max_w = rect.width() - 12.0;
    let icon_w = icon_r * 2.0 + gap;
    if icon_w + galley.size().x > max_w && max_w > icon_w + 1.0 {
        font = (font * (max_w - icon_w) / galley.size().x).max(8.0);
        galley = ui
            .painter()
            .layout_no_wrap(label, FontId::proportional(font), fg);
    }
    let total = icon_w + galley.size().x;
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

/// Маленькая иконка «копировать»: два наложенных листа.
fn draw_copy_icon(ui: &mut egui::Ui, rect: Rect, pal: &Palette, hovered: bool) {
    let painter = ui.painter();
    let color = if hovered {
        pal.accent
    } else {
        alpha(pal.text, 0.75)
    };
    let back = Rect::from_min_size(rect.min + Vec2::new(5.0, 4.0), Vec2::new(10.0, 13.0));
    let front = Rect::from_min_size(rect.min + Vec2::new(9.0, 8.0), Vec2::new(10.0, 13.0));
    // Заливаем передний лист фоном, чтобы линии не просвечивали.
    painter.rect_filled(front, 2.0, pal.frame);
    painter.rect_stroke(
        back,
        2.0,
        Stroke::new(1.3_f32, color),
        egui::StrokeKind::Inside,
    );
    painter.rect_stroke(
        front,
        2.0,
        Stroke::new(1.3_f32, color),
        egui::StrokeKind::Inside,
    );
}

fn chip(ui: &mut egui::Ui, rect: Rect, label: &str, selected: bool, pal: &Palette) -> bool {
    // В ID входит и X, и Y: иначе одинаковые подписи в разных строках
    // (например, один пак в списках звуков и музыки) конфликтуют.
    let id = ui.id().with(("chip", label, rect.left() as i32, rect.top() as i32));
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
    let label = i18n::tr_owned(label.to_string());
    let mut galley = ui
        .painter()
        .layout_no_wrap(label.clone(), FontId::proportional(12.0), fg);
    let max_w = rect.width() - 10.0;
    if galley.size().x > max_w && max_w > 1.0 {
        let scaled = (12.0 * max_w / galley.size().x).max(8.0);
        galley = ui
            .painter()
            .layout_no_wrap(label, FontId::proportional(scaled), fg);
    }
    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, fg);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn darken_keeps_alpha_and_dims_bright_pixels() {
        let mut px = vec![200u8, 210, 90, 255, 40, 40, 40, 128];
        darken_rgba(&mut px);
        assert!(
            px[0] < 200 && px[1] < 210 && px[2] < 90,
            "яркие участки темнеют"
        );
        assert_eq!(px[3], 255, "альфа не меняется");
        assert!(px[4] < 40 && px[5] < 40 && px[6] < 40, "тёмные тоже темнеют");
        assert_eq!(px[7], 128, "альфа не меняется у полупрозрачных");
    }
}
