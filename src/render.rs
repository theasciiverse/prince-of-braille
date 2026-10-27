//! Terminal rendering: the 280 x 192 Apple screen onto character cells.
//!
//! Every cell covers 2 x 4 "dots" (one Apple pixel each at full size, a
//! 2 x 2 pixel block at half size). How a cell is drawn depends on what's
//! in it:
//!
//! * the prince is always Braille: 8 dots a cell is what keeps Mechner's
//!   rotoscoped animation fluid;
//! * the dungeon picks, per cell, the block-element glyph (full, half,
//!   quadrant, edge) that best matches its dots, for solid stone with real
//!   definition, and only falls back to Braille where no solid glyph fits
//!   (thin details like chains, flames and bubbles).

use crate::apple::{Ink, Screen, SCREEN_H, SCREEN_W};
use crate::game::{Game, Phase};
use crate::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Glyphs {
    /// blocks for the dungeon, Braille for the prince and fine detail
    Hybrid,
    /// everything in Braille
    Braille,
    /// everything in the nearest block glyph
    Blocks,
}

impl Glyphs {
    pub fn next(self) -> Glyphs {
        match self {
            Glyphs::Hybrid => Glyphs::Braille,
            Glyphs::Braille => Glyphs::Blocks,
            Glyphs::Blocks => Glyphs::Hybrid,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Glyphs::Hybrid => "hybrid",
            Glyphs::Braille => "braille",
            Glyphs::Blocks => "blocks",
        }
    }
}

/// Dot (col, row) in a 2 x 4 cell -> bit in our cell mask.
#[inline]
fn bit(c: usize, r: usize) -> u8 {
    1 << (r * 2 + c)
}

const fn m(rows: [u8; 4]) -> u8 {
    // rows: 0b(c0 c1) per row, c0 = left (bit 1 of the pair)
    let mut v = 0;
    let mut r = 0;
    while r < 4 {
        if rows[r] & 0b10 != 0 {
            v |= 1 << (r * 2);
        }
        if rows[r] & 0b01 != 0 {
            v |= 1 << (r * 2 + 1);
        }
        r += 1;
    }
    v
}

/// Block-element glyphs with the dots they cover.
const BLOCKS: &[(char, u8)] = &[
    (' ', 0),
    ('█', 0xff),
    ('▀', m([0b11, 0b11, 0, 0])),
    ('▄', m([0, 0, 0b11, 0b11])),
    ('▌', m([0b10, 0b10, 0b10, 0b10])),
    ('▐', m([0b01, 0b01, 0b01, 0b01])),
    ('▘', m([0b10, 0b10, 0, 0])),
    ('▝', m([0b01, 0b01, 0, 0])),
    ('▖', m([0, 0, 0b10, 0b10])),
    ('▗', m([0, 0, 0b01, 0b01])),
    ('▚', m([0b10, 0b10, 0b01, 0b01])),
    ('▞', m([0b01, 0b01, 0b10, 0b10])),
    ('▙', m([0b10, 0b10, 0b11, 0b11])),
    ('▛', m([0b11, 0b11, 0b10, 0b10])),
    ('▜', m([0b11, 0b11, 0b01, 0b01])),
    ('▟', m([0b01, 0b01, 0b11, 0b11])),
    ('▂', m([0, 0, 0, 0b11])),
    ('▆', m([0, 0b11, 0b11, 0b11])),
    ('▔', m([0b11, 0, 0, 0])),
];

fn braille(mask: u8) -> char {
    // our bit layout -> Unicode Braille dot numbering
    const MAP: [u8; 8] = [0x01, 0x08, 0x02, 0x10, 0x04, 0x20, 0x40, 0x80];
    let mut v = 0u32;
    for (i, b) in MAP.iter().enumerate() {
        if mask & (1 << i) != 0 {
            v |= *b as u32;
        }
    }
    char::from_u32(0x2800 + v).unwrap_or(' ')
}

fn nearest_block(mask: u8) -> (char, u32) {
    let mut best = (' ', u32::MAX);
    for &(c, bm) in BLOCKS {
        let e = (mask ^ bm).count_ones();
        if e < best.1 {
            best = (c, e);
        }
    }
    best
}

fn lum(c: [u8; 3]) -> u32 {
    (c[0] as u32 * 30 + c[1] as u32 * 59 + c[2] as u32 * 11) / 100
}

fn dist(a: [u8; 3], b: [u8; 3]) -> u32 {
    (0..3).map(|k| (a[k] as i32 - b[k] as i32).pow(2) as u32).sum()
}

fn mean(cs: impl Iterator<Item = [u8; 3]>) -> [u8; 3] {
    let (mut acc, mut n) = ([0u32; 3], 0u32);
    for c in cs {
        for k in 0..3 {
            acc[k] += c[k] as u32;
        }
        n += 1;
    }
    if n == 0 {
        return [0; 3];
    }
    [(acc[0] / n) as u8, (acc[1] / n) as u8, (acc[2] / n) as u8]
}

fn color(c: [u8; 3]) -> Color {
    Color::Rgb(c[0], c[1], c[2])
}

const KID: Color = Color::Rgb(255, 255, 255);

/// The prince's colour in a cell: his pixels' own colour, kept bright so
/// the Braille dots stand out (white robe, orange turban, blue fringes).
fn kid_color(c: [u8; 3]) -> Color {
    let m = c.iter().copied().max().unwrap_or(0).max(1) as u32;
    let k = |v: u8| ((v as u32 * 255 / m + 255) / 2).min(255) as u8;
    if m < 60 {
        return KID;
    }
    Color::Rgb(k(c[0]), k(c[1]), k(c[2]))
}
/// Below this brightness a dot counts as dark.
const DARK: u32 = 28;

pub fn cells_needed(scale: i32) -> (u16, u16) {
    ((SCREEN_W / 2 / scale) as u16, (SCREEN_H / 4 / scale) as u16)
}

/// One dot: is the prince there, and what colour is the dungeon there.
fn dot(scr: &Screen, col: &[[u8; 3]], x: i32, y: i32, scale: i32) -> (bool, [u8; 3]) {
    let mut kid = false;
    let mut cs = Vec::with_capacity(4);
    for dy in 0..scale {
        for dx in 0..scale {
            let (px, py) = (x * scale + dx, y * scale + dy);
            if !(0..SCREEN_W).contains(&px) || !(0..SCREEN_H).contains(&py) {
                continue;
            }
            let i = (py * SCREEN_W + px) as usize;
            kid |= matches!(scr.px[i], Ink::Kid | Ink::Guard);
            cs.push(col[i]);
        }
    }
    (kid, mean(cs.into_iter()))
}

/// Paint the Apple screen into `area` at `scale`.
pub fn paint_screen(buf: &mut Buffer, area: Rect, scr: &Screen, scale: i32, glyphs: Glyphs, theme: Theme, palace: bool) {
    let mut col = scr.colors();
    if theme != Theme::Classic {
        let w = SCREEN_W as usize;
        for (i, c) in col.iter_mut().enumerate() {
            // stipple gaps carry no ink of their own: borrow a neighbour's
            let mut ink = scr.px[i];
            if ink == Ink::None {
                let x = i % w;
                if x > 0 && scr.px[i - 1] != Ink::None {
                    ink = scr.px[i - 1];
                } else if x + 1 < w && scr.px[i + 1] != Ink::None {
                    ink = scr.px[i + 1];
                }
            }
            *c = theme.apply(palace, ink, *c);
        }
    }
    let (cw, ch) = cells_needed(scale);
    for cy in 0..ch.min(area.height) {
        for cx in 0..cw.min(area.width) {
            let mut kid = 0u8;
            let mut dots = [[0u8; 3]; 8];
            let mut kid_cols = Vec::new();
            for r in 0..4 {
                for c in 0..2 {
                    let (k, a) = dot(scr, &col, cx as i32 * 2 + c as i32, cy as i32 * 4 + r as i32, scale);
                    if k {
                        kid |= bit(c, r);
                        kid_cols.push(a);
                    } else {
                        dots[r * 2 + c] = a;
                    }
                }
            }
            let (mut ch_, mut style) = cell(kid, &dots, glyphs);
            if kid != 0 {
                let m = mean(kid_cols.into_iter());
                style = style.fg(if theme == Theme::Classic { kid_color(m) } else { color(m) });
            }
            if ch_ == '\0' {
                ch_ = ' ';
            }
            if let Some(cell) = buf.cell_mut((area.x + cx, area.y + cy)) {
                cell.set_char(ch_).set_style(style);
            }
        }
    }
}

/// Split a cell's dots into two colour groups (2-means, seeded with the
/// brightest and darkest dot). Returns (fg, bg, mask of fg dots).
fn two_colors(dots: &[[u8; 3]; 8]) -> ([u8; 3], [u8; 3], u8) {
    let mut hi = *dots.iter().max_by_key(|c| lum(**c)).unwrap();
    let mut lo = *dots.iter().min_by_key(|c| lum(**c)).unwrap();
    if dist(hi, lo) < 40 * 40 {
        let m = mean(dots.iter().copied());
        return (m, m, 0xff);
    }
    let mut mask = 0u8;
    for _ in 0..3 {
        mask = 0;
        for (i, d) in dots.iter().enumerate() {
            if dist(*d, hi) < dist(*d, lo) {
                mask |= 1 << i;
            }
        }
        hi = mean(dots.iter().enumerate().filter(|(i, _)| mask & (1 << i) != 0).map(|(_, c)| *c));
        lo = mean(dots.iter().enumerate().filter(|(i, _)| mask & (1 << i) == 0).map(|(_, c)| *c));
    }
    (hi, lo, mask)
}

fn cell(kid: u8, dots: &[[u8; 3]; 8], glyphs: Glyphs) -> (char, Style) {
    let (fg, bg, mask) = two_colors(dots);
    if kid != 0 {
        // the prince: Braille dots over a dimmed hint of what's behind him
        let mut st = Style::default().fg(KID).bg(Color::Black).add_modifier(Modifier::BOLD);
        let behind = mean(dots.iter().copied());
        if glyphs != Glyphs::Braille && lum(behind) > DARK {
            st = st.bg(color([behind[0] / 3, behind[1] / 3, behind[2] / 3]));
        }
        let env = if lum(fg) > DARK { mask } else { 0 };
        return match glyphs {
            Glyphs::Blocks => (nearest_block(kid).0, st),
            Glyphs::Braille => (braille(kid | env), st),
            Glyphs::Hybrid => (braille(kid), st),
        };
    }
    let black = Style::default().bg(Color::Black);
    if lum(fg) <= DARK {
        return (' ', black);
    }
    // one colour throughout: a solid cell
    let uniform = dist(fg, bg) < 40 * 40;
    if uniform {
        let c = mean(dots.iter().copied());
        return match glyphs {
            Glyphs::Braille => (braille(0xff), black.fg(color(c))),
            _ => ('█', black.fg(color(c))),
        };
    }
    let bgc = if lum(bg) <= DARK { Color::Black } else { color(bg) };
    let st = Style::default().fg(color(fg)).bg(bgc);
    match glyphs {
        Glyphs::Braille => (braille(mask), st),
        Glyphs::Blocks => (nearest_block(mask).0, st),
        Glyphs::Hybrid => {
            let (c, err) = nearest_block(mask);
            // sparse detail keeps its exact dots; solid areas get blocks
            if mask.count_ones() <= 2 || err > 1 {
                (braille(mask), st)
            } else {
                (c, st)
            }
        }
    }
}

/// Bottom line: strength, level, time, and transient messages.
pub fn paint_status(buf: &mut Buffer, area: Rect, g: &Game, note: &str) {
    let y = area.y;
    let red = Style::default().fg(Color::Rgb(230, 60, 50)).bg(Color::Black);
    let dimr = Style::default().fg(Color::Rgb(110, 40, 35)).bg(Color::Black);
    let text = Style::default().fg(Color::Rgb(200, 200, 200)).bg(Color::Black);
    for x in area.x..area.x + area.width {
        if let Some(c) = buf.cell_mut((x, y)) {
            c.set_char(' ').set_style(text);
        }
    }
    let mut x = area.x;
    for i in 0..g.max_strength {
        let st = if i < g.kid_strength { red } else { dimr };
        if let Some(c) = buf.cell_mut((x, y)) {
            c.set_char(if i < g.kid_strength { '▲' } else { '△' }).set_style(st);
        }
        x += 1;
    }
    let msg = match g.phase {
        Phase::Dead(_) if g.dead_frames > 24 => Some("Press SPACE to continue".to_string()),
        Phase::TimeUp => Some("TIME HAS EXPIRED  (R restarts, Esc quits)".to_string()),
        Phase::Won => Some("YOU WIN".to_string()),
        _ => g.message.as_ref().map(|(s, _)| s.clone()),
    };
    if let Some(m) = msg {
        let w = m.chars().count() as u16;
        let sx = area.x + area.width.saturating_sub(w) / 2;
        buf.set_string(sx, y, &m, text.add_modifier(Modifier::BOLD));
    }
    // the opponent's meter at the right (GAMEBG.S DRAWOPPMETER): not for
    // the mouse or the skeleton, and the shadow's only on level 12
    let o = &g.shad;
    let opp = g.opp_strength > 0 && o.id != 24 && o.id != 4 && (o.id != 1 || g.level.number == 12);
    let mut rx = area.x + area.width;
    if opp {
        let blue = Style::default().fg(Color::Rgb(90, 150, 240)).bg(Color::Black);
        for _ in 0..g.opp_strength {
            rx = rx.saturating_sub(1);
            if let Some(c) = buf.cell_mut((rx, y)) {
                c.set_char('▲').set_style(blue);
            }
        }
        rx = rx.saturating_sub(2);
    }
    let right = format!("L{}  {} min  {}", g.level.number, g.minutes_left(), note);
    let w = right.chars().count() as u16;
    if rx > area.x + w {
        buf.set_string(rx - w, y, &right, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braille_bits_map_to_unicode_dots() {
        assert_eq!(braille(0), '\u{2800}');
        assert_eq!(braille(0xff), '\u{28ff}');
        // top-left dot is dot 1, bottom-right is dot 8
        assert_eq!(braille(bit(0, 0)), '\u{2801}');
        assert_eq!(braille(bit(1, 3)), '\u{2880}');
    }

    #[test]
    fn solid_and_half_cells_get_blocks() {
        assert_eq!(nearest_block(0xff), ('█', 0));
        assert_eq!(nearest_block(m([0b10, 0b10, 0b10, 0b10])).0, '▌');
        assert_eq!(nearest_block(m([0, 0, 0b11, 0b11])).0, '▄');
    }

    const BLUE: [u8; 3] = [20, 207, 253];
    const WHITE: [u8; 3] = [255, 255, 255];

    #[test]
    fn hybrid_keeps_thin_detail_in_braille() {
        let mut dots = [[0u8; 3]; 8];
        dots[0] = BLUE;
        let (c, _) = cell(0, &dots, Glyphs::Hybrid);
        assert_eq!(c, braille(1));
        let (c, _) = cell(0, &[BLUE; 8], Glyphs::Hybrid);
        assert_eq!(c, '█');
        // the kid is Braille even over solid wall
        let (c, _) = cell(0xff, &[BLUE; 8], Glyphs::Hybrid);
        assert_eq!(c, '\u{28ff}');
    }

    #[test]
    fn two_colour_cells_use_fg_and_bg() {
        let mut dots = [BLUE; 8];
        dots[0] = [0; 3];
        dots[1] = [0; 3];
        let (c, st) = cell(0, &dots, Glyphs::Hybrid);
        assert_eq!(c, '▆');
        assert_eq!(st.fg, Some(color(BLUE)));
        let mut dots = [BLUE; 8];
        dots[6] = WHITE;
        dots[7] = WHITE;
        let (fg, bg, mask) = two_colors(&dots);
        assert_eq!((fg, bg), (WHITE, BLUE));
        assert_eq!(nearest_block(mask).0, '▂');
    }
}
