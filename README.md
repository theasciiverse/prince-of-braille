# Prince of Braille

Jordan Mechner's Prince of Persia (Apple II, 1989) in the terminal, ported
routine by routine from his own 6502 source, and drawn with the DOS
version's art. The prince runs, leaps, hangs and fights in Braille dots, so
the rotoscoped animation keeps every frame; the dungeons and palaces around
him are drawn in block glyphs.

Sixty minutes, fourteen levels: gates and pressure plates, loose floors,
spikes and slicers, guards with their own fighting styles, the skeleton, the
shadow, the potions, the vizier and the princess waiting in her room.

## Play

With [av](https://asciiverse.sh), the Asciiverse client:

```
av install prince-of-braille
av play prince-of-braille
```

Or from the source: `cargo run --release -- [LEVEL]`. `--demo` plays the
original's attract demo; any key starts a game from it.

Needs a terminal with truecolour and a font with the Braille block. The
picture fills the window at any size; 80x24 is playable, 160x59 shows every
pixel of the DOS art.

## Controls

- Arrow keys, WASD or hjkl to run, climb, crouch and jump
- Home and PgUp, or `q` and `e`: jump up and to the left or right
- Shift or Space: the button (grab a ledge, careful step, pick up, drink,
  strike with the sword); Up blocks
- `r` restart the level, `p` pause, `m` or F8 sound on and off
- `v` switches between the DOS art and the Apple II picture; `g` glyph
  mode; with the Apple picture, `c` colour theme (dusk, phosphor, amber,
  classic) and `f` scale
- Space skips a scene in the princess's room; Esc twice quits

In terminals with the kitty keyboard protocol (kitty, WezTerm, foot,
Ghostty and others) keys are read exactly; elsewhere a held key is
recognised from the keyboard's repeat.

## Differences from the original

Bugs in the original are fixed the way SDLPoP fixes them: you can no
longer press a plate through a closed gate, walk through a wall next to an
open gate or a slicer, land safely on spikes that haven't sprung, fall
onto a guard unhurt or grab a ledge two storeys down, and guards don't
follow you through closed gates.

## Credits

Prince of Persia is by Jordan Mechner, who released the Apple II source
code in 2012. The level layouts and the Apple II picture come from that
source. The art, the princess's room and the music are the DOS version's,
from PrinceJS (github.com/oklemenz/PrinceJS).

Port by Asciiverse, MIT licensed (see LICENSE).
