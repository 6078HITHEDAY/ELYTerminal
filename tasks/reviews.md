# Reviews

Each task gets at most three codex rounds. Each round: the commit, the verdict, the findings and what was done.

## T01

- Round 1, 4e2f3c9: FAIL. Linux had no X11 or Wayland backend and would start headless; macOS CI linked no binary; `Cargo.lock` passed the 500-line rule unstated. Fixed in 9c0c067 and f8857fe: `wayland` and `x11` on, Linux starts with `WAYLAND | X11` and no headless; `cargo build --locked` in macOS CI; the lockfile named as exempt.
- Round 2, f8857fe: PASS. The Windows runner's missing `rustc` goes to T23.

## T02

- Round 1, e1770ba: FAIL. One shared temp name let overlapping saves corrupt the file; parse errors lost the field name. Fixed in 83ec4b1: a temp per save (pid, counter, create_new), cleaned on failure; serde_path_to_error names the field; a four-writer test, red on the old save.
- Round 2, 83ec4b1: PASS.

## T03

- Round 1, 7d4d535: PASS. Theme files match upstream 21f281d byte for byte.

## T04

- Round 1, 62e50fe: FAIL. A folder without enter permission started the shell elsewhere; no pid for the app; folder and exit unlogged; grid and pty sizes split past the kernel's cap. Fixed in 96ae85b.
- Round 2, 96ae85b: FAIL. The permission check raced alacritty's chdir, whose result alacritty ignores; closing a live pty logged nothing. Fixed in fff436c: ELYTerminal's own Unix pty, a checked chdir in pre_exec that fails the spawn, closing and hung-up logs with a test.
- Round 3, fff436c: FAIL. Drop waited on the program while holding the master, so a program ignoring SIGHUP never ended. Fixed in 0e7f309: the master closes before the wait; a test that traps HUP went red, then green.
- The review cap of three rounds is reached. The round-3 fix stands on its red-green test, not a fourth round.

## T05

- Round 1, dfaf378: FAIL. fish searched <dir>/fish/fish, so its script never loaded (fish 4 sends OSC 7 itself and hid it); PowerShell's Restricted policy would block the script; control bytes in a folder name broke OSC 7; bash and PowerShell prompts reset $?; install swallowed read errors. Fixed in c2da9dd, each break shown red in a scratch copy.
- Round 2, c2da9dd: PASS.

## T06

- Round 1, ae1d8ce: FAIL. SGR 8 text showed under the block cursor, in emoji and in underlines; an inactive window kept a solid cursor; combining marks dropped below their base (gpui adds CoreText's upward y offsets to downward screen y). Fixed in 3c8f746: hidden cells blank in the frame; window activation counts as focus; every glyph painted from the shaped layout on one baseline, CoreText's y flipped on macOS.
- Round 2, 3c8f746: PASS.

## T07

- Round 1, 387bfa1: FAIL. Dim geometry brightened where pieces overlapped (gpui blends each quad and path triangle on its own); a combining mark sent its drawn base back to the font, thicker and past the cell. Fixed in 2804aa6: overlaps cut from rects, strokes turned into polygons, the cross cut around its first band, a drawn base drawn under its marks; pixels of the dim row checked, no value above the dim color.
- Round 2, 2804aa6: PASS.
