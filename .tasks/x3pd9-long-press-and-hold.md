---
status: todo
tags: [windows, driver, rust]
---
# Long-press gesture and hold behaviour

## Goal
Per button, choose between hold-to-repeat (today's behaviour, e.g. holding
the small button keeps lowering the volume) and a long-press binding
(e.g. big long = Mute) after `LongPressMs` (~600 ms). A long press can also
end a click series: click+long (one click, then press and hold), and
possibly double-click+long, each with its own binding, e.g.
`configure.ps1 Big:hold=Mute Big.Big:hold=Next`.

## Wanted first (2026-11-22)
Click+hold on either button repeats its volume key while held: Big
(Volume Up) and Small (Volume Down). The user's setup is `E9 = CD` (Big
tap: Play/Pause) and `E9 E9 = E2` (Big double: Mute). The second press of
Big is a hold once it stays down `LongPressMs`, and a double click if it
comes up sooner. The key then stays down until release, so Windows repeats
it, like an uncontested single tap (ADR 7). A click+hold binding on Small
makes Small's single tap contested, so it comes one click window late, and
a plain hold of Small no longer repeats.

## Decisions
- The two are exclusive per button: a long-press binding means the key is
  not sent while held.
- Rust driver only, like Tk7wn2 (ADR 6).
- A series that ends in a hold is counted like one that ends in a release:
  N clicks then a hold is "N clicks + long", and a count with no binding
  sends nothing, as in Tk7wn2.

## Next
Build on Tk7wn2's pattern model (ADR 7): a hold is a `Step` in
`driver-rs/report/src/gesture.rs` whose `down` span starts at `LongPressMs`.
An uncontested pattern then fires while the button is still held, and a
contested one once its series ends. Measured (ADR 2): the remote sends one
down report and keeps the bit set until release; it does not repeat.
- Decide whether a long press on a contested button fires once `LongPressMs`
  passes or on release. Firing while held is the usual choice.
- A syntax for hold steps in `configure.ps1` (e.g. `Big:hold`,
  `Big.Big:hold`) and in the `Gestures` lines of `settings.ps1` and
  `Bindings::set_line` in `driver-rs/report/src/bindings.rs`, checked by
  `tests/protocol_test.ps1`.
- Chords (two buttons down at once) as a step of a button set.
