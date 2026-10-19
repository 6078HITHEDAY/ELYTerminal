//! Ely's window on X11: gpui's popup, which no window manager places, put on an edge of the main window's pane and just above it.

use anyhow::{Context as _, bail};
use gpui::{Bounds, Pixels, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use x11rb::{
    protocol::xproto::{Atom, AtomEnum, ConfigureWindowAux, ConnectionExt as _, StackMode},
    rust_connection::RustConnection,
};

use super::{Edge, HEAD, ROOM, edge, left, pane};

/// Her X window, the main one, a connection of her own to their server, and the atom of the work area its window manager names.
pub struct Glue {
    connection: RustConnection,
    pet: u32,
    main: u32,
    workarea: Atom,
}

impl Glue {
    pub fn new(pet: &Window, main: &Window) -> anyhow::Result<Self> {
        let (pet, main) = (native(pet)?, native(main)?);
        // gpui opens $DISPLAY too; a window's place belongs to whoever asks.
        let (connection, _) = x11rb::connect(None).context("pet: no X server")?;
        let workarea = (connection.intern_atom(false, b"_NET_WORKAREA"))?
            .reply()
            .context("pet: the work area's atom")?
            .atom;
        let glue = Self {
            connection,
            pet,
            main,
            workarea,
        };
        match glue.work_top(glue.frame()?.0)? {
            Some(top) => log::info!("pet: the work area starts {top} pixels down the screen"),
            None => log::info!(
                "pet: the window manager names no work area; the screen's top bounds her"
            ),
        }
        Ok(glue)
    }

    /// Stands her on the main window's pane, on its top edge while her box fits under the top of the work area, else on its bottom edge, as far in as its width allows, stacked just above the main window's frame, and gives the edge.
    pub fn place(&self, main: &Window) -> anyhow::Result<Edge> {
        self.put(main.scale_factor(), pane(main))
    }

    /// Places her at `scale` on an edge of `pane`, in the main window's points.
    fn put(&self, scale: f32, pane: Bounds<Pixels>) -> anyhow::Result<Edge> {
        let (root, frame) = self.frame()?;
        let at = (self.connection)
            .translate_coordinates(self.main, root, 0, 0)?
            .reply()
            .context("pet: the main window's place")?;
        let down = |y: Pixels| i32::from(at.dst_y) + (f32::from(y) * scale).round() as i32;
        let room = down(pane.origin.y) - self.work_top(root)?.unwrap_or(0);
        let edge = edge(room as f32 / scale);
        let bottom = down(match edge {
            Edge::Top => pane.origin.y,
            Edge::Bottom => pane.bottom(),
        });
        let x = f32::from(pane.origin.x) + left(pane.size.width);
        let aux = ConfigureWindowAux::new()
            .x(i32::from(at.dst_x) + (x * scale).round() as i32)
            .y(bottom - ((HEAD + ROOM) * scale).round() as i32)
            .sibling(frame)
            .stack_mode(StackMode::ABOVE);
        (self.connection)
            .configure_window(self.pet, &aux)?
            .check()
            .context("pet: the X server did not move her window")?;
        Ok(edge)
    }

    /// The top of the work area the window manager names in `_NET_WORKAREA`, its first desktop's, which managers keep alike; none where it names none.
    fn work_top(&self, root: u32) -> anyhow::Result<Option<i32>> {
        let reply = (self.connection)
            .get_property(false, root, self.workarea, AtomEnum::CARDINAL, 0, 4)?
            .reply()
            .context("pet: the work area")?;
        Ok(reply
            .value32()
            .and_then(|mut area| area.nth(1))
            .map(|top| top as i32))
    }

    /// Maps her window while the main one shows, and unmaps it while that hides.
    pub fn shown(&self, visible: bool) -> anyhow::Result<()> {
        let cookie = if visible {
            self.connection.map_window(self.pet)?
        } else {
            self.connection.unmap_window(self.pet)?
        };
        cookie
            .check()
            .context("pet: her window did not map or unmap")
    }

    /// The root, and the main window's top window under it: the frame a window manager put round it, or itself.
    fn frame(&self) -> anyhow::Result<(u32, u32)> {
        let mut window = self.main;
        loop {
            let tree = self.connection.query_tree(window)?.reply()?;
            if tree.parent == tree.root {
                return Ok((tree.root, window));
            }
            window = tree.parent;
        }
    }
}

/// The X window gpui made for `window`.
fn native(window: &Window) -> anyhow::Result<u32> {
    let handle = HasWindowHandle::window_handle(window)
        .map_err(|error| anyhow::anyhow!("pet: no native handle: {error:?}"))?;
    let RawWindowHandle::Xcb(xcb) = handle.as_raw() else {
        bail!("pet: not an X11 window");
    };
    Ok(xcb.window.get())
}

#[cfg(test)]
mod tests {
    use x11rb::{
        COPY_DEPTH_FROM_PARENT,
        connection::Connection as _,
        protocol::xproto::{CreateWindowAux, MapState, PropMode, WindowClass},
        wrapper::ConnectionExt as _,
    };

    use gpui::{point, px, size};

    use super::*;
    use crate::testx11::Xvfb;

    #[test]
    fn she_stands_on_an_edge_of_the_pane_just_above_the_frame_and_hides_with_it() {
        let xvfb = Xvfb::start();
        let (connection, screen) = x11rb::connect(Some(&xvfb.display)).expect("the test connects");
        let root = connection.setup().roots[screen].root;
        let window =
            |parent, (x, y, width, height): (i16, i16, u16, u16), aux: &CreateWindowAux| {
                let id = connection.generate_id().expect("a window id");
                let class = WindowClass::INPUT_OUTPUT;
                (connection.create_window(
                    COPY_DEPTH_FROM_PARENT,
                    id,
                    parent,
                    x,
                    y,
                    width,
                    height,
                    0,
                    class,
                    0,
                    aux,
                ))
                .expect("the request")
                .check()
                .expect("a window");
                connection
                    .map_window(id)
                    .expect("the request")
                    .check()
                    .expect("mapped");
                id
            };
        let plain = CreateWindowAux::new();
        let pet = window(
            root,
            (0, 0, 112, 124),
            &CreateWindowAux::new().override_redirect(1),
        );
        // A window manager's frame, its title bar 20 pixels tall, round the main window.
        let frame = window(root, (100, 400, 800, 620), &plain);
        let main = window(frame, (0, 20, 800, 600), &plain);
        window(root, (0, 0, 50, 50), &plain);
        let hers = x11rb::connect(Some(&xvfb.display)).expect("hers").0;
        let workarea = (hers.intern_atom(false, b"_NET_WORKAREA"))
            .expect("the request")
            .reply()
            .expect("the atom")
            .atom;
        let glue = Glue {
            connection: hers,
            pet,
            main,
            workarea,
        };
        let pane = Bounds::new(point(px(10.), px(10.)), size(px(380.), px(280.)));
        let roomy = glue.put(2.0, pane).expect("she is placed");
        let check = x11rb::connect(Some(&xvfb.display))
            .expect("the check connects")
            .0;
        let at = check
            .get_geometry(pet)
            .expect("the request")
            .reply()
            .expect("her place");
        let tall = ((HEAD + ROOM) * 2.0).round() as i32;
        let stands = (
            100 + ((10.0 + left(px(380.))) * 2.0).round() as i32,
            440 - tall,
        );
        assert_eq!(
            (roomy, i32::from(at.x), i32::from(at.y)),
            (Edge::Top, stands.0, stands.1)
        );
        let order = check
            .query_tree(root)
            .expect("the request")
            .reply()
            .expect("the stack")
            .children;
        let place = |id| {
            order
                .iter()
                .position(|&child| child == id)
                .expect("on the root")
        };
        assert_eq!(
            place(pet),
            place(frame) + 1,
            "just above the frame: {order:?}"
        );
        let mapped = |check: &x11rb::rust_connection::RustConnection| {
            check
                .get_window_attributes(pet)
                .expect("the request")
                .reply()
                .expect("her state")
                .map_state
        };
        glue.shown(false).expect("hidden");
        assert_eq!(mapped(&check), MapState::UNMAPPED);
        glue.shown(true).expect("shown");
        assert_ne!(mapped(&check), MapState::UNMAPPED);
        // A panel reserves the screen's top 300 pixels, so the pane's top lies 70 points under it.
        let area = [0, 300, 1024, 724];
        (check.change_property32(PropMode::REPLACE, root, workarea, AtomEnum::CARDINAL, &area))
            .expect("the request")
            .check()
            .expect("a work area");
        let cramped = glue.put(2.0, pane).expect("she is placed again");
        let low = check
            .get_geometry(pet)
            .expect("the request")
            .reply()
            .expect("her place");
        let inside = 420 + ((10.0 + 280.0) * 2.0_f32).round() as i32 - tall;
        assert_eq!(
            (cramped, i32::from(low.x), i32::from(low.y)),
            (Edge::Bottom, stands.0, inside),
            "with no room above, inside on the pane's bottom edge"
        );
    }
}
