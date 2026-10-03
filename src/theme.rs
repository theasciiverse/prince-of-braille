//! Colour themes.
//!
//! The NTSC model in `apple.rs` gives every pixel the colour an Apple II
//! monitor showed. A theme keeps that picture's *shading* (how bright each
//! pixel is, which is where Mechner's brick faces, mortar and highlights
//! live) and its one accent cue (the warm orange pixels), and repaints the
//! hues by what the pixel belongs to: stone, floor, gate, flame, potion,
//! sword or the prince. `Classic` passes the Apple colours straight through.

use crate::apple::Ink;

type Rgb = [u8; 3];

/// Colour ramp: black at 0, then three stops at 1/3, 2/3 and full brightness.
#[derive(Clone, Copy)]
struct Ramp([Rgb; 3]);

impl Ramp {
    fn at(&self, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0) * 3.0;
        let stops = [[0, 0, 0], self.0[0], self.0[1], self.0[2]];
        let i = (t.floor() as usize).min(2);
        let f = t - i as f32;
        let (a, b) = (stops[i], stops[i + 1]);
        let mix = |k: usize| (a[k] as f32 + (b[k] as f32 - a[k] as f32) * f).round() as u8;
        [mix(0), mix(1), mix(2)]
    }
}

/// One environment set (dungeon or palace).
#[derive(Clone, Copy)]
struct Set {
    wall: Ramp,
    floor: Ramp,
    /// what the Apple's accent pixels (mortar, cracks) become
    accent: Ramp,
    gate: Ramp,
    /// accents are the warm pixels (dungeon: blue stone, orange detail)
    /// or the cool ones (palace: orange stone, blue detail)
    warm_accent: bool,
}

#[derive(Clone, Copy)]
struct Common {
    flame: Ramp,
    potion: Ramp,
    sword: Ramp,
    /// the prince's robe and skin
    kid: Ramp,
    /// the prince's warm details (turban, sash)
    kid_accent: Ramp,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    /// slate stone, amber torchlight, warm palace sandstone (default)
    Dusk,
    /// green phosphor with amber highlights: the Asciiverse terminal look
    Phosphor,
    /// warm amber monochrome
    Amber,
    /// the original Apple II colours through the NTSC model
    Classic,
}

pub const ALL: [Theme; 4] = [Theme::Dusk, Theme::Phosphor, Theme::Amber, Theme::Classic];

const fn r(a: Rgb, b: Rgb, c: Rgb) -> Ramp {
    Ramp([a, b, c])
}

impl Theme {
    pub fn name(self) -> &'static str {
        match self {
            Theme::Dusk => "dusk",
            Theme::Phosphor => "phosphor",
            Theme::Amber => "amber",
            Theme::Classic => "classic",
        }
    }

    pub fn from_name(s: &str) -> Option<Theme> {
        ALL.into_iter().find(|t| t.name().eq_ignore_ascii_case(s.trim()))
    }

    pub fn next(self) -> Theme {
        let i = ALL.iter().position(|t| *t == self).unwrap_or(0);
        ALL[(i + 1) % ALL.len()]
    }

    fn sets(self) -> Option<(Set, Set, Common)> {
        match self {
            Theme::Classic => None,
            Theme::Dusk => Some((
                Set {
                    wall: r([34, 40, 58], [74, 86, 116], [146, 160, 190]),
                    floor: r([44, 48, 64], [96, 104, 128], [178, 186, 206]),
                    accent: r([60, 34, 22], [138, 82, 46], [204, 134, 78]),
                    gate: r([40, 40, 44], [104, 106, 112], [190, 192, 198]),
                    warm_accent: true,
                },
                Set {
                    wall: r([58, 40, 30], [150, 106, 72], [226, 196, 150]),
                    floor: r([66, 48, 36], [168, 128, 90], [240, 216, 176]),
                    accent: r([20, 52, 56], [42, 112, 116], [92, 176, 172]),
                    gate: r([46, 42, 38], [122, 112, 100], [214, 204, 186]),
                    warm_accent: false,
                },
                Common {
                    flame: r([150, 40, 16], [240, 120, 30], [255, 214, 120]),
                    potion: r([110, 20, 30], [210, 50, 60], [255, 140, 140]),
                    sword: r([60, 70, 80], [160, 190, 210], [235, 248, 255]),
                    kid: r([120, 116, 108], [206, 200, 186], [250, 246, 236]),
                    kid_accent: r([120, 30, 24], [204, 70, 52], [244, 120, 88]),
                },
            )),
            Theme::Phosphor => {
                let g = r([10, 60, 24], [30, 150, 60], [120, 240, 140]);
                let a = r([80, 50, 0], [190, 120, 10], [255, 190, 60]);
                let set = |warm_accent| Set {
                    wall: g,
                    floor: r([14, 80, 32], [44, 180, 76], [150, 255, 170]),
                    accent: r([6, 36, 14], [16, 90, 36], [40, 140, 60]),
                    gate: g,
                    warm_accent,
                };
                Some((
                    set(true),
                    set(false),
                    Common { flame: a, potion: a, sword: r([60, 120, 70], [170, 255, 180], [230, 255, 235]), kid: r([90, 180, 100], [190, 255, 200], [235, 255, 238]), kid_accent: a },
                ))
            }
            Theme::Amber => {
                let a = r([70, 36, 0], [170, 96, 8], [250, 170, 40]);
                let hot = r([110, 56, 0], [230, 140, 20], [255, 220, 120]);
                let set = |warm_accent| Set {
                    wall: a,
                    floor: r([86, 46, 0], [200, 116, 12], [255, 196, 70]),
                    accent: r([40, 20, 0], [100, 54, 4], [150, 84, 10]),
                    gate: a,
                    warm_accent,
                };
                Some((set(true), set(false), Common { flame: hot, potion: hot, sword: hot, kid: r([150, 100, 30], [250, 200, 110], [255, 236, 190]), kid_accent: hot }))
            }
        }
    }

    /// Recolour one pixel. `rgb` is its Apple colour, `ink` what it is.
    pub fn apply(self, palace: bool, ink: Ink, rgb: Rgb) -> Rgb {
        let Some((dungeon, pal, common)) = self.sets() else { return rgb };
        let set = if palace { pal } else { dungeon };
        let (r_, g_, b_) = (rgb[0] as f32, rgb[1] as f32, rgb[2] as f32);
        let lum = (0.30 * r_ + 0.59 * g_ + 0.11 * b_) / 255.0;
        if lum < 0.02 {
            return [0, 0, 0];
        }
        // the Apple's warm (orange/red) pixels against its cool (blue) ones
        let warm = r_ > b_ + 50.0 && r_ > g_ + 20.0;
        let cool = b_ > r_ + 50.0;
        let accent = if set.warm_accent { warm } else { cool };
        let t = (lum * 1.25).min(1.0);
        let ramp = match ink {
            Ink::None | Ink::Wall => {
                if accent {
                    set.accent
                } else {
                    set.wall
                }
            }
            Ink::Floor => {
                if accent {
                    set.accent
                } else {
                    set.floor
                }
            }
            Ink::Gate => set.gate,
            Ink::Flame => common.flame,
            Ink::Potion => common.potion,
            Ink::Sword => common.sword,
            Ink::Kid | Ink::Guard => {
                if warm {
                    common.kid_accent
                } else {
                    common.kid
                }
            }
        };
        ramp.at(t)
    }
}

/// Where the last chosen theme is kept between runs.
fn config_path() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))?;
    Some(base.join("prince-of-braille").join("theme"))
}

pub fn load() -> Option<Theme> {
    let s = std::fs::read_to_string(config_path()?).ok()?;
    Theme::from_name(&s)
}

pub fn save(t: Theme) {
    let Some(p) = config_path() else { return };
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if let Err(e) = std::fs::write(&p, t.name()) {
        log::warn!("could not save theme to {}: {e}", p.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_is_untouched() {
        let c = [20, 207, 253];
        assert_eq!(Theme::Classic.apply(false, Ink::Wall, c), c);
    }

    #[test]
    fn themes_keep_dark_dark_and_brightness_order() {
        for t in ALL {
            assert_eq!(t.apply(false, Ink::Wall, [0, 0, 0]), [0, 0, 0], "{t:?}");
            let lum = |c: [u8; 3]| c[0] as u32 * 30 + c[1] as u32 * 59 + c[2] as u32 * 11;
            let dim = t.apply(false, Ink::Wall, [40, 60, 90]);
            let bright = t.apply(false, Ink::Wall, [200, 220, 250]);
            assert!(lum(dim) < lum(bright), "{t:?}");
        }
    }

    #[test]
    fn names_round_trip() {
        for t in ALL {
            assert_eq!(Theme::from_name(t.name()), Some(t));
        }
        assert_eq!(Theme::Dusk.next().next().next().next(), Theme::Dusk);
    }
}
