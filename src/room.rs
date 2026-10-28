//! Drawing the princess's room (SUBS.S DoFast for the scenes, GAMEBG.S
//! DRAWGLASS, DRAWPOST, PMASK, PSETUPFLAME, TWINKLE, FLOW).
//!
//! The room's picture itself (MASTER.S `pacProom`) is not in the released
//! source. Everything the scenes add to it is drawn from the original
//! images at the original places: the torch flames, the twinkling stars,
//! the hourglass and its sand, the big white post, the masks over the
//! princess's face and hair. The walls, floor, window, torch niches and
//! doorway under them are our own stand-in, laid out around those places.

use crate::apple::{self, Ink, Op, Screen, SCREEN_H, SCREEN_W};
use crate::cut::{Cut, FLOOR_Y};
use crate::game::*;

// GAMEBG.S
const POSTX: i32 = 31;
const POSTY: i32 = 152;
const POSTIMG: u8 = 0x0c;
const STARX: i32 = 2;
const STARY: [i32; 4] = [0x62, 0x65, 0x6d, 0x72];
const STARI: [u8; 4] = [0x2a, 0x2b, 0x2b, 0x2a];
const GLASSX: i32 = 19;
const GLASSY: i32 = 151;
const GLASSIMG: [u8; 9] = [0x15, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14];
const SANDHT: [i32; 8] = [0, 1, 2, 3, 4, 5, 6, 7];
const FLOWX: i32 = GLASSX + 1;
const FLOWY: i32 = GLASSY - 2;
const FLOWIMG: [u8; 3] = [0x16, 0x17, 0x18];
const PMASKDY: [i32; 2] = [-4, -33];
const PMASKI: [u8; 2] = [0x2c, 0x22];
const PTORCHFLAME: [u8; 18] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 3, 5, 7, 1, 4, 9, 2, 8, 6];
// SUBS.S: the two torches (XCO, OFFSET) and the line their flames sit on
const PTORCHX: [(i32, i32); 2] = [(13, 0), (25, 6)];
const PTORCHY: i32 = 113;
/// chtable6 as FCharTable numbers it
const CH6: u8 = 5;
/// top of the dark band along the bottom of the wall (ours)
const DADO_TOP: i32 = 128;

pub fn draw(scr: &mut Screen, g: &Game, cut: &Cut) {
    scr.clear();
    stand_in(scr);
    let ch6 = apple::chtable(CH6, cut.level);
    let lay = |scr: &mut Screen, img: u8, x: i32, y: i32, op: Op, ink: Ink| {
        if let Some(i) = ch6.and_then(|t| t.get(img)) {
            scr.lay(&i, x, y, false, op, ink);
        }
    };
    // pburn: the torch flames, straight onto the screen
    for (t, &(x, off)) in PTORCHX.iter().enumerate() {
        let f = PTORCHFLAME.get(cut.torch[t] as usize).copied().unwrap_or(1);
        lay(scr, f, x * 7 + off, PTORCHY, Op::Sta, Ink::Flame);
    }
    // pstars: a twinkling star is EORed over the sky
    for (n, &c) in cut.stars.iter().enumerate() {
        if c > 0 {
            lay(scr, STARI[n], STARX * 7, STARY[n], Op::Eor, Ink::Flame);
        }
    }
    // the back plane: the hourglass
    if let Some(gs) = cut.glass {
        lay(scr, GLASSIMG[gs.min(8) as usize], GLASSX * 7, GLASSY, Op::Sta, Ink::Floor);
    }
    // the mid plane: kid, vizier or mouse, then the princess, sorted as
    // sortlist does (the lower one on the screen goes behind)
    let mut chars: Vec<&Char> = [&g.kid, &g.shad].into_iter().filter(|c| c.posn != 0).collect();
    if chars.len() == 2 && chars[1].y > chars[0].y {
        chars.swap(0, 1);
    }
    for c in chars {
        let (f, _) = crate::bg::setupchar(c);
        let Some(img) = apple::chtable(f.table, cut.level).and_then(|t| t.get(f.image)) else { continue };
        let mirror = f.face & 0x80 == 0;
        let ink = if matches!(c.id, 0 | 5) { Ink::Kid } else { Ink::Guard };
        scr.lay(&img, f.x, f.y as i32, mirror, Op::Mask, ink);
        // PMASK: the princess's face and hair in some positions
        if c.id == 5 {
            let m = match c.posn {
                19 => Some(0),
                1 | 18 => Some(1),
                _ => None,
            };
            if let Some(m) = m {
                if let Some(i) = ch6.and_then(|t| t.get(PMASKI[m])) {
                    scr.lay(&i, f.x, f.y as i32 + PMASKDY[m], mirror, Op::And, Ink::None);
                }
            }
        }
    }
    // the front plane: the big white post
    lay(scr, POSTIMG, POSTX * 7, POSTY, Op::Or, Ink::Wall);
    // pflow: sand falling into the glass, down to the sand already in it
    if let (Some(gs), true) = (cut.glass, cut.sand >= 0) {
        if gs < 8 {
            if let Some(i) = ch6.and_then(|t| t.get(FLOWIMG[cut.sand as usize % 3])) {
                let bot = GLASSY - SANDHT[gs as usize];
                scr.lay_clip(&i, FLOWX * 7, FLOWY, false, Op::Sta, Ink::Floor, (0, 0, SCREEN_W, bot));
            }
        }
    }
}

/// Our stand-in for the room's picture: a stone wall in courses, an arched
/// window on the left for the stars, niches for the two torches, a dark
/// doorway on the right where the others come in, and the floor.
fn stand_in(scr: &mut Screen) {
    let floor = FLOOR_Y as i32 + 1;
    // the wall: courses of 12 lines, stones 28 wide, offset every other
    // course, a lit top edge and a light stipple, dark joints; kept quiet
    // so the characters read against it
    let dado = DADO_TOP;
    for y in 8..dado {
        let course = (y - 8) / 12;
        let row = (y - 8) % 12;
        for x in 0..SCREEN_W {
            let sx = (x + if course % 2 == 1 { 14 } else { 0 }) % 28;
            let joint = row == 11 || sx == 27;
            if joint {
                continue;
            }
            let on = row == 0 || ((x + y) % 2 == 0 && row % 3 == 1);
            if on {
                scr.plot(x, y, Ink::Wall, true);
            }
        }
    }
    // a dark dado under a rail, where the mouse and a lying princess show
    for y in dado..floor {
        for x in 0..SCREEN_W {
            let rail = y < dado + 2;
            let panel = (x % 28 == 0 || x % 28 == 1) && y > dado + 4 && y < floor - 3;
            if rail || panel {
                scr.plot(x, y, Ink::Wall, rail);
            }
        }
    }
    // a beam along the top
    for y in 0..8 {
        for x in 0..SCREEN_W {
            if y == 6 || (y < 6 && (x + y) % 3 == 0) {
                scr.plot(x, y, Ink::Wall, false);
            }
        }
    }
    // the window: a pointed arch, open to the night sky
    let (wl, wr, wt, wb) = (6, 30, 84, 122);
    let cx = (wl + wr) / 2;
    for y in wt - 6..=wb + 3 {
        for x in wl - 3..=wr + 3 {
            let inside = |x: i32, y: i32, grow: i32| {
                let half = (wr - wl) / 2 + grow;
                if y > wb + grow || x < cx - half || x > cx + half {
                    return false;
                }
                // the arch: the sides close in over the top 12 lines
                let top = wt - grow;
                if y < top {
                    return false;
                }
                let d = (y - top).min(12);
                (x - cx).abs() <= half * (d + 4) / 16
            };
            if inside(x, y, 0) {
                scr.plot(x, y, Ink::None, false);
            } else if inside(x, y, 3) {
                scr.plot(x, y, Ink::Wall, false);
            }
        }
    }
    for (x, y) in [(10, 92), (25, 96), (16, 104), (27, 110), (9, 116), (21, 119)] {
        scr.plot(x, y, Ink::Wall, false);
    }
    // the sill
    for y in wb + 1..wb + 4 {
        for x in wl - 5..=wr + 5 {
            scr.plot(x, y, Ink::Floor, y == wb + 1);
        }
    }
    // torch niches behind the flames, with a bracket under each
    for &(bx, off) in &PTORCHX {
        let x0 = bx * 7 + off - 3;
        for y in PTORCHY - 18..=PTORCHY + 2 {
            for x in x0..x0 + 13 {
                scr.plot(x, y, Ink::None, false);
            }
        }
        for y in PTORCHY + 1..PTORCHY + 5 {
            for x in x0 + 2..x0 + 11 {
                if y == PTORCHY + 1 || (x - x0 - 6).abs() <= 1 {
                    scr.plot(x, y, Ink::Gate, false);
                }
            }
        }
    }
    // the doorway on the right
    let (dl, dt) = (240, 64);
    for y in dt..floor {
        for x in dl..SCREEN_W {
            let d = y - dt;
            let arch = d >= 10 || (x - dl) >= 10 - d;
            if arch {
                scr.plot(x, y, Ink::None, false);
            }
        }
    }
    for y in dt - 3..floor {
        for x in dl - 4..dl {
            scr.plot(x, y, Ink::Wall, (x + y) % 2 == 0);
        }
    }
    // the floor: a lit edge, a carpet, then the dark front of the slab
    for y in floor..SCREEN_H {
        for x in 0..SCREEN_W {
            let d = y - floor;
            let (ink, on, pal) = if d < 2 {
                (Ink::Floor, true, false)
            } else if d < 10 {
                (Ink::Floor, (x + y) % 2 == 0, true)
            } else if d == 10 {
                (Ink::Floor, true, false)
            } else {
                (Ink::Wall, (x + 2 * y) % 4 == 0 && d < 30, false)
            };
            if on {
                scr.plot(x, y, ink, pal);
            }
        }
    }
}
