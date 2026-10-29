# Prince of Braille

Jordan Mechner's Prince of Persia (Apple II, 1989) in the terminal, ported
routine by routine from his own 6502 source. The prince runs, leaps, hangs
and fights in Braille dots, so the rotoscoped animation keeps every frame;
the dungeons and palaces around him are drawn in block glyphs.

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

Needs a terminal of at least 70x25 with truecolour and a font with the
Braille block. At 140x49 and larger it draws the full 280x192 screen.

## Controls

- Arrow keys, WASD or hjkl to run, climb, crouch and jump
- Home and PgUp, or `q` and `e`: jump up and to the left or right
- Shift or Space: the button (grab a ledge, careful step, pick up, drink,
  strike with the sword); Up blocks
- `r` restart the level, `p` pause, `m` or F8 sound on and off
- `c` colour theme (dusk, phosphor, amber, classic), `g` glyph mode, `f`
  scale
- Space skips a scene in the princess's room; Esc twice quits

In terminals with the kitty keyboard protocol (kitty, WezTerm, foot,
Ghostty and others) keys are read exactly; elsewhere a held key is
recognised from the keyboard's repeat.

## Credits

Prince of Persia is by Jordan Mechner, who released the Apple II source
code in 2012. The images and level layouts come from that source. The music
and the princess's room are the DOS version's, from PrinceJS
(github.com/oklemenz/PrinceJS).

Port by Asciiverse, MIT licensed (see LICENSE).
