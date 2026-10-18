//! Ely's window on Windows: owned by the main window, so it stays above it and hides with it, and moved as it moves.

use anyhow::bail;
use gpui::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::{
    Foundation::{HWND, RECT},
    UI::WindowsAndMessaging::{
        GWLP_HWNDPARENT, GetWindowRect, HWND_NOTOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos,
    },
};

use super::{HEAD, LEFT, ROOM};

/// Her window and the main one.
pub struct Glue {
    pet: HWND,
    main: HWND,
}

impl Glue {
    /// Gives her window the main window as owner, and takes away the topmost place gpui gives a popup.
    pub fn new(pet: &Window, main: &Window) -> anyhow::Result<Self> {
        let (pet, main) = (native(pet)?, native(main)?);
        unsafe {
            SetWindowLongPtrW(pet, GWLP_HWNDPARENT, main as isize);
            let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
            anyhow::ensure!(
                SetWindowPos(pet, HWND_NOTOPMOST, 0, 0, 0, 0, flags) != 0,
                "pet: her window keeps the topmost place"
            );
        }
        Ok(Self { pet, main })
    }

    /// Stands her on the main window's top edge, `LEFT` points in from its left.
    pub fn place(&self, main: &Window) -> anyhow::Result<()> {
        let scale = main.scale_factor();
        let mut frame = RECT::default();
        unsafe {
            anyhow::ensure!(
                GetWindowRect(self.main, &mut frame) != 0,
                "pet: the main window's place does not read"
            );
            let x = frame.left + (LEFT * scale).round() as i32;
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

/// The HWND gpui made for `window`.
fn native(window: &Window) -> anyhow::Result<HWND> {
    let handle = HasWindowHandle::window_handle(window)
        .map_err(|error| anyhow::anyhow!("pet: no native handle: {error:?}"))?;
    let RawWindowHandle::Win32(win32) = handle.as_raw() else {
        bail!("pet: not a Win32 window");
    };
    Ok(win32.hwnd.get() as HWND)
}
