//! Ely's window on Windows: a bare popup owned by the main window, so it stays above it and hides with it, sized and moved as it moves.

use std::sync::atomic::{AtomicIsize, Ordering};

use anyhow::bail;
use gpui::{Bounds, Pixels, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    UI::{
        HiDpi::GetDpiForWindow,
        WindowsAndMessaging::{
            CallWindowProcW, GWL_STYLE, GWLP_HWNDPARENT, GWLP_WNDPROC, GetWindowInfo,
            GetWindowLongPtrW, GetWindowRect, HWND_NOTOPMOST, MA_NOACTIVATE, PostMessageW,
            SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
            SetWindowLongPtrW, SetWindowPos, USER_DEFAULT_SCREEN_DPI, WINDOWINFO, WM_APP,
            WM_DPICHANGED, WM_MOUSEACTIVATE, WNDPROC, WS_CAPTION, WS_POPUP,
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
        let (at, size) = stand(main.rcClient, scale, own, pane);
        let posted = unsafe { PostMessageW(self.pet, PLACE, pack(at), pack(size) as LPARAM) };
        anyhow::ensure!(posted != 0, "pet: her place does not post");
        log::debug!("pet: Ely is to stand at {at:?}, {size:?} pixels");
        Ok(())
    }
}

/// Her box at `own` scale, its bottom on `pane` at `scale` over the main window's `client` rect: its place and size in pixels.
fn stand(client: RECT, scale: f32, own: f32, pane: Bounds<Pixels>) -> ((i32, i32), (i32, i32)) {
    let size = (
        (WIDTH * own).round() as i32,
        ((HEAD + ROOM) * own).round() as i32,
    );
    let (x, y) = (f32::from(pane.origin.x), f32::from(pane.origin.y));
    let at = (
        client.left + ((x + left(pane.size.width)) * scale).round() as i32,
        client.top + (y * scale).round() as i32 - size.1,
    );
    (at, size)
}

/// Her window's procedure: a press does not activate her window, she takes her place, and at a new DPI keeps her corner; all else is gpui's.
unsafe extern "system" fn inactive(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let gpui: WNDPROC = unsafe { std::mem::transmute(GPUI.load(Ordering::Relaxed)) };
    match message {
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        PLACE => {
            let ((x, y), (width, height)) = (unpack(wparam), unpack(lparam as WPARAM));
            let flags = SWP_NOZORDER | SWP_NOACTIVATE;
            let null = std::ptr::null_mut();
            if unsafe { SetWindowPos(window, null, x, y, width, height, flags) } == 0 {
                log::error!("pet: her window does not move to {x},{y}");
            }
            0
        }
        WM_DPICHANGED => {
            // gpui takes the rect Windows suggests, as a move to another display brings.
            let (mut before, mut after) = (RECT::default(), RECT::default());
            let read = unsafe { GetWindowRect(window, &mut before) };
            let answer = unsafe { CallWindowProcW(gpui, window, message, wparam, lparam) };
            if read == 0 || unsafe { GetWindowRect(window, &mut after) } == 0 {
                log::error!("pet: her window's place does not read at a new DPI");
                return answer;
            }
            let top = before.bottom - (after.bottom - after.top);
            let flags = SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE;
            let null = std::ptr::null_mut();
            if unsafe { SetWindowPos(window, null, before.left, top, 0, 0, flags) } == 0 {
                log::error!("pet: her window does not keep its corner at a new DPI");
            } else {
                log::debug!(
                    "pet: at a new DPI Ely keeps her corner at {},{}",
                    before.left,
                    before.bottom
                );
            }
            answer
        }
        _ => unsafe { CallWindowProcW(gpui, window, message, wparam, lparam) },
    }
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
            CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, MSG,
            PM_REMOVE, PeekMessageW, SendMessageW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
            WS_OVERLAPPEDWINDOW,
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

    /// gpui's procedure as far as these tests need it: at a new DPI it takes the rect Windows suggests.
    unsafe extern "system" fn standin(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_DPICHANGED {
            let rect = unsafe { *(lparam as *const RECT) };
            let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
            let flags = SWP_NOZORDER | SWP_NOACTIVATE;
            let null = std::ptr::null_mut();
            unsafe { SetWindowPos(window, null, rect.left, rect.top, width, height, flags) };
            return 0;
        }
        unsafe { DefWindowProcW(window, message, wparam, lparam) }
    }

    #[test]
    fn her_bottom_stands_on_the_panes_top_edge_whatever_the_two_windows_dpi() {
        let client = RECT {
            left: 100,
            top: 20,
            right: 1300,
            bottom: 900,
        };
        let pane = Bounds::new(point(px(10.), px(8.)), size(px(800.), px(600.)));
        for (scale, own) in [(1.0, 1.0), (1.0, 1.5), (1.5, 1.0), (2.0, 1.25)] {
            let ((x, y), (width, height)) = stand(client, scale, own, pane);
            let own_box = (
                (WIDTH * own).round() as i32,
                ((HEAD + ROOM) * own).round() as i32,
            );
            let corner = (
                100 + ((10.0 + left(px(800.))) * scale).round() as i32,
                20 + (8.0 * scale).round() as i32,
            );
            assert_eq!(
                ((width, height), (x, y + height)),
                (own_box, corner),
                "the main window at {scale}, hers at {own}"
            );
        }
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
        unsafe { SetWindowLongPtrW(pet, GWLP_WNDPROC, standin as *const () as isize) };
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
        // A new DPI, whose rect Windows suggests 30 and 40 pixels off, 100 by 60.
        let at = her.rcWindow;
        let suggested = RECT {
            left: at.left + 30,
            top: at.top + 40,
            right: at.left + 130,
            bottom: at.top + 100,
        };
        let pointer = &suggested as *const RECT as LPARAM;
        unsafe { SendMessageW(pet, WM_DPICHANGED, 144 | 144 << 16, pointer) };
        let later = info(pet).rcWindow;
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
            under.rcClient.top,
        );
        let kept = (
            later.left,
            later.bottom,
            later.right - later.left,
            later.bottom - later.top,
        );
        assert_eq!(
            (
                her.dwStyle & WS_CAPTION,
                frame,
                drawn,
                (outer.left, outer.bottom),
                kept
            ),
            (
                0,
                (0, 0, 0, 0),
                own_box,
                edge,
                (outer.left, outer.bottom, 100, 60)
            ),
            "bare, in her own box, her bottom on the pane's top edge, kept there at a new DPI"
        );
        assert_ne!(
            (made.right - made.left, made.bottom - made.top),
            own_box,
            "no size until her window takes the posted place"
        );
    }
}
