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

## T08

- Round 1, 25365a9: FAIL. Ely's focus bindings took F6 and Shift-F6; any kitty flag forced disambiguation; IME commits ignored kitty; macOS repeats routed through AppKit's commands went out as presses; Cmd combos never reached kitty; a stray key.zig of 876 lines slipped past the line rule. Fixed in 11d7a30: NoAction bindings under Terminal; each flag reshapes only what it names; Cmd is Super, bit 8; repeats read from AppKit's event; a local monitor hands Command key-ups to the view; key.zig out; the line rule reads every text file. The IME finding was answered: kitty's keys.c and Ghostty's encoder send commits as UTF-8 under every flag. Mutations of the encoder shown red in a scratch copy.
- Round 2, 11d7a30: PASS.

## T09

- Round 1, 9f81f1f: FAIL. Resetting mode 9 revived an older 1000 (alacritty keeps each mouse mode apart); a second held button wiped the first one's release; URLs lost their combining marks; selections logged nothing; two shot comments ran past 200 characters. Fixed in d6fa38e: the tap holds one tracking and one format as xterm does; reported buttons kept as a set, motion reports the first held; marks kept; selection start, run and end logged; the gesture syntax moved to AGENTS.md. shot now hands real buttons to the view and shows its window without activating.
- Round 2, d6fa38e: FAIL. A NUL inside CSI ? 1000 l ended the tap's sequence while vte runs it and goes on, so tracking stayed on. Fixed in 8bb2763: the scanner mirrors vte's escape and CSI transitions (C0 runs, DEL and high bytes skipped, CAN and SUB cancel, ESC restarts, subparameters count toward 32 values); a test compares it with vte's parser after every byte. Copy on select and OSC 52 primary writes go to Linux's selection.
- Round 3, 8bb2763: PASS.

## T10

- Round 1, 4d8788f: FAIL. Matches went stale after a resize or a scrollback trim; the current match kept its index, not its line; alacritty's search read no combining marks; whole word used ASCII edges; a reset left the old title. Fixed in 1c21dfd: find runs Ely's find_all on the grid's shown text (marks, hidden cells blank, wrapped rows joined, Unicode words); output, resizes and settings changes search again; the current match follows its line; a reset replays the title.
- Round 2, 1c21dfd: FAIL. Following was quadratic on a long line (60 s on a million characters); text appended to the current line moved the current match; a reset inside a synchronized update kept the old title. Fixed in 11b2ce6: one pass, a forward walk and line ends found once, guarded by a timed test; identical lines rank first, then lines grown or cut, then line and column; a cleared or trimmed history moves no line; the title replays on CursorBlinkingChange, in the parser's order.
- Round 3, 11b2ce6: FAIL. With two identical lines, growing the current one moved the match to its twin. The three-round cap reached: fixed in 39761d9 on a red-green test, a line kept, grown or cut ranks first when held under the same line above or in its place. The reviewer's round 2 and 3 scenario records replayed with fresh homes, focus and long-line cases aside, all as expected; three mutations of the rule each fail a test.

## T11

- Round 1, 34ce046: FAIL. A press on a blank column or a header gave focus to the focus root, below which the Workspace key context and its toggle actions sat, so Cmd-B and Cmd-Alt-B did nothing. Fixed in 0de9e15: the context and its actions sit on the outermost element, above ResizeBorder and the focus root; the four press-then-toggle cases hide both columns and save both flags.
- Round 2, 0de9e15: PASS.
