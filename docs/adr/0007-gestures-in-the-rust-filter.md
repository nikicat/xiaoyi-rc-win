# 7. Gestures in the Rust filter

Status: accepted

## Context

Task Tk7wn2 adds per-button double- and triple-click bindings, for example a
double click on the big button that sends Mute, and task Tx3pd9 adds long
presses. The filter is the only place that can both see a click and keep
Windows from acting on it (Tk7wn2). The remote sends one down report and one
up report per press, and never repeats while held (ADR 2, "Input reports").
A down and its up can share a BLE connection event.

A gesture is a pattern over a series of timed button events, which can
involve more than one button: a double click, a long press, a click and
then a hold, or later two buttons together. Other remotes, with more
buttons, may follow.

ADR 6 first planned gestures in C, to be ported. By the time Tk7wn2
started, the Rust driver had reached parity and had passed the checks on the
remote and under the UMDF verifier (task Tbvzts).

## Decision

- **Rust only, and the C driver is gone.** Gestures are written only in
  `driver-rs`. To get them into a release sooner, the Rust driver became the
  only driver on 2026-11-22, before the upstream work of M3: `driver/` keeps
  only the INF and the project that packages the Rust DLL (ADR 6).

### Layers

Only the lowest layer knows the remote, so that another device is another
description of its reports, and the layers above it stay as they are. Each
layer is a component of its own with one interface, and the events flow
through them in order; one orchestrator wires them, without a general graph
framework.

1. **Device:** turns an input report into the set of buttons that are down,
   and a set of keys into an output report. A *button* is an input of the
   device; a *key* is a usage that the filter reports to Windows. Both are
   among the 19 usages of the remote's report (`docs/report-map.bin`).
2. **Events:** each change of a button's state becomes an event with its
   button, its edge (press or release) and its time, in a history. Every
   button goes through it; none has a path of its own.
3. **Gestures:** recognizers over the history, with an arbiter that decides
   which one fires.
4. **Output:** the keys that fired gestures hold down, plus the taps of the
   gestures that fired, become the reports that Windows gets. The reports to
   Windows are built from the keys alone, with padding bits of 0.
5. **Driver:** the read path below, the window timer, and the registry
   settings.

### Gestures

- **A gesture is passive and stateless.** It is a function of the history
  of events and the current time, and answers *no* (this series cannot be it),
  *maybe* (it could still be, so ask again by a given time) or *yes*. A
  *maybe* or a *yes* names the events that it claims. All the state is in the
  history. So one timer, set to the earliest *maybe*, is enough: when it fires,
  every gesture is asked again.
- **A gesture is a pattern**, not a hardcoded kind: a sequence of steps. A
  step is a press and a release of one button, with two spans: how long the
  button is held, and the gap since the previous step's release. A double
  click is `Big Big`, a triple click `Big Big Big`, and `Big Small` is a tap of
  big and then of small. A tap is held for any time, after a gap shorter than
  the click window. One recognizer matches any pattern, and a binding gives
  the pattern its key. A button's single tap is the one-tap pattern, so a
  button has no separate remap.
- **Gestures have a priority**, and it follows from the patterns: a longer
  pattern ranks above the patterns it starts with. A gesture fires only when
  no gesture of higher priority has claimed one of its events. So `Big Big`,
  which claims the first click while it waits for a second, keeps `Big` from
  firing until the double click is ruled out. A gesture that fires consumes
  the events it claimed. Events that no gesture claims send nothing, so that
  a miscounted series does no harm. They are dropped only once their series
  ends, so that the taps after them in the same series do not start one of
  their own: four fast clicks with a triple click bound send nothing.
- **A contested series is decided when it ends.** A pattern that shares a
  button with another pattern answers *yes* only once no press has followed
  its last release within the click window, and *no* as soon as a press does
  not match its next step. So a hold on such a button is one tap, and its
  single tap comes one window late.
- **An uncontested pattern fires at once.** A pattern that no other shares a
  button with fires as soon as its last step is down, and its key stays down
  until that step's release. So a button whose only gesture is its single
  tap is instant, and Windows repeats its key while it is held, as before
  gestures.
- **Holds and chords come later** (task Tx3pd9): a hold is a step whose held
  span starts at a long-press time, so that it fires while held, and a chord
  is a step of two buttons pressed together.
- **The click window** is measured from each release to the next press.
  Its default is 200 ms: normal double clicks on the remote have about
  100 ms from release to press, and 300 ms felt slow (2026-11-22). The
  times are the reports' arrival times, because the remote's reports carry
  none of their own, and the remote sends only in steps of about 50 ms. At
  150 ms, one late step turned a fast triple click into a double
  (2026-11-22). `MultiClickMs` overrides it.
- **Fast series of the remote lose taps.** Even with every report
  forwarded as it comes, the remote sometimes drops a release and the next
  press, so that two fast taps arrive as one long press (logged on
  2026-11-22, also with the filter of M2). The more taps a pattern has, the
  more often it is missed: double clicks were reliable, triple clicks were
  not.
- **Taps are synthesized.** A gesture that fires sends a press report and a
  release report of its key, with the keys that are held at the time.
- **Settings.** Every binding, single taps included, is a line of one
  `REG_MULTI_SZ` value, `Gestures`, with usages in hex: `E9 = CD` (a tap on
  Volume Up reports Play/Pause), `E9 E9 = E2` (a double click reports Mute).
  WDF cannot enumerate value names, so the patterns cannot be value names of
  their own. A rejected line is ignored, and a later line of a pattern
  replaces an earlier one. The `Remap<usage>` values of the C driver are no
  longer read.
- **Only a single tap can report nothing.** A key of 0 turns a button off
  (`EA = 0`). A pattern of more than one tap must name a key, and a line
  that sends it nothing is ignored; to remove a gesture, delete its line.

### The read path

hidclass keeps several READ_REPORT requests pending, and each one carries
one report up. Under gestures, the remote's reports and the reports that
Windows gets no longer pair one to one: a click sends none, and a series that
ends on the window timer sends two with no report from the remote.

- Each of hidclass's reads goes down to wait for the remote's next report,
  as before. When it comes back, the filter feeds the report to the gesture
  state, and then either completes the read with the next queued report for
  Windows, or sends it down again when none is queued.
- A report that the window timer queues, with every read down below, is
  carried up by cancelling one of those reads. A cancelled read always goes
  up, with the current button state when nothing is queued any more.
- Reports are button states, so they must reach Windows in order: a release
  that overtakes its press leaves the key held down (seen on 2026-11-22).
  So one completer at a time carries the reports up, in the order they were
  queued.
- Nothing is completed, sent or cancelled under the lock of the read state,
  since any of them can bring hidclass's next read, or a read that comes
  back, to the filter on the same thread. Each caller (hidclass's new read, a
  read that comes back, the window timer) records its work under the lock.
  When no completer is running, it becomes the completer. Otherwise it
  returns, and the running completer does that work.
- The completer takes its next action from the state under the lock, and
  performs it after it lets go: it completes a read with a report, sends a
  read down, cancels one to carry a queued report up, or starts the window
  timer. So the timer always follows the latest deadline. The completer
  stops only under the lock, when it finds nothing left to do, so no work
  that another caller records is lost.
- The filter reads the settings again for every report and every timer
  tick, so that a change applies on the next press.
- A failed read resets the gesture state. The remote's button state is then
  unknown: it disconnected, or the read path stopped. So a series in
  progress must not fire later, and no key may stay held.

## Alternatives

- **Active gestures**, each an `async` task that awaits events and timeouts,
  read like the gestures they recognize. In this driver they need a
  hand-written executor woken by the report callback and the WDF timer, and
  cancellation on device teardown. They also still need an arbiter, since two
  gestures can await the same events. Reconsider them if gestures grow into
  long sequences that are awkward as a function of the history.
- **Gestures with their own state and timers** would put a timer per gesture
  on the device, and the state would have to be kept outside the gestures to
  arbitrate between them anyway.
- **WDF's synchronization scope** cannot serialize the read path, since it
  does not cover I/O completion routines: "The framework doesn't synchronize
  a driver's other callback functions, such as the driver's
  CompletionRoutine callback function" (Microsoft docs, "Using Automatic
  Synchronization").

## Consequences

- A button in a pattern of more than one tap answers its single tap one
  window late.
- The settings are `Gestures` and `MultiClickMs`. `tests/protocol_test.ps1`
  checks that `settings.ps1`, which `configure.ps1` uses, writes them as the
  driver reads them. Reading `Gestures` needs `RegistryKey::query_multi_string`
  in our `wdf` fork (task Tpkpmd).
- Settings made for the C driver are lost: its `Remap<usage>` values have to
  be set again with `configure.ps1`.
- New kinds of step, such as a hold or a chord (task Tx3pd9), extend the
  pattern and its recognizer; the arbiter and the settings stay as they are.
