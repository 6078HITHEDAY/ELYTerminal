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

## T12

- Round 1, 5fd674e: FAIL. A session switched while a mouse button was down left the hidden terminal's pressed button set: its release landed on the other session, and back on the first one plain moves went out as drags. Fixed in dcf9f7b: a terminal about to be hidden ends its mouse gesture as releases would, the selection drag ending and each heard press reported coming up at the last cell in the current protocol; the reviewer's repro now gives the program a press and its release, as the single-session control does.
- Round 2, dcf9f7b: PASS.

## T13

- Round 1, 8369d96: FAIL. A file or folder named with a newline aborted gpui's line shaping under Ely's Ellipsis; on macOS a decomposed name on disk missed Git's composed mark; a folder named with a newline split the rev-parse prefix, so marks fell on a wrong path. Fixed in 937dab8: a label built from a path shows control characters as their pictures, the session row's folder too, which shared the cause; on macOS a name meets Git's mark in composed form; the prefix keeps its own newlines.
- Round 2, 937dab8: PASS.

## T14

- Round 1, e296668: FAIL. A staged copy kept its source as an old path, so unstaging or diffing the copy took the source's own staged change along; paths went through lossy text before git saw them, so a name not in UTF-8 could unstage another file that showed alike; diffs still ran text converters, which can hide a real change; a CRLF warning on stderr failed a valid untracked diff; a conflict where our side deleted the file diffed as empty. Fixed in 911500c: paths stay git's bytes until shown, Files' keys and listings too, which shared the cause; a copy is a new file; diffs pass --no-textconv; an untracked diff stands when git printed one, its warning logged; a conflict diffs against HEAD.
- Round 2, 911500c: PASS.

## T15

- Round 1, a8a30d3: FAIL. After a folder change the old view stayed until the new read landed, so an action pressed in between reached the repository left behind; a diff still loading from the old folder popped up over the new one; a slow commit's success emptied a draft typed while it ran. Fixed in 27270ae: no action runs until the shown folder has read, and the view blanks on a folder outside the shown work tree; a folder change drops a diff still loading; a commit empties only the fields that still hold what it committed; actions chain, each after the one asked before it, a race of the same kind found while fixing.
- Round 2, 27270ae: FAIL. Entering a nested repository kept the outer view while it read, and a commit finishing then emptied a retyped draft: its completion checked the view, not the read. Fixed in fcd0792: the completion passes the same gate as every action.
- Round 3, fcd0792: PASS.

## T16

- Round 1, 3e1535e: FAIL. The shell check asked only for an execute bit, so a file the running user may not run, as 0641 or an execute-deny ACL, saved and then failed every new session; a bare name counted relative PATH folders, which hang on each session's own folder, so the check and the start could find different programs. Fixed in 3c51b01: the check asks access(X_OK), and a bare name looks only in PATH's full folders; a session runs the full path the check found.
- Round 2, 3c51b01: FAIL. On Windows the .exe candidate replaced an explicit extension, so review.com saved and review.exe ran; a program found at a path not in UTF-8 saved, though the start takes UTF-8 and refused it. Fixed in 9e69288: .exe joins only a name without an extension; the shared resolver returns the program as UTF-8 text and refuses a match that is not, before the save.
- Round 3, 9e69288: PASS.

## T17

- Round 1, f2cc16f: FAIL. The font picker saved a family gpui cannot load, such as Apple Color Emoji, which has no m, and the terminal drew with a fallback without a word; after a refused save a number field kept the refused draft, so once a later save cleared the message the field and the settings disagreed. Fixed in ee37018: a font saves, and a settings file starts, only when gpui loads it as the terminal draws it, and the terminal draws with no other font; numbers save on commit; a refusal resets the number fields and the font picker to the settings.
- Round 2, ee37018: PASS.

## T18

- Round 1, 764ff45: PASS.

## T19

- Round 1, c98bbad: PASS.

## T20

- Round 1, 6523632 and 107069b: PASS. 6523632 fixed a test the first Linux run caught: on Linux a program that ignores SIGHUP reads EIO when its pty closes, so GNU cat exits 1 where macOS gives EOF.

## T21

- Round 1, b5e4967: PASS. The work found that Ely's ResizeBorder painted a client-drawn window's pane solid, so no Linux window showed the desktop and T20's blur could not show; Ely 5dc4dd1 adds ResizeBorder::glass, and T20's region now covers the pane alone.

## T22

- Round 1, 5ee67bd: FAIL. Edit's Copy, Paste and Select All named the terminal's own actions, so with find, a settings field or the commit field in focus gpui disabled them and a menu press did nothing; Select All took a space under a combining mark for a blank and dropped the mark at either end; two new comments ran long. Fixed in 37220a7: Ely a2cb531, approved by the user, exports its text input's Copy, Cut, Paste and SelectAll, which the terminal and the menu now name; a cell with a mark counts as a character; every new comment is one short line.
- Round 2, 37220a7: PASS.

## T23

- Round 1, e3b5cfb..82bdbaf: PASS. The owner's checks on the release build found three faults, fixed in 09930c2 before the review: on macOS 27 gpui's Selection material drew no blur, the debug build lagged, and no button hid the side columns. CI went green once the owner let the runners be put right: fish and PowerShell on the macOS runner, rustup's proxies on the Windows runner, a Linux runner of the repository's own; on the way, run 907 caught the PowerShell test typing before the line editor read keys.

## T24

- Round 1, c0a9eb3..65bb4d3: PASS. Before the review, 65bb4d3 named the icon's parts as SVG groups instead of comments, drawing the same pixels, and made mac-app.sh read the version in a clone named elyterminal. CI run 926 on 65bb4d3 passed on macOS, Ubuntu and Windows.

## T25

- Round 1, 2b8ae5d and 0075667: FAIL. The v0.1.0 release was not yet on Gitea, so the task line's release stood open.
- Round 2, 59e41ea: PASS. v0.1.0 at 0075667 carries the disk image, the Windows zip and the Linux tarball; the reviewer downloaded each and found its SHA-256, signature, icon, manifest and glibc as recorded. CI run 934 on 0075667 passed on macOS, Ubuntu and Windows.

## T26

- Round 1, 6f7d3a9 and 0f87676 with Ely cdc5ce8: FAIL. v0.1.1 was still a draft, so the task line's release stood open.
- Round 2, c316d27, 90a4625, 7bfb462, 43ea5b1 and 194c7bb: PASS. The owner's later asks went in first: a gear for Settings on Windows and Linux, Ctrl-V paste on Windows, the arrow pointer, the settings window without General, with the shortcut list and with columns that scroll apart, and the release's disk image signed with Developer ID and notarized on the macos-signing runner. v0.1.1 at 43ea5b1 is published and the repository public; the reviewer downloaded each file anonymously, checked its hash, signature and notarization, and drove the app on X11 and macOS. CI run 955 on 43ea5b1 passed on macOS, Ubuntu and Windows.

## T27

- Round 1, 340c7a9 and d8ebade: FAIL. A failed read marked the file's stamp as seen, so a theme file made readable again was never worn; `is_file`, `exists` and `metadata().ok()` took a permission error for absence, so a fresh install wore Catppuccin Mocha over an unreadable Omarchy folder, and an outage while following logged nothing. Fixed in aae7fba: the watcher compares the file's text, logs a failure once while it lasts and tries again; only a missing file means no Omarchy, and a file that cannot be read stops the start. The v0.1.1 Linux tarball was rebuilt from aae7fba on the owner's PC, which now runs Omarchy, and took the place of the one from 340c7a9 (1faf8ca).
- Round 2, aae7fba and 1faf8ca: PASS. The reviewer reproduced both faults on the parent build and saw them gone, ran all 22 of Omarchy's themes through the window, and matched the new tarball's download to its SHA-256. CI run 976 on aae7fba passed on macOS and Ubuntu; its Windows job waits for the PC to run Windows again.

## T28

- Round 1, 741e48b with the v0.1.1 release notes: PASS. The reviewer read the new lines against Hyprland 0.56.2's `shouldBlur` and background-effect protocol and Omarchy 4.0.4's default `looknfeel.lua`, and the owner's captures with Hyprland's blur on and off against the acceptance row, which stays Partial for Wayland.

## T29

- Round 1, 3e30f95: FAIL. A shell that ran `exec herdr` kept its process group, so the session never looked at its foreground again and the panel stayed where it was; a relative `HERDR_SOCKET_PATH` was looked up from ELYTerminal's own folder, not the client's, so no socket answered. Fixed in 2af45cb: the foreground is its group and the executable its leader runs, a program that did not read is tried again, and a relative socket lies in the client's folder (`/proc/PID/cwd`, `proc_pidinfo` on macOS); on Linux an executable an update replaced still names itself herdr.
- Round 2, 2af45cb: PASS. The reviewer ran real herdr 0.9.5 under the native view: a direct start, a delayed exec, a relative socket, both at once, and a folder with a space and an é each gave the folder events herdr's own snapshot named. CI run 988 on 3e30f95 and run 990 on 2af45cb passed on macOS, Ubuntu and Windows.

## T30

- Round 1, 1bf420c: FAIL. On X11 a maximized window, minimized, wrote the screen's size, since gpui's X11 window calls a maximized window unmaximized while it is hidden; and the window could be dragged under the 100 by 100 the settings keep, so a smaller size was refused and lost. Fixed in 2f58a06: a hidden window keeps the size from before, and the window is no smaller than 100 by 100.
- Round 2, 2f58a06: FAIL. A size the window took just before something covered it was skipped while it was hidden and never read again once it showed. Fixed in 9729d24: a window that shows again is read again; and the window opens centered at whole points, since macOS grew a window whose centered edge fell on a half point by a point, so a kept 900 by 600 had opened at 900 by 601.
- Round 3, 9729d24: PASS. The reviewer replayed the cover on macOS through gpui and AppKit, against the parent too, and saw the size kept once shown and the restart open at it; the window opened at its kept size and a move wrote nothing; on X11 a maximized, minimized and restored window and a resize minimized at once kept the right size, and the 100 by 100 minimum held. CI runs 993 on 1bf420c and 995 on 2f58a06 passed on macOS, Ubuntu and Windows; run 996 on 9729d24 passed on Ubuntu and Windows.

## T31

- Round 1, 4364802 and 2a1060c, with release v0.1.2: PASS. The reviewer compared `Cargo.lock` with its parent, where only elyterminal moved to 0.1.2; matched the 112 files of the source the builds came from to the tag byte for byte; downloaded the three files anonymously against their SHA-256s; checked the disk image's notarization and its app's signature, the zip's icon, manifest and imports, and the tarball's glibc 2.35; and ran the downloaded macOS app with herdr 0.9.5: the folder followed herdr's focus, and the window reopened at its kept 923 by 617. CI run 1000 on 4364802 passed on macOS, Ubuntu and Windows before the release was published; Release run 1001 built the notarized disk image.
