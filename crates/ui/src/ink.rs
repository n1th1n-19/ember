// SPDX-License-Identifier: GPL-3.0-or-later
//! Comic print primitives: palette, fonts, panels, captions, textures, bursts.
//! Rules (DESIGN.md): hard shadows, no blur/gradients, motion snaps to 83ms poses.
use cosmic::iced::{Color, Font, Point, Radians, Rectangle, Size, Vector};
use cosmic::iced::alignment::Vertical;
use cosmic::iced::widget::canvas::{Frame, Path, Stroke, Text};
use cosmic::iced::advanced::text::Alignment;
use std::f32::consts::{PI, TAU};

pub const INK: Color = rgb(0x11, 0x11, 0x11);
pub const PAPER: Color = rgb(0xF6, 0xEF, 0xE0);
pub const RED: Color = rgb(0xE8, 0x20, 0x2A);
pub const MAGENTA: Color = rgb(0xFF, 0x2E, 0x88);
pub const CYAN: Color = rgb(0x00, 0xC2, 0xFF);
pub const YELLOW: Color = rgb(0xFF, 0xD4, 0x00);
pub const WHITE: Color = rgb(0xFF, 0xFF, 0xFF);
pub const GREY: Color = rgb(0xB8, 0xB2, 0xA6);

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: 1.0 }
}

pub const DISPLAY: Font = Font::with_name("Bangers");
pub const MARKER: Font = Font::with_name("Permanent Marker");
pub const BODY: Font = Font::with_name("Space Grotesk");

/// Font files to register at startup (`cosmic::iced::font::load`).
pub const FONT_FILES: [&[u8]; 3] = [
    include_bytes!("../../../res/fonts/Bangers-Regular.ttf"),
    include_bytes!("../../../res/fonts/PermanentMarker-Regular.ttf"),
    include_bytes!("../../../res/fonts/SpaceGrotesk.ttf"),
];

/// Page-level colors that change between light and dark. Accents (red, cyan, yellow,
/// magenta) and the machines themselves keep their print colors in both themes.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub page: Color,
    pub panel: Color,
    pub text: Color,
    pub chart_bg: Color,
    pub chart_line: Color,
    pub track: Color,
    /// floor lines, speed lines, loose marks drawn straight onto an art ground
    pub scene_line: Color,
}

pub const LIGHT: Palette = Palette {
    dark: false,
    page: PAPER,
    panel: PAPER,
    text: INK,
    chart_bg: WHITE,
    chart_line: INK,
    track: PAPER,
    scene_line: INK,
};

pub const DARK: Palette = Palette {
    dark: true,
    page: rgb(0x17, 0x15, 0x1E),
    panel: rgb(0x22, 0x1F, 0x2C),
    text: PAPER,
    chart_bg: rgb(0x13, 0x11, 0x19),
    chart_line: PAPER,
    track: rgb(0x2E, 0x2A, 0x3A),
    scene_line: PAPER,
};

static DARK_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Current palette (dark by default).
pub fn pal() -> Palette {
    if DARK_MODE.load(std::sync::atomic::Ordering::Relaxed) { DARK } else { LIGHT }
}

pub fn set_dark(on: bool) {
    DARK_MODE.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// Art-box ground: the bright accent in light mode; at night a deep color that pairs
/// with it (darkened yellow turns to mud, so yellow maps to violet). The accent itself
/// stays in the halftone corner.
pub fn ground(accent: Color) -> Color {
    if !pal().dark {
        return accent;
    }
    match accent {
        a if a == YELLOW => rgb(0x2B, 0x20, 0x48),
        a if a == CYAN => rgb(0x0A, 0x33, 0x4A),
        a if a == MAGENTA => rgb(0x46, 0x10, 0x2D),
        a if a == RED => rgb(0x4A, 0x12, 0x18),
        a => Color { r: a.r * 0.3 + 0.06, g: a.g * 0.3 + 0.06, b: a.b * 0.3 + 0.08, a: 1.0 },
    }
}

/// Register Ember's fonts in iced's global font system. Call once at startup,
/// before any window exists, so no text gets shaped with a fallback first.
pub fn load_fonts() {
    let mut fs = cosmic::iced::advanced::graphics::text::font_system().write().expect("font system lock");
    for bytes in FONT_FILES {
        fs.load_font(std::borrow::Cow::Borrowed(bytes));
    }
}

/// One pose = 83ms ("on twos" at 24fps). Animation counters are in poses.
pub const POSE_MS: u64 = 83;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Calm,
    Busy,
    Hot,
}

impl Level {
    pub fn from_percent(p: f32) -> Self {
        if p > 85.0 { Level::Hot } else if p >= 50.0 { Level::Busy } else { Level::Calm }
    }
    pub fn from_celsius(c: f32) -> Self {
        if c > 80.0 { Level::Hot } else if c >= 60.0 { Level::Busy } else { Level::Calm }
    }
    pub fn color(self) -> Color {
        match self {
            Level::Calm => CYAN,
            Level::Busy => YELLOW,
            Level::Hot => RED,
        }
    }
}

pub fn stroke(width: f32) -> Stroke<'static> {
    Stroke::default().with_color(INK).with_width(width)
}

pub fn poly(points: &[Point]) -> Path {
    Path::new(|b| {
        if let Some((first, rest)) = points.split_first() {
            b.move_to(*first);
            rest.iter().for_each(|p| b.line_to(*p));
            b.close();
        }
    })
}

pub fn rect(r: Rectangle) -> Path {
    Path::rectangle(r.position(), r.size())
}

pub fn inset(r: Rectangle, d: f32) -> Rectangle {
    Rectangle::new(Point::new(r.x + d, r.y + d), Size::new((r.width - 2.0 * d).max(0.0), (r.height - 2.0 * d).max(0.0)))
}

/// Filled ink box with a hard offset shadow and thick outline.
pub fn boxed(f: &mut Frame, r: Rectangle, ground: Color, border: f32, shadow: f32) {
    if shadow > 0.0 {
        f.fill(&rect(r + Vector::new(shadow, shadow)), INK);
    }
    f.fill(&rect(r), ground);
    f.stroke(&rect(r), stroke(border));
}

/// A comic panel: 5px border, 6px shadow.
pub fn panel(f: &mut Frame, r: Rectangle, ground: Color) {
    boxed(f, r, ground, 5.0, 6.0);
}

#[allow(clippy::too_many_arguments)]
pub fn text(f: &mut Frame, s: impl Into<String>, at: Point, size: f32, font: Font, color: Color, align: Alignment, valign: Vertical) {
    f.fill_text(Text {
        content: s.into(),
        position: at,
        size: size.into(),
        font,
        color,
        align_x: align,
        align_y: valign,
        ..Text::default()
    });
}

/// Rough advance width for layout of boxes around text (Bangers is narrow).
pub fn approx_width(s: &str, size: f32, font: Font) -> f32 {
    let em = if font == DISPLAY { 0.46 } else if font == MARKER { 0.62 } else { 0.56 };
    s.chars().count() as f32 * size * em
}

/// Yellow caption tag, tilted 1°, Bangers text. Returns the box drawn.
pub fn caption(f: &mut Frame, at: Point, s: &str, size: f32, ground: Color) -> Rectangle {
    let w = approx_width(s, size, DISPLAY) + size * 0.9;
    let h = size * 1.35;
    let r = Rectangle::new(at, Size::new(w, h));
    f.with_save(|f| {
        f.translate(Vector::new(at.x, at.y));
        f.rotate(Radians(-PI / 180.0));
        f.translate(Vector::new(-at.x, -at.y));
        boxed(f, r, ground, 3.0, 4.0);
        let fg = if ground == INK || ground == RED { PAPER } else { INK };
        text(f, s, Point::new(r.center_x(), r.center_y()), size, DISPLAY, fg, Alignment::Center, Vertical::Center);
    });
    r
}

/// Big display number with cyan-left / magenta-right misprint and ink drop. Use only at ≥48px.
pub fn misprint(f: &mut Frame, s: &str, at: Point, size: f32, fill: Color, align: Alignment, wobble: u64) {
    // 3-pose misprint cycle (anim-misprint); wobble = pose counter, 0 when motion is reduced.
    let (dc, dm) = match wobble % 4 {
        1 => (Vector::new(-5.0, 1.0), Vector::new(3.0, -1.0)),
        3 => (Vector::new(-2.5, -1.0), Vector::new(5.0, 1.0)),
        _ => (Vector::new(-3.5, 0.0), Vector::new(3.5, 0.0)),
    };
    let k = size / 72.0;
    let t = |f: &mut Frame, d: Vector, c: Color| {
        text(f, s, at + d * k, size, DISPLAY, c, align, Vertical::Top);
    };
    t(f, Vector::new(6.0, 6.0), INK);
    t(f, dc, CYAN);
    t(f, dm, MAGENTA);
    // fake ink outline: 4 tight offsets
    for d in [Vector::new(-1.5, 0.0), Vector::new(1.5, 0.0), Vector::new(0.0, -1.5), Vector::new(0.0, 1.5)] {
        t(f, d, INK);
    }
    t(f, Vector::new(0.0, 0.0), fill);
}

/// Halftone dots filling a rectangle. `density` 0..1 scales dot radius.
pub fn halftone(f: &mut Frame, r: Rectangle, color: Color, spacing: f32, density: f32) {
    halftone_where(f, r, color, spacing, density, |_| true);
}

/// Halftone dots inside `r`, only where `inside(point)` is true (for circles, chart areas, shapes).
pub fn halftone_where(f: &mut Frame, r: Rectangle, color: Color, spacing: f32, density: f32, inside: impl Fn(Point) -> bool) {
    let rad = spacing * 0.5 * density.clamp(0.05, 1.0);
    let dots = Path::new(|b| {
        let mut y = r.y + spacing / 2.0;
        let mut row = 0;
        while y < r.y + r.height {
            let mut x = r.x + spacing / 2.0 + if row % 2 == 1 { spacing / 2.0 } else { 0.0 };
            while x < r.x + r.width {
                let p = Point::new(x, y);
                if inside(p) {
                    b.circle(p, rad);
                }
                x += spacing;
            }
            y += spacing;
            row += 1;
        }
    });
    f.fill(&dots, color);
}

/// 45° hatch lines inside `r`, clipped by math (canvas clip regions misplace content
/// in this renderer, so nothing in Ember uses `with_clip`).
pub fn hatch(f: &mut Frame, r: Rectangle, gap: f32, width: f32, color: Color) {
    let lines = Path::new(|b| {
        // line k: points (r.x + d - t, r.y + t), t in [0, r.height]; keep the part with x inside r
        let mut d = gap / 2.0;
        while d < r.width + r.height {
            let t0 = (d - r.width).max(0.0);
            let t1 = d.min(r.height);
            if t1 > t0 {
                b.move_to(Point::new(r.x + d - t0, r.y + t0));
                b.line_to(Point::new(r.x + d - t1, r.y + t1));
            }
            d += gap;
        }
    });
    f.stroke(&lines, Stroke::default().with_color(color).with_width(width));
}

/// Radial speed lines between r_in and r_out; `pose` rotates them in 8 steps.
pub fn speed_lines(f: &mut Frame, c: Point, r_in: f32, r_out: f32, n: usize, pose: u64) {
    let rot = (pose % 8) as f32 * (TAU / n as f32) / 8.0;
    let rays = Path::new(|b| {
        for i in 0..n {
            let a = rot + i as f32 * TAU / n as f32;
            let w = 0.025;
            b.move_to(c + Vector::new((a - w).cos(), (a - w).sin()) * r_in);
            b.line_to(c + Vector::new(a.cos(), a.sin()) * r_out);
            b.line_to(c + Vector::new((a + w).cos(), (a + w).sin()) * r_in);
            b.close();
        }
    });
    f.fill(&rays, pal().scene_line);
}

/// 20-point SFX burst (shape from comic-verse `.burst`) with the word inside.
/// `age` = poses since the burst started: sfx-pop over 4 poses, then holds.
pub fn burst(f: &mut Frame, c: Point, r: f32, word: &str, age: u64) {
    const PTS: [(f32, f32); 20] = [
        (50., 0.), (61., 18.), (82., 6.), (78., 29.), (100., 30.), (84., 47.), (98., 64.), (76., 66.), (80., 90.), (60., 78.),
        (50., 100.), (40., 78.), (20., 90.), (24., 66.), (2., 64.), (16., 47.), (0., 30.), (22., 29.), (18., 6.), (39., 18.),
    ];
    let (scale, rot): (f32, f32) = match age {
        0 => (0.2, -20.0),
        1 => (1.35, 6.0),
        2 => (0.92, -12.0),
        _ => (1.0, -10.0),
    };
    let shape = |off: f32| poly(&PTS.map(|(x, y)| Point::new(c.x + (x - 50.) / 50. * r + off, c.y + (y - 50.) / 50. * r + off)));
    f.with_save(|f| {
        f.translate(Vector::new(c.x, c.y));
        f.scale(scale);
        f.translate(Vector::new(-c.x, -c.y));
        f.fill(&shape(r * 0.08), INK);
        f.fill(&shape(0.0), YELLOW);
        f.stroke(&shape(0.0), stroke(3.0));
        f.translate(Vector::new(c.x, c.y));
        f.rotate(Radians(rot.to_radians()));
        f.translate(Vector::new(-c.x, -c.y));
        let size = r * 0.55;
        let at = Point::new(c.x, c.y - size * 0.5);
        if size >= 48.0 {
            misprint(f, word, at, size, RED, Alignment::Center, 0);
        } else {
            text(f, word, at + Vector::new(2.0, 2.0), size, DISPLAY, INK, Alignment::Center, Vertical::Top);
            text(f, word, at, size, DISPLAY, RED, Alignment::Center, Vertical::Top);
        }
    });
}

/// White speech bubble with tail pointing at `tail`; Permanent Marker text.
pub fn bubble(f: &mut Frame, r: Rectangle, tail: Point, s: &str, size: f32) {
    let c = r.center();
    let ell = Path::new(|b| {
        b.ellipse(cosmic::iced::widget::canvas::path::arc::Elliptical {
            center: c,
            radii: Vector::new(r.width / 2.0, r.height / 2.0),
            rotation: Radians(0.0),
            start_angle: Radians(0.0),
            end_angle: Radians(TAU),
        })
    });
    // tail base sits on the ellipse edge facing `tail`; tip is at most ~45px out
    let dir = tail - c;
    let ang = dir.y.atan2(dir.x);
    let edge = |a: f32| Point::new(c.x + a.cos() * r.width * 0.42, c.y + a.sin() * r.height * 0.42);
    let (base_l, base_r) = (edge(ang - 0.22), edge(ang + 0.22));
    let rim = edge(ang);
    let len = (dir.x.hypot(dir.y) - r.width.min(r.height) * 0.45).clamp(18.0, 45.0);
    let tip = rim + Vector::new(ang.cos(), ang.sin()) * len;
    let tail_path = poly(&[base_l, tip, base_r]);
    f.fill(&tail_path, WHITE);
    f.stroke(&tail_path, stroke(3.0));
    f.fill(&ell, WHITE);
    f.stroke(&ell, stroke(3.0));
    // cover the seam so bubble + tail read as one shape
    f.fill(&poly(&[edge(ang - 0.18), rim + Vector::new(ang.cos(), ang.sin()) * 6.0, edge(ang + 0.18), c]), WHITE);
    f.fill_text(Text {
        content: s.into(),
        position: c,
        max_width: r.width * 0.8,
        size: size.into(),
        font: MARKER,
        color: INK,
        align_x: Alignment::Center,
        align_y: Vertical::Center,
        ..Text::default()
    });
}

/// Human byte sizes: 7.7 GB, 512 MB.
pub fn bytes(b: f64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = b;
    let mut i = 0;
    while v >= 1000.0 && i < U.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if i == 0 { format!("{v:.0} {}", U[i]) } else { format!("{v:.1} {}", U[i]) }
}

pub fn opt<T>(v: Option<T>, f: impl FnOnce(T) -> String) -> String {
    v.map_or_else(|| "--".to_string(), f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_and_formatting() {
        assert_eq!(Level::from_percent(10.0), Level::Calm);
        assert_eq!(Level::from_percent(50.0), Level::Busy);
        assert_eq!(Level::from_percent(90.0), Level::Hot);
        assert_eq!(Level::from_celsius(85.0), Level::Hot);
        assert_eq!(bytes(7_690_043_392.0), "7.7 GB");
        assert_eq!(bytes(999.0), "999 B");
        assert_eq!(opt(None::<f32>, |v| v.to_string()), "--");
    }
}
