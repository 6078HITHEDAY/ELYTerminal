use objc::{
    class, msg_send,
    runtime::{NO, Object},
    sel, sel_impl,
};

/// AppKit's event types for a key going down and coming up.
const KEY_DOWN: u64 = 10;
const KEY_UP: u64 = 11;

/// The device bits of the left and right Option keys in an event's modifier flags.
const LEFT_OPTION: u64 = 0x20;
const RIGHT_OPTION: u64 = 0x40;

/// Which Option keys are down in the key event AppKit is handling: left, then right.
pub fn option_sides() -> anyhow::Result<(bool, bool)> {
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let event: *mut Object = msg_send![app, currentEvent];
        anyhow::ensure!(!event.is_null(), "macOS: no key event to read Option from");
        let kind: u64 = msg_send![event, type];
        anyhow::ensure!(
            kind == KEY_DOWN || kind == KEY_UP,
            "macOS: the current event is type {kind}, not a key"
        );
        let flags: u64 = msg_send![event, modifierFlags];
        Ok((flags & LEFT_OPTION != 0, flags & RIGHT_OPTION != 0))
    }
}

/// Turns off the accent menu a held key opens, so held keys repeat as in other terminals.
pub fn repeat_held_keys() {
    unsafe {
        let key: *mut Object = msg_send![
            class!(NSString),
            stringWithUTF8String: c"ApplePressAndHoldEnabled".as_ptr()
        ];
        let off: *mut Object = msg_send![class!(NSNumber), numberWithBool: NO];
        let defaults: *mut Object =
            msg_send![class!(NSDictionary), dictionaryWithObject: off forKey: key];
        let store: *mut Object = msg_send![class!(NSUserDefaults), standardUserDefaults];
        let _: () = msg_send![store, registerDefaults: defaults];
    }
    log::info!("macOS: held keys repeat");
}
