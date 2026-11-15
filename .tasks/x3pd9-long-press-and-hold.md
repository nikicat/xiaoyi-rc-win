---
status: blocked
deps: [Tk7wn2]
tags: [windows, driver]
---
# Long-press gesture and hold behaviour

## Goal
Per button, choose between hold-to-repeat (today's behaviour, e.g. holding
the small button keeps lowering the volume) and a long-press binding
(e.g. big long = Mute) after `LongPressMs` (~600 ms).

## Decisions
- The two are exclusive per button: a long-press binding means the key is
  not sent while held.

## Next
Build on Tk7wn2's state machine and timer. Measured (ADR 2): the remote
sends one down report and keeps the bit set until release; it does not
repeat. So a long press is detected by the timer firing while the button is
still down, and hold-to-repeat is Windows' own behaviour on a held bit.
