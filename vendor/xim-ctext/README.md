# xim-ctext

ELYTerminal's compound text decoder, in the place of crates.io's xim-ctext.

gpui's X11 input method client, zed-xim 0.4.0-zed, asks for `xim-ctext` `^0.3` and calls `expect` on each decode, so a decode error stops the app. crates.io's 0.3.0 refuses GB2312. xim-rs's newer decoder stops at JIS X 0212 and types Big5 and GBK segments as text. IBus and fcitx5 write all three for Chinese.

This crate keeps the name, the version and the one function zed-xim's client calls, `compound_text_to_utf8`; `[patch.crates-io]` in ELYTerminal's `Cargo.toml` takes it. It reads what libX11 writes, as ibus-x11 runs it in the C locale and as fcitx5's xcb-imdkit carries its tables. `tests/ime.rs` holds those bytes.

Remove it once gpui's zed-xim takes a released decoder that reads them.
