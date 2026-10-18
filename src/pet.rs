//! Ely, the icon's girl, peeking over the main window's top edge from a window of her own.

mod faces;
mod look;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
mod x11;

use std::{
    cell::RefCell,
    f32::consts::PI,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    Animation, AnimationExt as _, App, AppContext as _, Bounds, Context, Decorations, Image,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Render, Styled,
    Subscription, Task, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind,
    WindowOptions, div, img, point, popup, px, size,
};

#[cfg(target_os = "macos")]
use self::macos as platform;
#[cfg(windows)]
use self::windows as platform;
#[cfg(target_os = "linux")]
use self::x11 as platform;
use self::{
    faces::Face,
    look::{Mood, PETTED, face, mood},
};
use crate::{settings::SettingsStore, workspace::Workspace};

/// Her head's box, in points, and the room above it her hops take.
const WIDTH: f32 = 112.0;
const HEAD: f32 = 107.4;
const ROOM: f32 = 16.0;
/// How far in from the pane's left edge she holds its top edge.
const LEFT: f32 = 140.0;

/// Ely in her window: her face follows the sessions' marks, and a press pets her.
pub struct Pet {
    faces: Vec<(Face, Arc<Image>)>,
    mood: Mood,
    /// When the mood began.
    since: Instant,
    petted: Option<Instant>,
    /// Counts her moves, so each plays from its start.
    moves: usize,
    /// What keeps her window on the main window's edge, where the platform does not.
    glue: Option<platform::Glue>,
    _wake: Option<Task<()>>,
    _watch: Vec<Subscription>,
}

/// Shows Ely over `main` while the settings want her.
pub fn follow(main: WindowHandle<Workspace>, cx: &mut App) {
    let shown: Rc<RefCell<Option<WindowHandle<Pet>>>> = Rc::default();
    let sync = move |cx: &mut App| {
        let open = *shown.borrow();
        match (SettingsStore::get(cx).ely, open) {
            (true, None) => match show(main, cx) {
                Ok(pet) => *shown.borrow_mut() = Some(pet),
                Err(error) => log::error!("pet: Ely does not show: {error:#}"),
            },
            (false, Some(pet)) => {
                *shown.borrow_mut() = None;
                log::info!("pet: Ely goes, as the settings say");
                if pet
                    .update(cx, |_, window, _| window.remove_window())
                    .is_err()
                {
                    log::debug!("pet: her window had gone");
                }
            }
            _ => {}
        }
    };
    sync(cx);
    cx.observe_global::<SettingsStore>(sync).detach();
}

/// Opens her window on `main`'s top edge: a popup Wayland's compositor places, else a window kept there.
fn show(main: WindowHandle<Workspace>, cx: &mut App) -> anyhow::Result<WindowHandle<Pet>> {
    let (bounds, inset, wayland) = main.update(cx, |_, window, _| {
        (window.bounds(), top_inset(window), wayland(window))
    })?;
    let room = size(px(WIDTH), px(HEAD + ROOM));
    let kind = if wayland {
        WindowKind::AnchoredPopup(popup::PopupOptions {
            parent: main.into(),
            anchor_rect: Bounds::new(point(inset + px(LEFT), inset), size(px(1.), px(1.))),
            anchor: popup::PopupAnchor::TopLeft,
            gravity: popup::PopupGravity::TopRight,
            constraint_adjustment: popup::PopupConstraintAdjustment::empty(),
            offset: point(px(0.), px(0.)),
            grab: false,
        })
    } else {
        WindowKind::PopUp
    };
    let origin = point(
        bounds.origin.x + inset + px(LEFT),
        bounds.origin.y + inset - room.height,
    );
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(origin, room))),
        titlebar: None,
        focus: false,
        show: true,
        kind,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let workspace = main.entity(cx)?;
    let pet = cx.open_window(options, |_, cx| {
        cx.new(|cx| {
            let feel = cx.observe(&workspace, |pet: &mut Pet, workspace, cx| {
                let sessions = workspace.read(cx).sessions();
                let now = mood(sessions.iter().filter_map(|session| session.looks(cx)));
                if now != pet.mood {
                    pet.feel(now, cx);
                }
            });
            let mut pet = Pet {
                faces: faces::images(),
                mood: Mood::Calm,
                since: Instant::now(),
                petted: None,
                moves: 0,
                glue: None,
                _wake: None,
                _watch: vec![feel],
            };
            pet.wake(cx);
            pet
        })
    })?;
    if !wayland {
        glue(main, pet, cx)?;
    }
    let way = if wayland {
        "a Wayland popup"
    } else {
        "a window kept there"
    };
    log::info!("pet: Ely peeks over the main window, {way}");
    Ok(pet)
}

/// Keeps her window on `main`'s top edge as it moves, comes forward and hides.
fn glue(main: WindowHandle<Workspace>, pet: WindowHandle<Pet>, cx: &mut App) -> anyhow::Result<()> {
    main.update(cx, |_, main, cx| {
        pet.update(cx, |pet, window, cx| {
            let glue = platform::Glue::new(window, main)?;
            glue.place(main)?;
            pet.glue = Some(glue);
            pet._watch
                .push(cx.observe_window_bounds(main, |pet, main, _| pet.place(main)));
            pet._watch
                .push(cx.observe_window_activation(main, |pet, main, _| pet.place(main)));
            #[cfg(target_os = "linux")]
            pet._watch.push(
                cx.observe_window_visibility(main, |pet, visibility, main, _| {
                    pet.shown(visibility.is_visible(), main)
                }),
            );
            anyhow::Ok(())
        })?
    })?
}

/// The pane's distance from its window's top: the resize margin of a window that draws its own frame and is free at the top.
fn top_inset(window: &Window) -> Pixels {
    match (window.window_decorations(), window.client_inset()) {
        (Decorations::Client { tiling }, Some(inset)) if !tiling.top => inset,
        _ => px(0.),
    }
}

/// Whether `window` is a Wayland surface, where the compositor places windows.
#[cfg(target_os = "linux")]
fn wayland(window: &Window) -> bool {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    HasWindowHandle::window_handle(window)
        .is_ok_and(|handle| matches!(handle.as_raw(), RawWindowHandle::Wayland(_)))
}

#[cfg(not(target_os = "linux"))]
fn wayland(_: &Window) -> bool {
    false
}

impl Pet {
    /// Takes up `mood`: a new face, and the move it starts with.
    fn feel(&mut self, mood: Mood, cx: &mut Context<Self>) {
        log::debug!("pet: Ely feels {mood:?}, was {:?}", self.mood);
        (self.mood, self.since) = (mood, Instant::now());
        self.moves += 1;
        self.wake(cx);
        cx.notify();
    }

    /// Her face now, a press's wink first.
    fn face(&self, now: Instant) -> (Face, Option<Duration>) {
        match self.petted.map(|at| now.saturating_duration_since(at)) {
            Some(held) if held < PETTED => (Face::Petted, Some(PETTED - held)),
            _ => face(self.mood, now.saturating_duration_since(self.since)),
        }
    }

    /// Draws her again when her face changes next.
    fn wake(&mut self, cx: &mut Context<Self>) {
        let (_, next) = self.face(Instant::now());
        self._wake = next.map(|next| {
            cx.spawn(async move |pet, cx| {
                cx.background_executor().timer(next).await;
                if pet
                    .update(cx, |pet, cx| {
                        pet.wake(cx);
                        cx.notify();
                    })
                    .is_err()
                {
                    log::debug!("pet: her window went before her next face");
                }
            })
        });
    }

    fn place(&self, main: &Window) {
        if let Some(glue) = &self.glue
            && let Err(error) = glue.place(main)
        {
            log::error!("{error:#}");
        }
    }

    #[cfg(target_os = "linux")]
    fn shown(&self, visible: bool, main: &Window) {
        if let Some(glue) = &self.glue {
            match glue.shown(visible) {
                Ok(()) => log::debug!("pet: Ely shows {visible} with the main window"),
                Err(error) => log::error!("{error:#}"),
            }
        }
        if visible {
            self.place(main);
        }
    }
}

impl Render for Pet {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (face, _) = self.face(Instant::now());
        let image = (self.faces.iter())
            .find(|(drawn, _)| *drawn == face)
            .map(|(_, image)| image.clone())
            .expect("every face is drawn");
        let (mood, first) = (self.mood, self.moves == 0);
        let head = img(image)
            .absolute()
            .left_0()
            .bottom_0()
            .w(px(WIDTH))
            .h(px(HEAD))
            .with_animation(
                ("ely", self.moves),
                Animation::new(Duration::from_millis(600)),
                move |head, delta| match mood {
                    // She rises from behind the edge when she comes.
                    _ if first => head.bottom(px(-HEAD * (1.0 - delta).powi(3))),
                    // A hop: two for a program that waits, one for one that finished.
                    Mood::Waiting | Mood::Done => {
                        let hops = if mood == Mood::Waiting { 2.0 } else { 1.0 };
                        head.bottom(px((delta * hops * PI).sin().abs() * (ROOM - 4.0)))
                    }
                    // A shake that dies away.
                    Mood::Error => head.left(px((delta * 6.0 * PI).sin() * 4.0 * (1.0 - delta))),
                    Mood::Calm | Mood::Working => head,
                },
            );
        div().size_full().relative().child(head).on_mouse_down(
            MouseButton::Left,
            cx.listener(|pet, _, _, cx| {
                log::debug!("pet: Ely is pressed");
                pet.petted = Some(Instant::now());
                pet.wake(cx);
                cx.notify();
            }),
        )
    }
}
