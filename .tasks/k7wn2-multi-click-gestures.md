---
status: todo
tags: [windows, driver]
---
# Double- and triple-click gestures

## Goal
Per-button multi-click bindings, e.g. big: single = Play/Pause,
double = Next, triple = Previous; configured like the remap (Tr4hq7), e.g.
`configure.ps1 Big.Double=Next`, with a `MultiClickMs` window (~300 ms).

## Decisions
- In the filter: it is the only place that can both detect a click and keep
  Windows from acting on it. Emitted keys are limited to the report's 19
  usages.
- A button with no multi-click bindings stays instant; one with bindings
  delays its single click by the window.

## Next
- Design the report path: hold hidclass's pending READ_REPORT requests in a
  manual queue, read from below with our own requests, run a per-button
  press/release state machine with a WDF timer, and complete hidclass's
  requests with synthesized press + release reports.
- Raw reports are in ADR 2, "Input reports". Fast clicks are 200-300 ms
  apart, press to press, and the wake press arrives normally. So a 300 ms
  window measured from each release should be enough, and the wake press
  needs no special case. Count on press/release edges, not durations,
  because a down and its up can arrive together.
