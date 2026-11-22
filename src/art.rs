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

// ---------------------------------------------------------------------------
// Набор фигур: контуры, потрейсенные из SVG (Staunton, с вырезами).
// Координаты в коробке фигуры 0..1, низ — 0.905, высота — 0.80.
// ---------------------------------------------------------------------------

pub const KING_TRACE: [(f32, f32); 47] = [
    (0.4729, 0.1269), (0.4628, 0.1470), (0.4278, 0.1519), (0.4203, 0.1594), (0.4240, 0.1956),
    (0.4729, 0.2373), (0.4149, 0.3119), (0.4020, 0.3847), (0.3501, 0.3552), (0.3147, 0.3461),
    (0.2578, 0.3456), (0.2053, 0.3618), (0.1728, 0.3883), (0.1422, 0.4345), (0.1330, 0.4742),
    (0.1374, 0.5183), (0.2274, 0.6825), (0.2419, 0.7274), (0.2588, 0.8343), (0.2803, 0.8656),
    (0.3418, 0.8909), (0.4393, 0.9037), (0.5587, 0.9032), (0.6628, 0.8904), (0.7122, 0.8732),
    (0.7400, 0.8459), (0.7705, 0.6983), (0.8612, 0.5304), (0.8675, 0.4932), (0.8602, 0.4360),
    (0.8319, 0.3887), (0.8111, 0.3707), (0.7599, 0.3480), (0.7010, 0.3448), (0.6705, 0.3513),
    (0.6014, 0.3838), (0.5879, 0.3118), (0.5450, 0.2633), (0.5342, 0.2409), (0.5416, 0.2227),
    (0.5755, 0.1950), (0.5781, 0.1565), (0.5379, 0.1473), (0.5241, 0.1165), (0.5107, 0.1068),
    (0.4856, 0.1062), (0.4729, 0.1269),
];
pub const KING_HOLES: [&[(f32, f32)]; 4] = [
    &KING_H0,
    &KING_H1,
    &KING_H2,
    &KING_H3,
];
pub const KING_H0: [(f32, f32); 34] = [
    (0.3425, 0.3986), (0.3957, 0.4345), (0.4356, 0.4827), (0.4592, 0.5392), (0.4725, 0.6091),
    (0.4896, 0.6220), (0.5111, 0.6228), (0.5289, 0.6084), (0.5439, 0.5289), (0.5728, 0.4743),
    (0.6148, 0.4285), (0.6646, 0.3969), (0.7171, 0.3852), (0.7525, 0.3893), (0.7813, 0.4013),
    (0.8028, 0.4206), (0.8162, 0.4464), (0.8192, 0.4940), (0.8078, 0.5308), (0.7750, 0.5902),
    (0.7320, 0.6403), (0.6951, 0.6630), (0.6379, 0.6672), (0.5000, 0.6554), (0.3809, 0.6663),
    (0.3082, 0.6654), (0.2555, 0.6240), (0.2054, 0.5587), (0.1811, 0.4971), (0.1800, 0.4601),
    (0.2002, 0.4164), (0.2399, 0.3922), (0.2907, 0.3856), (0.3425, 0.3986),
];
pub const KING_H1: [(f32, f32); 16] = [
    (0.2542, 0.4193), (0.2197, 0.4433), (0.2053, 0.4808), (0.2167, 0.5239), (0.2578, 0.5849),
    (0.2972, 0.6218), (0.3221, 0.6321), (0.3875, 0.6347), (0.4303, 0.6218), (0.4384, 0.5991),
    (0.4316, 0.5543), (0.3960, 0.4762), (0.3717, 0.4510), (0.3291, 0.4251), (0.2836, 0.4156),
    (0.2542, 0.4193),
];
pub const KING_H2: [(f32, f32); 15] = [
    (0.6890, 0.4186), (0.6380, 0.4432), (0.6094, 0.4713), (0.5866, 0.5069), (0.5639, 0.5861),
    (0.5673, 0.6167), (0.5796, 0.6275), (0.6368, 0.6340), (0.6911, 0.6281), (0.7428, 0.5849),
    (0.7796, 0.5320), (0.7940, 0.4835), (0.7823, 0.4465), (0.7471, 0.4196), (0.6890, 0.4186),
];
pub const KING_H3: [(f32, f32); 14] = [
    (0.6529, 0.7759), (0.6746, 0.7866), (0.6791, 0.7960), (0.6683, 0.8056), (0.6491, 0.8082),
    (0.4997, 0.8018), (0.3524, 0.8083), (0.3268, 0.8037), (0.3191, 0.7926), (0.3287, 0.7827),
    (0.3571, 0.7747), (0.4479, 0.7657), (0.5814, 0.7667), (0.6529, 0.7759),
];

pub const QUEEN_TRACE: [(f32, f32); 77] = [
    (0.4572, 0.1177), (0.4409, 0.1372), (0.4352, 0.1632), (0.4550, 0.2987), (0.4311, 0.4726),
    (0.3737, 0.3200), (0.3668, 0.2732), (0.3669, 0.1871), (0.3567, 0.1644), (0.3383, 0.1476),
    (0.3014, 0.1370), (0.2746, 0.1424), (0.2438, 0.1638), (0.2328, 0.1911), (0.2323, 0.2262),
    (0.2704, 0.2801), (0.2808, 0.3143), (0.2836, 0.4726), (0.2345, 0.4157), (0.2127, 0.3773),
    (0.2039, 0.3393), (0.2044, 0.2713), (0.1796, 0.2318), (0.1519, 0.2171), (0.1207, 0.2193),
    (0.0875, 0.2397), (0.0726, 0.2624), (0.0682, 0.2881), (0.0742, 0.3143), (0.1235, 0.3758),
    (0.1710, 0.5831), (0.2594, 0.7238), (0.2334, 0.8201), (0.2341, 0.8583), (0.2515, 0.8739),
    (0.3083, 0.8926), (0.4067, 0.9030), (0.6325, 0.9006), (0.7357, 0.8812), (0.7604, 0.8666),
    (0.7708, 0.8461), (0.7400, 0.7190), (0.8294, 0.5826), (0.8770, 0.3712), (0.9271, 0.3130),
    (0.9318, 0.2841), (0.9249, 0.2561), (0.9065, 0.2331), (0.8672, 0.2172), (0.8352, 0.2245),
    (0.8060, 0.2488), (0.7934, 0.2723), (0.7975, 0.3353), (0.7905, 0.3686), (0.7593, 0.4224),
    (0.7174, 0.4726), (0.7193, 0.3132), (0.7340, 0.2727), (0.7619, 0.2386), (0.7695, 0.2046),
    (0.7583, 0.1712), (0.7330, 0.1459), (0.6983, 0.1363), (0.6581, 0.1484), (0.6397, 0.1675),
    (0.6342, 0.1879), (0.6322, 0.2871), (0.6233, 0.3310), (0.5677, 0.4712), (0.5455, 0.2976),
    (0.5489, 0.2427), (0.5643, 0.1806), (0.5616, 0.1444), (0.5485, 0.1232), (0.5248, 0.1087),
    (0.4844, 0.1063), (0.4572, 0.1177),
];
pub const QUEEN_HOLES: [&[(f32, f32)]; 7] = [
    &QUEEN_H0,
    &QUEEN_H1,
    &QUEEN_H2,
    &QUEEN_H3,
    &QUEEN_H4,
    &QUEEN_H5,
    &QUEEN_H6,
];
pub const QUEEN_H0: [(f32, f32); 12] = [
    (0.6645, 0.7499), (0.7079, 0.7742), (0.7030, 0.7876), (0.6842, 0.7941), (0.4958, 0.7899),
    (0.3239, 0.7949), (0.2980, 0.7896), (0.2911, 0.7773), (0.3260, 0.7558), (0.3939, 0.7423),
    (0.5607, 0.7414), (0.6645, 0.7499),
];
pub const QUEEN_H1: [(f32, f32); 11] = [
    (0.6927, 0.6002), (0.7188, 0.6161), (0.7242, 0.6394), (0.3305, 0.6371), (0.2855, 0.6335),
    (0.2774, 0.6256), (0.2866, 0.6104), (0.3150, 0.5993), (0.3963, 0.5899), (0.5789, 0.5900),
    (0.6927, 0.6002),
];
pub const QUEEN_H2: [(f32, f32); 9] = [
    (0.8842, 0.2673), (0.8947, 0.2873), (0.8834, 0.3116), (0.8630, 0.3185), (0.8426, 0.3092),
    (0.8334, 0.2872), (0.8405, 0.2663), (0.8641, 0.2571), (0.8842, 0.2673),
];
pub const QUEEN_H3: [(f32, f32); 9] = [
    (0.5231, 0.1479), (0.5311, 0.1718), (0.5211, 0.1946), (0.5044, 0.2006), (0.4810, 0.1900),
    (0.4704, 0.1667), (0.4764, 0.1500), (0.4993, 0.1399), (0.5231, 0.1479),
];
pub const QUEEN_H4: [(f32, f32); 9] = [
    (0.7194, 0.1829), (0.7294, 0.2009), (0.7238, 0.2287), (0.6973, 0.2345), (0.6750, 0.2245),
    (0.6692, 0.2052), (0.6764, 0.1913), (0.7000, 0.1754), (0.7194, 0.1829),
];
pub const QUEEN_H5: [(f32, f32); 9] = [
    (0.1593, 0.2646), (0.1673, 0.2884), (0.1573, 0.3113), (0.1360, 0.3164), (0.1150, 0.3067),
    (0.1077, 0.2887), (0.1164, 0.2666), (0.1370, 0.2563), (0.1593, 0.2646),
];
pub const QUEEN_H6: [(f32, f32); 9] = [
    (0.3268, 0.1905), (0.3330, 0.2111), (0.3250, 0.2270), (0.3057, 0.2345), (0.2760, 0.2268),
    (0.2715, 0.2009), (0.2815, 0.1829), (0.3060, 0.1782), (0.3268, 0.1905),
];

pub const ROOK_TRACE: [(f32, f32); 35] = [
    (0.2072, 0.1261), (0.1888, 0.1598), (0.1868, 0.2605), (0.3000, 0.3721), (0.3000, 0.6370),
    (0.1566, 0.7804), (0.1612, 0.8816), (0.1904, 0.8976), (0.3236, 0.9048), (0.7374, 0.9040),
    (0.8289, 0.8911), (0.8431, 0.8682), (0.8433, 0.7804), (0.7075, 0.6446), (0.7075, 0.3721),
    (0.8132, 0.2680), (0.8124, 0.1626), (0.8037, 0.1300), (0.7718, 0.1082), (0.6853, 0.1072),
    (0.6593, 0.1202), (0.6500, 0.1548), (0.6168, 0.1575), (0.6046, 0.1531), (0.5998, 0.1249),
    (0.5848, 0.1111), (0.4608, 0.1057), (0.4156, 0.1146), (0.4029, 0.1531), (0.3792, 0.1578),
    (0.3563, 0.1536), (0.3462, 0.1238), (0.3160, 0.1083), (0.2385, 0.1070), (0.2072, 0.1261),
];
pub const ROOK_HOLES: [&[(f32, f32)]; 2] = [
    &ROOK_H0,
    &ROOK_H1,
];
pub const ROOK_H0: [(f32, f32); 6] = [
    (0.6547, 0.3880), (0.6547, 0.4144), (0.3491, 0.4106), (0.3445, 0.3616), (0.6547, 0.3616),
    (0.6547, 0.3880),
];
pub const ROOK_H1: [(f32, f32); 6] = [
    (0.6547, 0.6332), (0.6547, 0.6559), (0.3453, 0.6559), (0.3453, 0.6106), (0.6547, 0.6106),
    (0.6547, 0.6332),
];

pub const BISHOP_TRACE: [(f32, f32); 52] = [
    (0.4506, 0.1171), (0.4295, 0.1402), (0.4215, 0.1719), (0.4275, 0.2072), (0.4424, 0.2375),
    (0.3870, 0.2685), (0.3093, 0.3402), (0.2858, 0.3765), (0.2738, 0.4257), (0.2831, 0.4900),
    (0.3393, 0.5900), (0.3261, 0.6554), (0.3261, 0.6902), (0.3441, 0.7080), (0.3914, 0.7222),
    (0.3375, 0.7379), (0.1870, 0.7497), (0.1606, 0.7673), (0.1245, 0.8139), (0.1714, 0.8798),
    (0.1997, 0.9032), (0.2523, 0.8864), (0.4238, 0.8573), (0.4582, 0.8450), (0.5010, 0.8177),
    (0.5835, 0.8572), (0.7452, 0.8846), (0.7959, 0.9043), (0.8082, 0.9031), (0.8478, 0.8593),
    (0.8755, 0.8145), (0.8349, 0.7638), (0.8005, 0.7456), (0.6639, 0.7377), (0.6080, 0.7215),
    (0.6589, 0.7064), (0.6806, 0.6859), (0.6595, 0.5878), (0.7009, 0.5247), (0.7222, 0.4740),
    (0.7276, 0.4265), (0.7170, 0.3812), (0.6904, 0.3374), (0.6623, 0.3075), (0.5626, 0.2399),
    (0.5806, 0.1749), (0.5744, 0.1475), (0.5570, 0.1228), (0.5405, 0.1100), (0.5200, 0.1054),
    (0.4799, 0.1071), (0.4506, 0.1171),
];
pub const BISHOP_HOLES: [&[(f32, f32)]; 4] = [
    &BISHOP_H0,
    &BISHOP_H1,
    &BISHOP_H2,
    &BISHOP_H3,
];
pub const BISHOP_H0: [(f32, f32); 16] = [
    (0.5201, 0.3547), (0.5231, 0.3771), (0.5615, 0.3841), (0.5625, 0.4144), (0.5555, 0.4225),
    (0.5251, 0.4301), (0.5170, 0.4725), (0.4881, 0.4735), (0.4813, 0.4651), (0.4778, 0.4274),
    (0.4468, 0.4199), (0.4405, 0.3855), (0.4779, 0.3749), (0.4854, 0.3295), (0.5172, 0.3326),
    (0.5201, 0.3547),
];
pub const BISHOP_H1: [(f32, f32); 10] = [
    (0.6041, 0.6120), (0.6135, 0.6213), (0.6071, 0.6349), (0.5498, 0.6424), (0.4359, 0.6439),
    (0.3926, 0.6356), (0.3850, 0.6265), (0.3965, 0.6152), (0.4209, 0.6115), (0.6041, 0.6120),
];
pub const BISHOP_H2: [(f32, f32); 10] = [
    (0.6124, 0.5483), (0.6073, 0.5642), (0.5567, 0.5682), (0.4125, 0.5659), (0.3882, 0.5623),
    (0.3858, 0.5545), (0.3947, 0.5437), (0.4674, 0.5371), (0.5624, 0.5386), (0.6124, 0.5483),
];
pub const BISHOP_H3: [(f32, f32); 9] = [
    (0.5213, 0.1598), (0.5311, 0.1783), (0.5206, 0.2008), (0.5017, 0.2072), (0.4828, 0.1986),
    (0.4742, 0.1782), (0.4808, 0.1588), (0.5027, 0.1503), (0.5213, 0.1598),
];

pub const KNIGHT_TRACE: [(f32, f32); 35] = [
    (0.4300, 0.1342), (0.4061, 0.1633), (0.3265, 0.1310), (0.2558, 0.1237), (0.2565, 0.2110),
    (0.2448, 0.2653), (0.1081, 0.5819), (0.0925, 0.6320), (0.0974, 0.6577), (0.1204, 0.6886),
    (0.1547, 0.7131), (0.2121, 0.7308), (0.2457, 0.7252), (0.2646, 0.7478), (0.2858, 0.7535),
    (0.3038, 0.7393), (0.3933, 0.6254), (0.4448, 0.5912), (0.4801, 0.5783), (0.4546, 0.6418),
    (0.3530, 0.7966), (0.3253, 0.9013), (0.8988, 0.9050), (0.9083, 0.7879), (0.8961, 0.6474),
    (0.8650, 0.5118), (0.8154, 0.3999), (0.7694, 0.3332), (0.6912, 0.2579), (0.6348, 0.2261),
    (0.5434, 0.2020), (0.5143, 0.1457), (0.4844, 0.1141), (0.4616, 0.1054), (0.4300, 0.1342),
];
pub const KNIGHT_HOLES: [&[(f32, f32)]; 3] = [
    &KNIGHT_H0,
    &KNIGHT_H1,
    &KNIGHT_H2,
];
pub const KNIGHT_H0: [(f32, f32); 22] = [
    (0.5855, 0.2807), (0.6346, 0.3058), (0.6857, 0.3499), (0.7560, 0.4398), (0.7922, 0.5079),
    (0.8230, 0.6075), (0.8377, 0.7008), (0.8399, 0.7985), (0.8331, 0.8286), (0.8208, 0.8420),
    (0.8016, 0.8358), (0.7934, 0.8178), (0.7868, 0.7056), (0.7724, 0.6070), (0.7485, 0.5237),
    (0.7196, 0.4661), (0.6409, 0.3678), (0.5957, 0.3323), (0.5129, 0.2864), (0.5170, 0.2772),
    (0.5346, 0.2708), (0.5855, 0.2807),
];
pub const KNIGHT_H1: [(f32, f32); 9] = [
    (0.3679, 0.3420), (0.3514, 0.3717), (0.3147, 0.4064), (0.2776, 0.4136), (0.2598, 0.4041),
    (0.2668, 0.3746), (0.3054, 0.3360), (0.3373, 0.3308), (0.3679, 0.3420),
];
pub const KNIGHT_H2: [(f32, f32); 8] = [
    (0.1758, 0.6119), (0.1617, 0.6422), (0.1474, 0.6509), (0.1307, 0.6502), (0.1314, 0.6269),
    (0.1470, 0.6058), (0.1663, 0.5984), (0.1758, 0.6119),
];

pub const PAWN_TRACE: [(f32, f32); 33] = [
    (0.4386, 0.1196), (0.4020, 0.1490), (0.3842, 0.1886), (0.3857, 0.2358), (0.4012, 0.2873),
    (0.3446, 0.3286), (0.3256, 0.3604), (0.3151, 0.3974), (0.3140, 0.4360), (0.3233, 0.4730),
    (0.3762, 0.5406), (0.2866, 0.6133), (0.2354, 0.6886), (0.2162, 0.7563), (0.2058, 0.9050),
    (0.7942, 0.9050), (0.7846, 0.7657), (0.7630, 0.6861), (0.7088, 0.6096), (0.6198, 0.5404),
    (0.6531, 0.5068), (0.6716, 0.4766), (0.6841, 0.4431), (0.6867, 0.4109), (0.6642, 0.3444),
    (0.6423, 0.3180), (0.5988, 0.2844), (0.6122, 0.2098), (0.6016, 0.1616), (0.5734, 0.1289),
    (0.5331, 0.1092), (0.4863, 0.1051), (0.4386, 0.1196),
];
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

    if damka {
        draw_damka_crown(painter, center, size, side, a);
    }

    if royal {
        draw_royal_dot(painter, center, size, side, a);
    }

    let _ = pal;
}

fn checker_mark_color(side: Side) -> Color32 {
    // Корона дамки — красная у обеих сторон.
    let _ = side;
    Color32::from_rgb(214, 54, 54)
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

/// Маленькая эмблема фигуры на шашке (кроме пешки): мини-контуры набора.
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
    let empty: [&[(f32, f32)]; 0] = [];
    let (trace, _holes): (&[(f32, f32)], &[&[(f32, f32)]]) = match kind {
        Kind::King => (&KING_TRACE, &KING_HOLES[..]),
        Kind::Queen => (&QUEEN_TRACE, &QUEEN_HOLES[..]),
        Kind::Rook => (&ROOK_TRACE, &ROOK_HOLES[..]),
        Kind::Bishop => (&BISHOP_TRACE, &BISHOP_HOLES[..]),
        Kind::Knight => (&KNIGHT_TRACE, &KNIGHT_HOLES[..]),
        Kind::Pawn => (&PAWN_TRACE, &empty[..]),
    };
    let s = size * 0.62;
    // Трейс занимает y 0.105..0.905, ширина до ~0.86 — центрируем в метке.
    let pts: Vec<Pos2> = trace
        .iter()
        .map(|(x, y)| {
            u(
                painter,
                center,
                s,
                0.5 + (x - 0.5) * 0.92,
                0.5 + (y - 0.505) * 0.92,
            )
        })
        .collect();
    let w = (s * 0.05).max(0.8);
    painter.add(egui::Shape::convex_polygon(
        pts,
        alpha(fill, a),
        Stroke::new(w, alpha(line, a * 0.9)),
    ));
}

