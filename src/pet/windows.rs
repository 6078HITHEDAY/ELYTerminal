//! Ely's window on Windows: a bare popup owned by the main window, so it stays above it and hides with it, sized and moved as it moves.

use std::sync::atomic::{AtomicIsize, Ordering};

use anyhow::bail;
use gpui::{Bounds, Pixels, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        HiDpi::GetDpiForWindow,
        WindowsAndMessaging::{
            CallWindowProcW, GWL_STYLE, GWLP_HWNDPARENT, GWLP_WNDPROC, GetWindowInfo,
            GetWindowLongPtrW, HWND_NOTOPMOST, MA_NOACTIVATE, PostMessageW, SWP_FRAMECHANGED,
            SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos,
            USER_DEFAULT_SCREEN_DPI, WINDOWINFO, WM_APP, WM_MOUSEACTIVATE, WNDPROC, WS_CAPTION,
            WS_POPUP,
        },
    },
};

use super::{HEAD, ROOM, WIDTH, left, pane};

/// gpui's window procedure, which hers passes every message on to but a press's activation and her place.
static GPUI: AtomicIsize = AtomicIsize::new(0);

/// Her place, posted to her window: Windows sends a new size at once, which gpui hears only outside its own updates.
const PLACE: u32 = WM_APP;
const _: () = assert!(
    size_of::<WPARAM>() == 8,
    "a place packs two i32 into a word"
);

/// Her window and the main one.
pub struct Glue {
    pet: HWND,
    main: HWND,
}

impl Glue {
    pub fn new(pet: &Window, main: &Window) -> anyhow::Result<Self> {
        Self::hold(native(pet)?, native(main)?)
    }

    /// Gives her window the main window as owner, a bare popup's frame, none of the topmost place gpui gives a popup, and keeps a press from activating it, so the keys stay with the terminal.
    fn hold(pet: HWND, main: HWND) -> anyhow::Result<Self> {
        unsafe {
            SetWindowLongPtrW(pet, GWLP_HWNDPARENT, main as isize);
            // gpui's popup is overlapped, which Windows gives a caption and a frame.
            let style = (GetWindowLongPtrW(pet, GWL_STYLE) as u32 & !WS_CAPTION) | WS_POPUP;
            SetWindowLongPtrW(pet, GWL_STYLE, style as isize);
            let gpui = SetWindowLongPtrW(pet, GWLP_WNDPROC, inactive as *const () as isize);
            anyhow::ensure!(gpui != 0, "pet: her window's procedure does not read");
            GPUI.store(gpui, Ordering::Relaxed);
            let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED;
            anyhow::ensure!(
                SetWindowPos(pet, HWND_NOTOPMOST, 0, 0, 0, 0, flags) != 0,
                "pet: her window keeps the topmost place or its frame"
            );
        }
        Ok(Self { pet, main })
    }

    /// Stands her on the main window's top edge, as far in as its width allows.
    pub fn place(&self, main: &Window) -> anyhow::Result<()> {
        self.put(main.scale_factor(), pane(main))
    }

    /// Posts her box, sized at her own DPI as gpui did not when it opened her off the display, standing at `scale` over `pane`, in the main window's points.
    fn put(&self, scale: f32, pane: Bounds<Pixels>) -> anyhow::Result<()> {
        let mut main = WINDOWINFO {
            cbSize: size_of::<WINDOWINFO>() as u32,
            ..Default::default()
        };
        let (dpi, read) = unsafe {
            (
                GetDpiForWindow(self.pet),
                GetWindowInfo(self.main, &mut main),
            )
        };
        anyhow::ensure!(dpi != 0, "pet: her window's DPI does not read");
        anyhow::ensure!(read != 0, "pet: the main window's place does not read");
        let own = dpi as f32 / USER_DEFAULT_SCREEN_DPI as f32;
        let (x, y) = (f32::from(pane.origin.x), f32::from(pane.origin.y));
        let at = (
            main.rcClient.left + ((x + left(pane.size.width)) * scale).round() as i32,
            main.rcClient.top + ((y - HEAD - ROOM) * scale).round() as i32,
        );
        let size = (
            (WIDTH * own).round() as i32,
            ((HEAD + ROOM) * own).round() as i32,
        );
        let posted = unsafe { PostMessageW(self.pet, PLACE, pack(at), pack(size) as LPARAM) };
        anyhow::ensure!(posted != 0, "pet: her place does not post");
        log::debug!("pet: Ely is to stand at {at:?}, {size:?} pixels");
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
    if message == PLACE {
        let ((x, y), (width, height)) = (unpack(wparam), unpack(lparam as WPARAM));
        let flags = SWP_NOZORDER | SWP_NOACTIVATE;
        let moved =
            unsafe { SetWindowPos(window, std::ptr::null_mut(), x, y, width, height, flags) };
        if moved == 0 {
            log::error!("pet: her window does not move to {x},{y}");
        }
        return 0;
    }
    let gpui: WNDPROC = unsafe { std::mem::transmute(GPUI.load(Ordering::Relaxed)) };
    unsafe { CallWindowProcW(gpui, window, message, wparam, lparam) }
}

fn pack((low, high): (i32, i32)) -> WPARAM {
    low as u32 as WPARAM | (high as u32 as WPARAM) << 32
}

fn unpack(word: WPARAM) -> (i32, i32) {
    (word as u32 as i32, (word >> 32) as u32 as i32)
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

#[cfg(test)]
mod tests {
    use gpui::{point, px, size};
    use windows_sys::{
        Win32::UI::WindowsAndMessaging::{
            CW_USEDEFAULT, CreateWindowExW, DestroyWindow, DispatchMessageW, MSG, PM_REMOVE,
            PeekMessageW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_OVERLAPPEDWINDOW,
        },
        w,
    };

    use super::*;

    /// A hidden window of `style`, at the size Windows picks, as gpui asks for its windows.
    fn window(style: u32, extended: u32) -> HWND {
        let window = unsafe {
            CreateWindowExW(
                extended,
                w!("STATIC"),
                w!(""),
                style,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        assert!(!window.is_null(), "a test window");
        window
    }

    fn info(window: HWND) -> WINDOWINFO {
        let mut info = WINDOWINFO {
            cbSize: size_of::<WINDOWINFO>() as u32,
            ..Default::default()
        };
        assert_ne!(unsafe { GetWindowInfo(window, &mut info) }, 0, "its info");
        info
    }

    #[test]
    fn the_large_framed_popup_gpui_makes_stands_bare_in_her_box_on_the_panes_top_edge() {
        let main = window(WS_OVERLAPPEDWINDOW, 0);
        // gpui's popup: style 0, an overlapped window, which Windows gives a caption.
        let pet = window(0, WS_EX_TOOLWINDOW | WS_EX_TOPMOST);
        assert_eq!(
            info(pet).dwStyle & WS_CAPTION,
            WS_CAPTION,
            "framed as gpui makes it"
        );
        let glue = Glue::hold(pet, main).expect("held");
        let pane = Bounds::new(point(px(0.), px(0.)), size(px(380.), px(280.)));
        glue.put(1.0, pane).expect("placed");
        let made = info(pet).rcWindow;
        let mut message = MSG::default();
        while unsafe { PeekMessageW(&mut message, pet, 0, 0, PM_REMOVE) } != 0 {
            unsafe { DispatchMessageW(&message) };
        }
        let (her, under) = (info(pet), info(main));
        let own = unsafe { GetDpiForWindow(pet) } as f32 / 96.0;
        unsafe {
            DestroyWindow(pet);
            DestroyWindow(main);
        }
        let (outer, inner) = (her.rcWindow, her.rcClient);
        let frame = (
            inner.left - outer.left,
            inner.top - outer.top,
            outer.right - inner.right,
            outer.bottom - inner.bottom,
        );
        let drawn = (outer.right - outer.left, outer.bottom - outer.top);
        let own_box = (
            (WIDTH * own).round() as i32,
            ((HEAD + ROOM) * own).round() as i32,
        );
        let edge = (
            under.rcClient.left + left(px(380.)).round() as i32,
            under.rcClient.top + (-HEAD - ROOM).round() as i32,
        );
        assert_eq!(
            (
                her.dwStyle & WS_CAPTION,
                frame,
                drawn,
                (outer.left, outer.top)
            ),
            (0, (0, 0, 0, 0), own_box, edge),
            "bare, in her own box, on the pane's top edge"
        );
        assert_ne!(
            (made.right - made.left, made.bottom - made.top),
            own_box,
            "no size until her window takes the posted place"
        );
    }
}
