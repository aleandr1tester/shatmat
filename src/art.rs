//! Рисование «сумеречной» доски, фигур и шашек.
//!
//! Фигуры рисуются векторами, чтобы не зависеть от шрифтов и выглядеть ровно
//! при любом размере доски.

use eframe::egui::{self, Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind};

use crate::game::{Kind, Phase, Side};

// ---------------------------------------------------------------------------
// Палитра
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg_top: Color32,
    pub bg_bottom: Color32,
    pub accent: Color32,
    pub square_light: Color32,
    pub square_dark: Color32,
    pub frame: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub white_fill: Color32,
    pub white_line: Color32,
    pub black_fill: Color32,
    pub black_line: Color32,
    pub danger: Color32,
}

impl Palette {
    pub fn chess() -> Self {
        Palette {
            bg_top: Color32::from_rgb(13, 13, 22),
            bg_bottom: Color32::from_rgb(30, 19, 31),
            accent: Color32::from_rgb(232, 179, 106),
            square_light: Color32::from_rgb(233, 219, 195),
            square_dark: Color32::from_rgb(122, 90, 72),
            frame: Color32::from_rgb(21, 19, 33),
            text: Color32::from_rgb(240, 231, 217),
            text_dim: Color32::from_rgb(156, 146, 166),
            white_fill: Color32::from_rgb(246, 235, 214),
            white_line: Color32::from_rgb(32, 24, 18),
            black_fill: Color32::from_rgb(24, 28, 43),
            black_line: Color32::from_rgb(238, 212, 168),
            danger: Color32::from_rgb(235, 106, 106),
        }
    }

    pub fn checkers() -> Self {
        Palette {
            bg_top: Color32::from_rgb(10, 16, 24),
            bg_bottom: Color32::from_rgb(15, 30, 38),
            accent: Color32::from_rgb(111, 217, 229),
            square_light: Color32::from_rgb(206, 214, 229),
            square_dark: Color32::from_rgb(72, 84, 112),
            frame: Color32::from_rgb(13, 22, 30),
            text: Color32::from_rgb(226, 240, 241),
            text_dim: Color32::from_rgb(140, 158, 168),
            white_fill: Color32::from_rgb(240, 231, 209),
            white_line: Color32::from_rgb(58, 46, 33),
            black_fill: Color32::from_rgb(24, 32, 45),
            black_line: Color32::from_rgb(169, 206, 214),
            danger: Color32::from_rgb(235, 116, 116),
        }
    }

    pub fn for_phase(phase: Phase) -> Self {
        match phase {
            Phase::Chess => Palette::chess(),
            Phase::Checkers => Palette::checkers(),
        }
    }
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        f(a.r(), b.r()),
        f(a.g(), b.g()),
        f(a.b(), b.b()),
        f(a.a(), b.a()),
    )
}

pub fn blend(a: Palette, b: Palette, t: f32) -> Palette {
    Palette {
        bg_top: mix(a.bg_top, b.bg_top, t),
        bg_bottom: mix(a.bg_bottom, b.bg_bottom, t),
        accent: mix(a.accent, b.accent, t),
        square_light: mix(a.square_light, b.square_light, t),
        square_dark: mix(a.square_dark, b.square_dark, t),
        frame: mix(a.frame, b.frame, t),
        text: mix(a.text, b.text, t),
        text_dim: mix(a.text_dim, b.text_dim, t),
        white_fill: mix(a.white_fill, b.white_fill, t),
        white_line: mix(a.white_line, b.white_line, t),
        black_fill: mix(a.black_fill, b.black_fill, t),
        black_line: mix(a.black_line, b.black_line, t),
        danger: mix(a.danger, b.danger, t),
    }
}

pub fn alpha(c: Color32, a: f32) -> Color32 {
    // Прозрачность считаем в гамма-пространстве (как в CSS): иначе egui
    // перемножает цвет в линейном пространстве и полупрозрачные слои
    // выглядят намного светлее задуманного.
    let a = a.clamp(0.0, 1.0);
    let av = (a * 255.0).round() as u8;
    Color32::from_rgba_premultiplied(
        (c.r() as f32 * a).round() as u8,
        (c.g() as f32 * a).round() as u8,
        (c.b() as f32 * a).round() as u8,
        av,
    )
}

pub fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ---------------------------------------------------------------------------
// Базовые фигуры
// ---------------------------------------------------------------------------

pub fn gradient(painter: &Painter, rect: Rect, top: Color32, bottom: Color32) {
    let steps = 48;
    for i in 0..steps {
        let t0 = i as f32 / steps as f32;
        let t1 = (i + 1) as f32 / steps as f32;
        let y0 = rect.top() + rect.height() * t0;
        let y1 = rect.top() + rect.height() * t1 + 0.8;
        let c = mix(top, bottom, (t0 + t1) * 0.5);
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(rect.left(), y0), Pos2::new(rect.right(), y1)),
            0.0,
            c,
        );
    }
}

pub fn rr(painter: &Painter, rect: Rect, radius: f32, fill: Color32, stroke: Stroke) {
    painter.rect(rect, radius, fill, stroke, StrokeKind::Inside);
}

pub fn glow_dot(painter: &Painter, center: Pos2, radius: f32, color: Color32, strength: f32) {
    for i in 0..5 {
        let t = i as f32 / 5.0;
        let r = radius * (1.0 - t * 0.55);
        painter.circle_filled(center, r, alpha(color, strength * (0.18 - t * 0.03)));
    }
}

pub fn sparkle(painter: &Painter, center: Pos2, size: f32, color: Color32, a: f32) {
    let a = a.clamp(0.0, 1.0);
    if a <= 0.01 {
        return;
    }
    painter.line_segment(
        [
            Pos2::new(center.x - size, center.y),
            Pos2::new(center.x + size, center.y),
        ],
        Stroke::new(1.0_f32, alpha(color, a * 0.9)),
    );
    painter.line_segment(
        [
            Pos2::new(center.x, center.y - size),
            Pos2::new(center.x, center.y + size),
        ],
        Stroke::new(1.0_f32, alpha(color, a * 0.9)),
    );
    painter.circle_filled(center, size * 0.22, alpha(Color32::WHITE, a * 0.8));
}

pub fn spaced_text(
    painter: &Painter,
    pos: Pos2,
    text: &str,
    size: f32,
    color: Color32,
    spacing: f32,
) -> f32 {
    let font = FontId::proportional(size);
    let text = crate::i18n::tr_owned(text.to_string());
    let mut x = pos.x;
    for ch in text.chars() {
        let galley = painter.layout_no_wrap(ch.to_string(), font.clone(), color);
        painter.galley(Pos2::new(x, pos.y), galley.clone(), color);
        x += galley.size().x + spacing;
    }
    x - pos.x
}

pub fn spaced_width(painter: &Painter, text: &str, size: f32, spacing: f32) -> f32 {
    let font = FontId::proportional(size);
    let text = crate::i18n::tr_owned(text.to_string());
    let mut w = 0.0;
    for ch in text.chars() {
        w += painter
            .layout_no_wrap(ch.to_string(), font.clone(), Color32::WHITE)
            .size()
            .x
            + spacing;
    }
    (w - spacing).max(0.0)
}

// ---------------------------------------------------------------------------
// Шахматные фигуры
// ---------------------------------------------------------------------------

fn u(_painter: &Painter, center: Pos2, size: f32, x: f32, y: f32) -> Pos2 {
    Pos2::new(
        center.x + (x - 0.5) * size,
        center.y + (y - 0.5) * size,
    )
}

fn fill_rect(
    painter: &Painter,
    center: Pos2,
    size: f32,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    radius: f32,
    fill: Color32,
    line: Color32,
    a: f32,
) {
    let rect = Rect::from_min_max(u(painter, center, size, x0, y0), u(painter, center, size, x1, y1));
    let w = (size * 0.021).max(0.8);
    painter.rect(
        rect,
        radius * size,
        alpha(fill, a),
        Stroke::new(w, alpha(line, a * 0.9)),
        StrokeKind::Inside,
    );
}

fn fill_circle(
    painter: &Painter,
    center: Pos2,
    size: f32,
    x: f32,
    y: f32,
    r: f32,
    fill: Color32,
    line: Color32,
    a: f32,
) {
    let c = u(painter, center, size, x, y);
    let w = (size * 0.021).max(0.8);
    painter.circle(
        c,
        r * size,
        alpha(fill, a),
        Stroke::new(w, alpha(line, a * 0.9)),
    );
}

fn fill_poly(
    painter: &Painter,
    center: Pos2,
    size: f32,
    pts: &[(f32, f32)],
    fill: Color32,
    line: Color32,
    a: f32,
) {
    let points: Vec<Pos2> = pts.iter().map(|(x, y)| u(painter, center, size, *x, *y)).collect();
    let w = (size * 0.021).max(0.8);
    painter.add(egui::Shape::convex_polygon(
        points,
        alpha(fill, a),
        Stroke::new(w, alpha(line, a * 0.9)),
    ));
}

pub fn draw_chess_piece(
    painter: &Painter,
    center: Pos2,
    size: f32,
    kind: Kind,
    side: Side,
    pal: &Palette,
    a: f32,
) {
    let (fill, line) = match side {
        Side::White => (pal.white_fill, pal.white_line),
        Side::Black => (pal.black_fill, pal.black_line),
    };

    // общая база
    fill_rect(painter, center, size, 0.17, 0.815, 0.83, 0.90, 0.02, fill, line, a);
    fill_rect(painter, center, size, 0.245, 0.735, 0.755, 0.825, 0.015, fill, line, a);

    match kind {
        Kind::Pawn => {
            fill_poly(
                painter,
                center,
                size,
                &[(0.37, 0.75), (0.63, 0.75), (0.57, 0.53), (0.43, 0.53)],
                fill,
                line,
                a,
            );
            fill_rect(painter, center, size, 0.37, 0.485, 0.63, 0.555, 0.014, fill, line, a);
            fill_circle(painter, center, size, 0.5, 0.36, 0.13, fill, line, a);
        }
        Kind::Rook => {
            fill_rect(painter, center, size, 0.285, 0.40, 0.715, 0.755, 0.012, fill, line, a);
            fill_rect(painter, center, size, 0.245, 0.30, 0.755, 0.42, 0.01, fill, line, a);
            fill_rect(painter, center, size, 0.245, 0.165, 0.375, 0.31, 0.008, fill, line, a);
            fill_rect(painter, center, size, 0.435, 0.165, 0.565, 0.31, 0.008, fill, line, a);
            fill_rect(painter, center, size, 0.625, 0.165, 0.755, 0.31, 0.008, fill, line, a);
            fill_rect(painter, center, size, 0.455, 0.50, 0.545, 0.615, 0.02, line, line, a * 0.9);
        }
        Kind::Knight => {
            // Классический конь смотрит влево, как на эталонной фигуре.
            fill_poly(
                painter,
                center,
                size,
                &[
                    (0.68, 0.76),
                    (0.34, 0.76),
                    (0.38, 0.58),
                    (0.44, 0.46),
                    (0.56, 0.44),
                    (0.66, 0.58),
                ],
                fill,
                line,
                a,
            );
            fill_poly(
                painter,
                center,
                size,
                &[
                    (0.56, 0.44),
                    (0.54, 0.31),
                    (0.50, 0.24),
                    (0.40, 0.25),
                    (0.31, 0.30),
                    (0.24, 0.37),
                    (0.23, 0.44),
                    (0.30, 0.50),
                    (0.44, 0.50),
                ],
                fill,
                line,
                a,
            );
            fill_poly(
                painter,
                center,
                size,
                &[(0.49, 0.25), (0.51, 0.18), (0.55, 0.26)],
                fill,
                line,
                a,
            );
            // Грива по задней стороне шеи.
            fill_poly(
                painter,
                center,
                size,
                &[
                    (0.66, 0.74),
                    (0.56, 0.46),
                    (0.59, 0.45),
                    (0.67, 0.55),
                    (0.69, 0.72),
                ],
                fill,
                line,
                a,
            );
            // глаз и ноздря
            fill_circle(painter, center, size, 0.405, 0.350, 0.016, line, line, a);
            fill_circle(painter, center, size, 0.265, 0.415, 0.009, line, line, a);
        }
        Kind::Bishop => {
            // Классическая митра: высокий купол, шарик и прорезь.
            fill_poly(
                painter,
                center,
                size,
                &[(0.38, 0.75), (0.62, 0.75), (0.565, 0.55), (0.435, 0.55)],
                fill,
                line,
                a,
            );
            fill_rect(painter, center, size, 0.36, 0.50, 0.64, 0.565, 0.014, fill, line, a);
            fill_poly(
                painter,
                center,
                size,
                &[
                    (0.40, 0.52),
                    (0.42, 0.34),
                    (0.47, 0.20),
                    (0.50, 0.14),
                    (0.53, 0.20),
                    (0.58, 0.34),
                    (0.60, 0.52),
                ],
                fill,
                line,
                a,
            );
            fill_circle(painter, center, size, 0.50, 0.125, 0.028, fill, line, a);
            painter.line_segment(
                [
                    u(painter, center, size, 0.548, 0.30),
                    u(painter, center, size, 0.468, 0.44),
                ],
                Stroke::new((size * 0.022).max(1.0), alpha(line, a * 0.95)),
            );
        }
        Kind::Queen => {
            fill_poly(
                painter,
                center,
                size,
                &[(0.33, 0.75), (0.67, 0.75), (0.60, 0.50), (0.40, 0.50)],
                fill,
                line,
                a,
            );
            fill_rect(painter, center, size, 0.355, 0.455, 0.645, 0.525, 0.014, fill, line, a);
            fill_rect(painter, center, size, 0.33, 0.37, 0.67, 0.47, 0.01, fill, line, a);
            for (i, x) in [0.345f32, 0.422, 0.5, 0.578, 0.655].iter().enumerate() {
                let tip = if i == 2 { 0.185 } else { 0.215 };
                fill_poly(
                    painter,
                    center,
                    size,
                    &[(*x - 0.038, 0.385), (*x + 0.038, 0.385), (*x, tip)],
                    fill,
                    line,
                    a,
                );
                fill_circle(painter, center, size, *x, tip - 0.008, 0.023, fill, line, a);
            }
        }
        Kind::King => {
            fill_poly(
                painter,
                center,
                size,
                &[(0.33, 0.75), (0.67, 0.75), (0.615, 0.48), (0.385, 0.48)],
                fill,
                line,
                a,
            );
            fill_rect(painter, center, size, 0.36, 0.435, 0.64, 0.505, 0.014, fill, line, a);
            fill_rect(painter, center, size, 0.345, 0.345, 0.655, 0.445, 0.01, fill, line, a);
            fill_rect(painter, center, size, 0.462, 0.075, 0.538, 0.355, 0.008, fill, line, a);
            fill_rect(painter, center, size, 0.385, 0.15, 0.615, 0.225, 0.008, fill, line, a);
        }
    }
}

// ---------------------------------------------------------------------------
// Шашки
// ---------------------------------------------------------------------------

/// Полупрозрачный контурный ореол фигуры: показывает, чей сейчас ход.
pub fn draw_piece_glow(
    painter: &Painter,
    center: Pos2,
    size: f32,
    kind: Kind,
    side: Side,
    checkers_form: bool,
    color: Color32,
    a: f32,
) {
    if a <= 0.01 {
        return;
    }
    // Тонкая контрастная кромка, чтобы дымка читалась и на близком фоне.
    let rim = match side {
        Side::White => Color32::from_rgb(64, 46, 34),
        Side::Black => Color32::from_rgb(214, 218, 228),
    };
    if checkers_form {
        // У шашки контур — круг: слои дают мягкую дымку и кромку.
        painter.circle_filled(center, size * 0.475, alpha(rim, a * 0.22));
        painter.circle_filled(center, size * 0.40, alpha(color, a));
        painter.circle_filled(center, size * 0.455, alpha(color, a * 0.35));
    } else {
        // У шахматной фигуры контур повторяет её силуэт.
        let pal = Palette {
            bg_top: color,
            bg_bottom: color,
            accent: color,
            square_light: color,
            square_dark: color,
            frame: color,
            text: color,
            text_dim: color,
            white_fill: color,
            white_line: color,
            black_fill: color,
            black_line: color,
            danger: color,
        };
        draw_chess_piece(painter, center, size * 1.18, kind, side, &pal, a * 0.22);
        draw_chess_piece(painter, center, size * 1.10, kind, side, &pal, a);
        draw_chess_piece(painter, center, size * 1.16, kind, side, &pal, a * 0.30);
    }
}

pub fn draw_checker(
    painter: &Painter,
    center: Pos2,
    size: f32,
    damka: bool,
    royal: bool,
    side: Side,
    pal: &Palette,
    a: f32,
) {
    let (rim, mid, light) = match side {
        Side::White => (
            Color32::from_rgb(139, 107, 69),
            Color32::from_rgb(214, 194, 160),
            Color32::from_rgb(244, 233, 211),
        ),
        Side::Black => (
            Color32::from_rgb(138, 157, 187),
            Color32::from_rgb(37, 45, 63),
            Color32::from_rgb(54, 64, 86),
        ),
    };

    painter.circle_filled(center, 0.355 * size, alpha(rim, a));
    painter.circle_filled(
        Pos2::new(center.x, center.y - 0.008 * size),
        0.332 * size,
        alpha(mid, a),
    );
    painter.circle_filled(
        Pos2::new(center.x, center.y - 0.022 * size),
        0.278 * size,
        alpha(light, a),
    );

    painter.circle_stroke(
        center,
        0.312 * size,
        Stroke::new((size * 0.012).max(1.0), alpha(rim, a * 0.55)),
    );

    // блик
    painter.circle_filled(
        Pos2::new(center.x - 0.10 * size, center.y - 0.13 * size),
        0.095 * size,
        alpha(Color32::WHITE, a * 0.16),
    );

    if damka {
        draw_damka_crown(painter, center, size, side, a);
    }

    if royal {
        draw_royal_dot(painter, center, size, side, a);
    }

    let _ = pal;
}

fn checker_mark_color(side: Side) -> Color32 {
    match side {
        Side::White => Color32::from_rgb(120, 88, 52),
        Side::Black => Color32::from_rgb(196, 214, 238),
    }
}

/// Только корона дамки: её можно дорисовать поверх своей картинки шашки.
pub fn draw_damka_crown(painter: &Painter, center: Pos2, size: f32, side: Side, a: f32) {
    let mark = checker_mark_color(side);
    painter.circle_stroke(
        center,
        0.205 * size,
        Stroke::new((size * 0.020).max(1.2), alpha(mark, a * 0.9)),
    );
    // корона из трёх зубцов
    let base = Rect::from_min_max(
        u(painter, center, size, 0.415, 0.525),
        u(painter, center, size, 0.585, 0.565),
    );
    painter.rect_filled(base, 0.0, alpha(mark, a * 0.9));
    for (x0, xt, x1) in [(0.415f32, 0.443, 0.465), (0.475, 0.5, 0.525), (0.535, 0.557, 0.585)] {
        let pts = vec![
            u(painter, center, size, x0, 0.532),
            u(painter, center, size, xt, 0.452),
            u(painter, center, size, x1, 0.532),
        ];
        painter.add(egui::Shape::convex_polygon(
            pts,
            alpha(mark, a * 0.9),
            Stroke::NONE,
        ));
    }
}

/// Маленькая «королевская» точка: короля видно и среди обычных шашек.
pub fn draw_royal_dot(painter: &Painter, center: Pos2, size: f32, side: Side, a: f32) {
    let mark = checker_mark_color(side);
    let dot = u(painter, center, size, 0.5, 0.405);
    painter.circle_filled(dot, size * 0.026, alpha(mark, a * 0.95));
    painter.circle_stroke(
        dot,
        size * 0.045,
        Stroke::new((size * 0.012).max(1.0), alpha(mark, a * 0.8)),
    );
}

// ---------------------------------------------------------------------------
// Текст
// ---------------------------------------------------------------------------

pub fn text(
    painter: &Painter,
    pos: Pos2,
    anchor: Align2,
    s: impl Into<String>,
    size: f32,
    color: Color32,
) {
    let s = crate::i18n::tr_owned(s.into());
    painter.text(pos, anchor, s, FontId::proportional(size), color);
}

/// Маленькая эмблема шахматной фигуры на шашке (кроме пешки).
pub fn draw_checker_icon(
    painter: &Painter,
    center: Pos2,
    size: f32,
    kind: Kind,
    side: Side,
    a: f32,
) {
    if a <= 0.01 || kind == Kind::Pawn {
        return;
    }
    let (fill, line) = if side == Side::White {
        (
            Color32::from_rgb(62, 46, 30),
            Color32::from_rgb(245, 222, 180),
        )
    } else {
        (
            Color32::from_rgb(238, 228, 208),
            Color32::from_rgb(40, 32, 24),
        )
    };
    let s = size * 0.44;
    match kind {
        Kind::King => {
            fill_poly(
                painter,
                center,
                s,
                &[
                    (-0.46, 0.26),
                    (-0.46, -0.02),
                    (-0.24, 0.10),
                    (0.0, -0.24),
                    (0.24, 0.10),
                    (0.46, -0.02),
                    (0.46, 0.26),
                ],
                fill,
                line,
                a,
            );
            fill_poly(
                painter,
                center,
                s,
                &[(-0.06, -0.48), (-0.06, -0.24), (0.06, -0.24), (0.06, -0.48)],
                fill,
                line,
                a,
            );
            fill_poly(
                painter,
                center,
                s,
                &[(-0.20, -0.37), (0.20, -0.37), (0.20, -0.27), (-0.20, -0.27)],
                fill,
                line,
                a,
            );
        }
        Kind::Queen => {
            fill_poly(
                painter,
                center,
                s,
                &[
                    (-0.46, 0.26),
                    (-0.46, -0.06),
                    (-0.28, 0.06),
                    (-0.14, -0.28),
                    (0.0, 0.0),
                    (0.14, -0.28),
                    (0.28, 0.06),
                    (0.46, -0.06),
                    (0.46, 0.26),
                ],
                fill,
                line,
                a,
            );
        }
        Kind::Rook => {
            fill_poly(
                painter,
                center,
                s,
                &[(-0.40, 0.28), (-0.40, -0.14), (0.40, -0.14), (0.40, 0.28)],
                fill,
                line,
                a,
            );
            for x in [-0.36_f32, -0.12, 0.12] {
                fill_poly(
                    painter,
                    center,
                    s,
                    &[
                        (x, -0.34),
                        (x + 0.16, -0.34),
                        (x + 0.16, -0.14),
                        (x, -0.14),
                    ],
                    fill,
                    line,
                    a,
                );
            }
        }
        Kind::Bishop => {
            fill_poly(
                painter,
                center,
                s,
                &[(-0.28, 0.26), (0.0, -0.32), (0.28, 0.26)],
                fill,
                line,
                a,
            );
            fill_circle(painter, center, s, 0.0, -0.38, 0.09, fill, line, a);
        }
        Kind::Knight => {
            fill_poly(
                painter,
                center,
                s,
                &[
                    (-0.30, 0.28),
                    (-0.30, 0.0),
                    (-0.04, 0.08),
                    (0.0, 0.28),
                ],
                fill,
                line,
                a,
            );
            fill_poly(
                painter,
                center,
                s,
                &[
                    (-0.30, 0.02),
                    (-0.38, -0.16),
                    (-0.28, -0.34),
                    (0.02, -0.40),
                    (0.22, -0.20),
                    (0.14, 0.04),
                ],
                fill,
                line,
                a,
            );
            fill_poly(
                painter,
                center,
                s,
                &[(0.02, -0.36), (0.08, -0.50), (0.15, -0.34)],
                fill,
                line,
                a,
            );
            fill_circle(painter, center, s, -0.08, -0.24, 0.035, line, line, a);
        }
        Kind::Pawn => {}
    }
}

