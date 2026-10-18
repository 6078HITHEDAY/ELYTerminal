//! Ely's window on Windows: owned by the main window, so it stays above it and hides with it, and moved as it moves.

use std::sync::atomic::{AtomicIsize, Ordering};

use anyhow::bail;
use gpui::{Window, px};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    UI::WindowsAndMessaging::{
        CallWindowProcW, GWLP_HWNDPARENT, GWLP_WNDPROC, GetWindowRect, HWND_NOTOPMOST,
        MA_NOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW,
        SetWindowPos, WM_MOUSEACTIVATE, WNDPROC,
    },
};

use super::{HEAD, ROOM, left};

/// gpui's window procedure, which hers passes every message on to but a press's activation.
static GPUI: AtomicIsize = AtomicIsize::new(0);

/// Her window and the main one.
pub struct Glue {
    pet: HWND,
    main: HWND,
}

impl Glue {
    /// Gives her window the main window as owner, takes away the topmost place gpui gives a popup, and keeps a press from activating it, so the keys stay with the terminal.
    pub fn new(pet: &Window, main: &Window) -> anyhow::Result<Self> {
        let (pet, main) = (native(pet)?, native(main)?);
        unsafe {
            SetWindowLongPtrW(pet, GWLP_HWNDPARENT, main as isize);
            let gpui = SetWindowLongPtrW(pet, GWLP_WNDPROC, inactive as *const () as isize);
            anyhow::ensure!(gpui != 0, "pet: her window's procedure does not read");
            GPUI.store(gpui, Ordering::Relaxed);
            let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
            anyhow::ensure!(
                SetWindowPos(pet, HWND_NOTOPMOST, 0, 0, 0, 0, flags) != 0,
                "pet: her window keeps the topmost place"
            );
        }
        Ok(Self { pet, main })
    }

    /// Stands her on the main window's top edge, as far in as its width allows.
    pub fn place(&self, main: &Window) -> anyhow::Result<()> {
        let scale = main.scale_factor();
        let mut frame = RECT::default();
        unsafe {
            anyhow::ensure!(
                GetWindowRect(self.main, &mut frame) != 0,
                "pet: the main window's place does not read"
            );
            let width = px((frame.right - frame.left) as f32 / scale);
            let x = frame.left + (left(width) * scale).round() as i32;
            let y = frame.top - ((HEAD + ROOM) * scale).round() as i32;
            let flags = SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE;
            anyhow::ensure!(
                SetWindowPos(self.pet, std::ptr::null_mut(), x, y, 0, 0, flags) != 0,
                "pet: her window does not move"
            );
        }
        Ok(())
    }
}

/// Her window's procedure: a press does not activate her window; all else is gpui's.
unsafe extern "system" fn inactive(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_MOUSEACTIVATE {
        return MA_NOACTIVATE as LRESULT;
    }
    let gpui: WNDPROC = unsafe { std::mem::transmute(GPUI.load(Ordering::Relaxed)) };
    unsafe { CallWindowProcW(gpui, window, message, wparam, lparam) }
}

/// The HWND gpui made for `window`.
fn native(window: &Window) -> anyhow::Result<HWND> {
    let handle = HasWindowHandle::window_handle(window)
        .map_err(|error| anyhow::anyhow!("pet: no native handle: {error:?}"))?;
    let RawWindowHandle::Win32(win32) = handle.as_raw() else {
        bail!("pet: not a Win32 window");
    };
    Ok(win32.hwnd.get() as HWND)
}
