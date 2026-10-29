//! Sequence and frame tables, assembled from SEQTABLE.S / FRAMEDEF.S.

#[path = "gen/seqtable.rs"]
mod seqtable;
#[path = "gen/framedef.rs"]
mod framedef;

pub use seqtable::{op, seq, ORG};

/// Start of sequence `n` (1-114) as an index into the table (JUMPSEQ).
pub fn start(n: u8) -> u16 {
    let i = 2 * (n as usize - 1);
    let addr = seqtable::SEQ[i] as u16 | (seqtable::SEQ[i + 1] as u16) << 8;
    addr - seqtable::ORG
}

/// GETSEQ: the byte at `pos`, with SDLPoP's patches to the table.
pub fn byte(pos: u16) -> u8 {
    patched(pos).unwrap_or_else(|| seqtable::SEQ.get(pos as usize).copied().unwrap_or(op::GOTO))
}

/// SDLPoP's apply_seqtbl_patches, at the same places in the Apple table:
/// - bumpfall starts in midair (3), not bumped (5): bumping a wall above a
///   pressure plate no longer presses the plate below (fix_wall_bump_
///   triggers_tile_below);
/// - standing up from a crouch steps 1 forward and 4 back; he now steps 0
///   and 3, so he no longer stands for a moment past an edge
///   (fix_stand_on_thin_air).
fn patched(pos: u16) -> Option<u8> {
    let (bumpfall, standup) = (start(seq::BUMPFALL), start(seq::STANDUP));
    match pos {
        p if p == bumpfall + 1 => Some(3),
        p if p == standup + 11 => Some(0),
        p if p == standup + 16 => Some((-3i8) as u8),
        _ => None,
    }
}

/// A `dw` operand (absolute address) as a table index.
pub fn addr(lo: u8, hi: u8) -> u16 {
    (lo as u16 | (hi as u16) << 8).wrapping_sub(seqtable::ORG)
}

/// Frame definition (FRAMEDEF.S): Fimage, Fsword, Fdx, Fdy, Fcheck.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Frame {
    pub image: u8,
    pub sword: u8,
    pub dx: u8,
    pub dy: u8,
    pub check: u8,
}

pub const FCHECKMARK: u8 = 0b0100_0000;
pub const FTHINMARK: u8 = 0b0010_0000;
pub const FFOOTMARK: u8 = 0b0001_1111;

fn entry(t: &[u8], i: usize) -> Frame {
    match t.get(i * 5..i * 5 + 5) {
        Some(b) => Frame { image: b[0], sword: b[1], dx: b[2], dy: b[3], check: b[4] },
        None => Frame::default(),
    }
}

/// GETFRAME + usealtsets for the kid (CharID 0 uses the main set).
pub fn kid_frame(posn: u8) -> Frame {
    if posn == 0 {
        return Frame::default();
    }
    entry(&framedef::FDEF, posn as usize - 1)
}

/// GETFRAMEINFO + usealtsets: the frame for `posn` as drawn for CharID
/// `id`. The kid (0) and the mouse (24) use the main set; the shadowman (1)
/// takes 150-189 from altset 1; guards and the skeleton (2-4) also swap the
/// falling frames 102-106 for 172-176; the princess and vizier (5 and up)
/// use altset 2 for everything.
pub fn frame(id: u8, posn: u8) -> Frame {
    if id == 0 || id == 24 || posn == 0 {
        return kid_frame(posn);
    }
    if id >= 5 {
        return entry(&framedef::ALTSET2, posn as usize - 1);
    }
    let p = if id >= 2 && (102..107).contains(&posn) { posn + 70 } else { posn };
    if (150..190).contains(&p) {
        entry(&framedef::ALTSET1, (p - 150) as usize)
    } else {
        kid_frame(p)
    }
}

/// Sword table entry (image, dx, dy) for a 1-based sword frame.
pub fn sword(n: u8) -> (u8, u8, u8) {
    let t = &framedef::SWORDTAB;
    let i = (n as usize).saturating_sub(1) * 3;
    match t.get(i..i + 3) {
        Some(b) => (b[0], b[1], b[2]),
        None => (0, 0, 0),
    }
}

/// decodeim: FCharTable and FCharImage from Fimage/Fsword.
pub fn decode_image(f: &Frame) -> (u8, u8) {
    let table = (f.image & 0x80) >> 5 | (f.sword & 0xc0) >> 6;
    (table, f.image & 0x7f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_patches_land_on_the_bytes_sdlpop_patches() {
        let raw = |p: u16| seqtable::SEQ[p as usize];
        let (b, u) = (start(seq::BUMPFALL), start(seq::STANDUP));
        assert_eq!((raw(b), raw(b + 1)), (op::ACT, 5));
        assert_eq!((raw(u + 10), raw(u + 11), raw(u + 12)), (op::CHX, 1, 114));
        assert_eq!((raw(u + 15), raw(u + 16), raw(u + 17)), (op::CHX, (-4i8) as u8, 117));
        assert_eq!((byte(b + 1), byte(u + 11), byte(u + 16)), (3, 0, (-3i8) as u8));
    }

    #[test]
    fn stand_loops_on_frame_15() {
        let s = start(seq::STAND);
        assert_eq!((byte(s), byte(s + 1)), (op::ACT, 0));
        assert_eq!(byte(s + 2), 15);
        assert_eq!(byte(s + 3), op::GOTO);
        assert_eq!(addr(byte(s + 4), byte(s + 5)), s);
    }

    #[test]
    fn frame_15_is_the_standing_kid() {
        let f = kid_frame(15);
        assert_eq!(f, Frame { image: 0x0f, sword: 9, dx: 0, dy: 0, check: 0x43 });
        assert_eq!(decode_image(&f), (0, 0x0f));
        // jumphang frames use chtable2
        assert_eq!(decode_image(&kid_frame(67)).0, 1);
    }

    #[test]
    fn guards_use_altset_1_for_fighting_and_falling() {
        // en garde (158) comes from the guard set, chtable4
        assert_eq!(frame(2, 158), entry(&framedef::ALTSET1, 8));
        assert_eq!(decode_image(&frame(2, 158)).0, 3);
        // falling 102 becomes 172 for guards, not for the shadowman
        assert_eq!(frame(2, 102), frame(2, 172));
        assert_eq!(frame(1, 102), kid_frame(102));
        assert_eq!(frame(0, 158), kid_frame(158));
    }
}
