use block::ConcreteBlock;
use objc::{
    class, msg_send,
    runtime::{BOOL, NO, Object},
    sel, sel_impl,
};

/// AppKit's event types for a key going down and coming up.
const KEY_DOWN: u64 = 10;
const KEY_UP: u64 = 11;

/// The Command key's bit in an event's modifier flags.
const COMMAND: u64 = 1 << 20;

/// The device bits of the left and right Option keys in an event's modifier flags.
const LEFT_OPTION: u64 = 0x20;
const RIGHT_OPTION: u64 = 0x40;

/// What gpui drops from the key event AppKit is handling.
#[derive(Clone, Copy, Debug)]
pub struct KeyFacts {
    pub left_option: bool,
    pub right_option: bool,
    /// Held down; gpui loses this for keys AppKit routes through commands.
    pub repeat: bool,
}

/// The facts of the key event AppKit is handling now.
pub fn key_facts() -> anyhow::Result<KeyFacts> {
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let event: *mut Object = msg_send![app, currentEvent];
        anyhow::ensure!(!event.is_null(), "macOS: no key event to read");
        let kind: u64 = msg_send![event, type];
        anyhow::ensure!(
            kind == KEY_DOWN || kind == KEY_UP,
            "macOS: the current event is type {kind}, not a key"
        );
        let flags: u64 = msg_send![event, modifierFlags];
        let repeat: BOOL = msg_send![event, isARepeat];
        Ok(KeyFacts {
            left_option: flags & LEFT_OPTION != 0,
            right_option: flags & RIGHT_OPTION != 0,
            repeat: repeat != NO,
        })
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

/// Hands the focused view the Command key-ups AppKit swallows, as Ghostty does.
pub fn deliver_command_key_ups() -> anyhow::Result<()> {
    let handler = ConcreteBlock::new(|event: *mut Object| -> *mut Object {
        unsafe {
            let flags: u64 = msg_send![event, modifierFlags];
            let window: *mut Object = msg_send![event, window];
            if flags & COMMAND == 0 || window.is_null() {
                return event;
            }
            let view: *mut Object = msg_send![window, firstResponder];
            let _: () = msg_send![view, keyUp: event];
            std::ptr::null_mut()
        }
    })
    .copy();
    unsafe {
        let monitor: *mut Object = msg_send![
            class!(NSEvent),
            addLocalMonitorForEventsMatchingMask: 1u64 << KEY_UP
            handler: &*handler
        ];
        anyhow::ensure!(
            !monitor.is_null(),
            "macOS: AppKit refused the key-up monitor"
        );
    }
    log::info!("macOS: Command key-ups reach the focused view");
    Ok(())
}
