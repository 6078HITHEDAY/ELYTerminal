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

/// Hands her window the messages posted to it, as the message loop does.
fn deliver(window: HWND) {
    let mut message = MSG::default();
    while unsafe { PeekMessageW(&mut message, window, 0, 0, PM_REMOVE) } != 0 {
        unsafe { DispatchMessageW(&message) };
    }
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

/// Two displays split at 0, 144 DPI before and 96 after, across or down, giving a box the DPI of the one under most of it, as Windows does.
fn displays(down: bool) -> impl Fn(&RECT) -> anyhow::Result<u32> {
    move |rect: &RECT| {
        let (from, to) = if down {
            (rect.top, rect.bottom)
        } else {
            (rect.left, rect.right)
        };
        Ok(if to.min(0) - from > to - from.max(0) {
            144
        } else {
            96
        })
    }
}

/// Placing her box `rect` at `dpi` on `displays`, Windows' rect at each change scaled about its middle, which lands on the new display: the box she settles in and how many changes it took.
fn settle(mut rect: RECT, mut dpi: u32, down: bool) -> (RECT, usize) {
    let dpi_of = displays(down);
    let corner = (rect.left, rect.bottom);
    for changes in 0..8 {
        let now = dpi_of(&rect).expect("a display");
        if now == dpi {
            return (rect, changes);
        }
        let grow = |from: i32, to: i32| {
            let length = ((to - from) as f64 * now as f64 / dpi as f64).round() as i32;
            let start = (from + to - length).div_euclid(2);
            (start, start + length)
        };
        let ((left, right), (top, bottom)) =
            (grow(rect.left, rect.right), grow(rect.top, rect.bottom));
        let after = RECT {
            left,
            top,
            right,
            bottom,
        };
        rect = keeping(corner, now, &dpi_of)
            .expect("a display")
            .unwrap_or(after);
        dpi = now;
    }
    panic!("her DPI goes back and forth");
}

#[test]
fn placing_her_across_two_displays_of_two_dpis_settles_after_one_change() {
    // 168 by 185 at 144 DPI, 70 pixels on the first display and 98 on the second.
    let across = RECT {
        left: -70,
        top: 315,
        right: 98,
        bottom: 500,
    };
    let (rect, changes) = settle(across, 144, false);
    assert_eq!(
        changes, 1,
        "Windows' rect stands where her corner would set her DPI back"
    );
    assert_eq!(displays(false)(&rect).expect("a display"), 96);
    // 112 by 123 at 96 DPI, her bottom 50 pixels down the second display.
    let down = RECT {
        left: 200,
        top: -73,
        right: 312,
        bottom: 50,
    };
    let (rect, changes) = settle(down, 96, true);
    assert_eq!(
        (changes, rect.left, rect.bottom),
        (1, 200, 50),
        "her corner kept on the edge"
    );
    assert_eq!(rect.bottom - rect.top, 185);
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
    deliver(pet);
    let (her, under) = (info(pet), info(main));
    let own = unsafe { GetDpiForWindow(pet) } as f32 / 96.0;
    // New DPIs, each with a rect Windows suggests 30 and 40 pixels off, 100 by 60.
    let display = display_dpi(&her.rcWindow).expect("the display under her");
    let change = |dpi: u32| {
        let at = info(pet).rcWindow;
        let suggested = RECT {
            left: at.left + 30,
            top: at.top + 40,
            right: at.left + 130,
            bottom: at.top + 100,
        };
        let pointer = &suggested as *const RECT as LPARAM;
        unsafe { SendMessageW(pet, WM_DPICHANGED, (dpi | dpi << 16) as WPARAM, pointer) };
        let taken = info(pet).rcWindow;
        deliver(pet);
        (suggested, taken, info(pet).rcWindow)
    };
    let (_, _, kept) = change(display);
    let (suggested, taken, stood) = change(display + 48);
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
    let rect = |rect: RECT| (rect.left, rect.top, rect.right, rect.bottom);
    assert_eq!(
        (
            her.dwStyle & WS_CAPTION,
            frame,
            drawn,
            (outer.left, outer.bottom),
            rect(kept)
        ),
        (0, (0, 0, 0, 0), own_box, edge, rect(outer)),
        "bare, in her own box, her bottom on the pane's top edge, kept there at her display's DPI"
    );
    assert_eq!(
        (rect(taken), rect(stood)),
        (rect(suggested), rect(suggested)),
        "gpui takes Windows' rect, which stands where her corner lies on a display of another DPI"
    );
    assert_ne!(
        (made.right - made.left, made.bottom - made.top),
        own_box,
        "no size until her window takes the posted place"
    );
}
