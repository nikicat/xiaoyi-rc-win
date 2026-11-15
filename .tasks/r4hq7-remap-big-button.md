---
status: todo
tags: [windows]
---
# Optionally remap the big button to Play/Pause on Windows

## Goal
Match the Linux setup (`linux/70-xiaoyi-rc.hwdb`): big button sends
Play/Pause instead of Volume Up. Not needed for the remote to work; the
driver already gives Volume Up / Volume Down.

## Next
Pick one:
- in the filter: rewrite the usage in the descriptor copy we already hold
  (`0x09 0xE9` → `0x09 0xCD` swaps the bit meanings, but Play/Pause is
  already in the same report, so swap the two usages rather than duplicate);
- a per-user key-remapping tool, leaving the driver generic.
