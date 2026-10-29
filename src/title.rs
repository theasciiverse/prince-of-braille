//! The title, the story screens and the end, as the DOS version shows them
//! (SDLPoP show_title and end_sequence; PrinceJS Title, Credits and
//! EndTitle, whose pictures these are).
//!
//! A game starts at the title: the palace, "Broderbund presents", "a game
//! by Jordan Mechner", the name; then "In the Sultan's absence", the
//! vizier in the princess's room, "Marry Jaffar", the name again, the
//! credits and the demo, round and round until a key is pressed. Winning
//! ends with "The tyrant Jaffar" and the epilogue, then the title.
//!
//! Hold times are SDLPoP's (in 60ths of a second); where it waits for a
//! song to end, the song's length.

use crate::dos::{Pic, S};
use crate::game::FPS;
use crate::music::Song;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// A DOS screen's lines: the title pictures are whole screens.
pub const LINES: i32 = 200;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Part {
    /// the title up to the story, before the princess's room
    Opening,
    /// after the princess's room, before the demo
    Story,
    /// after the embrace, before the title
    Epilogue,
}

/// A picture on the screen.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Shot {
    Background,
    Presents,
    Author,
    /// the name and the copyright over the palace
    Name,
    Absence,
    Marry,
    Credits,
    Tyrant,
}

#[derive(Clone, Copy, Debug)]
enum Look {
    Show(Shot),
    FadeIn(Shot),
    FadeOut(Shot),
    /// SDLPoP transition_ltr: the new picture wipes in from the left
    Wipe(Shot, Shot),
}

struct Stage {
    frames: u32,
    look: Look,
    /// a song that starts with the stage
    song: Option<Song>,
}

/// SDLPoP's 60ths of a second, in frames
const fn t60(n: u32) -> u32 {
    n * FPS / 60
}

/// a song's length in frames, from its tenths of a second
const fn song(tenths: u32) -> u32 {
    tenths * FPS / 10
}

const FADE: u32 = FPS;
const WIPE: u32 = FPS / 2;

fn stage(frames: u32, look: Look) -> Stage {
    Stage { frames, look, song: None }
}

fn stages(part: Part) -> Vec<Stage> {
    use Look::*;
    use Shot::*;
    match part {
        Part::Opening => {
            let before_name = t60(130 + 205 + 65 + 270 + 235);
            vec![
                stage(2 * FADE, FadeIn(Background)),
                Stage { frames: t60(130), look: Show(Background), song: Some(Song::PrologueA) },
                stage(t60(205), Show(Presents)),
                stage(t60(65), Show(Background)),
                stage(t60(270), Show(Author)),
                stage(t60(235), Show(Background)),
                // until the main theme ends (25.2 s)
                stage(song(252) - before_name, Show(Name)),
                Stage { frames: WIPE, look: Wipe(Name, Absence), song: Some(Song::PrologueB) },
                stage(t60(600), Show(Absence)),
                stage(FADE, FadeOut(Absence)),
            ]
        }
        Part::Story => vec![
            stage(FADE, FadeIn(Marry)),
            // SDLPoP holds it while the princess's song ends
            stage(6 * FPS, Show(Marry)),
            stage(WIPE, Wipe(Marry, Name)),
            stage(t60(120), Show(Name)),
            stage(WIPE, Wipe(Name, Credits)),
            stage(t60(360), Show(Credits)),
            stage(2 * FADE, FadeOut(Credits)),
        ],
        Part::Epilogue => {
            let before = FADE + t60(900) + WIPE;
            vec![
                Stage { frames: FADE, look: FadeIn(Tyrant), song: Some(Song::Epilogue) },
                stage(t60(900), Show(Tyrant)),
                stage(WIPE, Wipe(Tyrant, Background)),
                // until the epilogue ends (117.6 s)
                stage(song(1176) - before, Show(Background)),
                stage(2 * FADE, FadeOut(Background)),
            ]
        }
    }
}

pub struct Title {
    pub part: Part,
    stages: Vec<Stage>,
    stage: usize,
    /// frames into the stage
    t: u32,
}

impl Title {
    pub fn new(part: Part) -> Title {
        log::info!("title {part:?}");
        Title { part, stages: stages(part), stage: 0, t: 0 }
    }

    /// One frame; the song that starts now, if one does.
    pub fn tick(&mut self) -> Option<Song> {
        let st = self.stages.get(self.stage)?;
        let song = if self.t == 0 { st.song } else { None };
        self.t += 1;
        if self.t >= st.frames {
            self.stage += 1;
            self.t = 0;
        }
        song
    }

    pub fn done(&self) -> bool {
        self.stage >= self.stages.len()
    }

    /// This frame's picture, LINES tall.
    pub fn picture(&self) -> Pic {
        let Some(st) = self.stages.get(self.stage) else { return Pic::tall(LINES) };
        let f = self.t as f32 / st.frames.max(1) as f32;
        match st.look {
            Look::Show(s) => shot(s),
            Look::FadeIn(s) => dim(shot(s), f),
            Look::FadeOut(s) => dim(shot(s), 1.0 - f),
            Look::Wipe(a, b) => {
                let (mut old, new) = (shot(a), shot(b));
                let edge = ((f * crate::dos::W as f32) as i32).max(1);
                for y in 0..LINES {
                    for x in 0..edge {
                        let i = (y * crate::dos::W + x) as usize;
                        old.col[i] = new.col[i];
                    }
                }
                old
            }
        }
    }
}

/// The text screens' words, for windows too small to read them in the
/// DOS lettering.
const ABSENCE: &str = "In the Sultan's absence the Grand Vizier JAFFAR rules with the iron fist of tyranny. \
Only one obstacle remains between Jaffar and the throne: the Sultan's beautiful young daughter...";
const MARRY: &str = "Marry Jaffar... or die within the hour. All the Princess's hopes now rest on the brave \
youth she loves. Little does she know that he is already a prisoner in Jaffar's dungeons...";
const TYRANT: &str = "The tyrant Jaffar lies dead, his power shattered. Throughout the land the people of \
Persia hail their Princess... and the brave youth who saved her from the forces of darkness. No longer a \
stranger, he shall from this day forth be known as... PRINCE_OF_PERSIA.";
const CREDITS: [(&str, &str); 9] = [
    ("Programmed by", "Lance Groody"),
    ("Graphics by", "Leila Bronstein"),
    ("", "Avril Harrison"),
    ("", "James St. Louis"),
    ("Sound design by", "Tom Rettig"),
    ("Music composed by", "Francis Mechner"),
    ("Produced by", "Brian Eheler"),
    ("Directed by", "Jordan Mechner"),
    ("", ""),
];

/// Below this many rows the DOS lettering is too small to read.
const READABLE_ROWS: u16 = 50;
/// The text panel inside the frame, in DOS pixels: x0, y0, x1, y1.
const PANEL: (i32, i32, i32, i32) = (16, 16, 304, 171);

impl Title {
    /// Write the words of the text screen on the screen as terminal text
    /// (over `r`, where the picture is), when the window is too small to
    /// read the DOS lettering.
    pub fn paint_words(&self, buf: &mut Buffer, r: Rect) {
        if r.height >= READABLE_ROWS {
            return;
        }
        let Some(st) = self.stages.get(self.stage) else { return };
        let f = self.t as f32 / st.frames.max(1) as f32;
        // (shot, its columns as fractions of the width, brightness)
        let layers = match st.look {
            Look::Show(s) => vec![(s, 0.0, 1.0, 1.0)],
            Look::FadeIn(s) => vec![(s, 0.0, 1.0, f)],
            Look::FadeOut(s) => vec![(s, 0.0, 1.0, 1.0 - f)],
            Look::Wipe(a, b) => vec![(a, f, 1.0, 1.0), (b, 0.0, f, 1.0)],
        };
        let cx = |x: i32, up: bool| {
            let v = x as f32 * r.width as f32 / crate::dos::W as f32;
            r.x + if up { v.ceil() } else { v.floor() } as u16
        };
        let cy = |y: i32, up: bool| {
            let v = y as f32 * r.height as f32 / LINES as f32;
            r.y + if up { v.ceil() } else { v.floor() } as u16
        };
        let (x0, y0, x1, y1) = (cx(PANEL.0, true), cy(PANEL.1, true), cx(PANEL.2, false), cy(PANEL.3, false));
        if x1 <= x0 + 4 || y1 <= y0 + 2 {
            return;
        }
        for (s, from, to, light) in layers {
            // about as many letters a line as the screens have
            let width = ((x1 - x0 - 2) as usize).min(46);
            let Some(lines) = words(s, width, (y1 - y0) as usize) else { continue };
            let inside = |x: u16| {
                let fx = (x - r.x) as f32 / r.width as f32;
                fx >= from && fx < to
            };
            let scale = |c: [u8; 3]| Color::Rgb((c[0] as f32 * light) as u8, (c[1] as f32 * light) as u8, (c[2] as f32 * light) as u8);
            let back = scale(panel_colour(&shot(s)));
            for y in y0..y1 {
                for x in (x0..x1).filter(|x| inside(*x)) {
                    if let Some(c) = buf.cell_mut((x, y)) {
                        c.set_char(' ').set_style(Style::default().bg(back));
                    }
                }
            }
            let style = Style::default().fg(scale([255, 255, 255])).bg(back).add_modifier(Modifier::BOLD);
            let top = y0 + (y1 - y0).saturating_sub(lines.len() as u16) / 2;
            for (i, line) in lines.iter().enumerate() {
                let y = top + i as u16;
                if y >= y1 {
                    break;
                }
                let w = line.chars().count() as u16;
                let mut x = x0 + (x1 - x0).saturating_sub(w) / 2;
                for ch in line.chars() {
                    if inside(x) {
                        if let Some(c) = buf.cell_mut((x, y)) {
                            c.set_char(ch).set_style(style);
                        }
                    }
                    x += 1;
                }
            }
        }
    }
}

/// The text screen's lines for a panel `width` cells wide and `rows` tall.
fn words(s: Shot, width: usize, rows: usize) -> Option<Vec<String>> {
    let text = match s {
        Shot::Absence => ABSENCE,
        Shot::Marry => MARRY,
        Shot::Tyrant => TYRANT,
        Shot::Credits => {
            // the roles right-aligned against the names, as on the screen
            let rw = CREDITS.iter().map(|c| c.0.len()).max().unwrap_or(0);
            let nw = CREDITS.iter().map(|c| c.1.len()).max().unwrap_or(0);
            let lines = CREDITS
                .iter()
                .filter(|c| !c.1.is_empty())
                .map(|(r, n)| format!("{r:>rw$}   {n:<nw$}"))
                .collect();
            return Some(lines);
        }
        _ => return None,
    };
    let mut lines = vec![String::new()];
    for w in text.split_whitespace() {
        let last = lines.last_mut().unwrap();
        if !last.is_empty() && last.chars().count() + 1 + w.chars().count() > width {
            lines.push(w.to_string());
        } else {
            if !last.is_empty() {
                last.push(' ');
            }
            last.push_str(w);
        }
    }
    // the name stays on one line
    let mut lines: Vec<String> = lines.into_iter().map(|l| l.replace('_', " ")).collect();
    // spaced out when there is room, as the screens space them
    if 2 * lines.len() - 1 <= rows {
        let n = lines.len();
        lines = lines.into_iter().enumerate().flat_map(|(i, l)| if i + 1 < n { vec![l, String::new()] } else { vec![l] }).collect();
    }
    Some(lines)
}

/// The panel's colour: the dark of its dither, without the letters.
fn panel_colour(pic: &Pic) -> [u8; 3] {
    let (mut acc, mut n) = ([0u32; 3], 0u32);
    for y in PANEL.1..PANEL.3 {
        for x in PANEL.0..PANEL.2 {
            let c = pic.col[(y * crate::dos::W + x) as usize];
            if c.iter().map(|v| *v as u32).sum::<u32>() < 300 {
                for k in 0..3 {
                    acc[k] += c[k] as u32;
                }
                n += 1;
            }
        }
    }
    let n = n.max(1);
    [(acc[0] / n) as u8, (acc[1] / n) as u8, (acc[2] / n) as u8]
}

fn shot(s: Shot) -> Pic {
    let mut p = Pic::tall(LINES);
    let back = |p: &mut Pic| p.named(S::Title, "main_background", 0, 0);
    match s {
        Shot::Background => back(&mut p),
        // where SDLPoP puts them
        Shot::Presents => {
            back(&mut p);
            p.named(S::Title, "presents", 96, 106);
        }
        Shot::Author => {
            back(&mut p);
            p.named(S::Title, "author", 96, 122);
        }
        Shot::Name => p.named(S::Title, "prince", 0, 0),
        Shot::Absence => p.named(S::Title, "in_the_absence", 0, 0),
        Shot::Marry => p.named(S::Title, "marry_jaffar", 0, 0),
        Shot::Credits => p.named(S::Title, "credits", 0, 0),
        Shot::Tyrant => p.named(S::Title, "the_tyrant", 0, 0),
    }
    p
}

fn dim(mut p: Pic, f: f32) -> Pic {
    let f = f.clamp(0.0, 1.0);
    for c in p.col.iter_mut() {
        *c = c.map(|v| (v as f32 * f) as u8);
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shot_has_its_pictures() {
        for s in ["main_background", "presents", "author", "prince", "in_the_absence", "marry_jaffar", "credits", "the_tyrant"] {
            assert!(crate::dos::img(S::Title, s).is_some(), "{s}");
        }
    }

    #[test]
    fn the_name_shows_until_the_main_theme_ends() {
        let mut t = Title::new(Part::Opening);
        let mut songs = Vec::new();
        let mut frames = 0;
        while !t.done() {
            if let Some(s) = t.tick() {
                songs.push((frames, s));
            }
            frames += 1;
        }
        assert_eq!(songs.len(), 2);
        // the second theme starts as the first ends, 25.2 s after it began
        assert_eq!(songs[1].0 - songs[0].0, song(252));
    }
}
