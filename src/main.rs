use ely_gpui_component::{
    Assets,
    primitives::FocusScope,
    theme::{ActiveTheme, Mode, Theme},
};
use elyterminal::settings::SettingsStore;
use gpui::{
    App, AppContext, Bounds, Context, FocusHandle, IntoElement, Render, Styled, TitlebarOptions,
    Window, WindowBounds, WindowOptions, px, size,
};

struct Workspace {
    focus: FocusHandle,
}

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        FocusScope::new(&self.focus)
            .root()
            .size_full()
            .bg(cx.theme().colors.bg)
    }
}

/// A Linux app with no display server fails at start instead of running headless.
#[cfg(target_os = "linux")]
fn application() -> gpui::Application {
    gpui_platform::linux(gpui::WindowingModes::WAYLAND | gpui::WindowingModes::X11)
}

#[cfg(not(target_os = "linux"))]
fn application() -> gpui::Application {
    gpui_platform::application()
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("elyterminal=info"))
        .init();
    application().with_assets(Assets).run(|cx: &mut App| {
        ely_gpui_component::init(cx).expect("Ely failed to start");
        if let Err(error) = SettingsStore::init(cx) {
            log::error!("{error:#}");
            std::process::exit(1);
        }
        Theme::set_mode_now(Mode::Dark, cx);
        let bounds = Bounds::centered(None, size(px(1200.0), px(760.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("ELYTerminal".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        cx.open_window(options, |window, cx| {
            cx.new(|cx| {
                let focus = cx.focus_handle();
                window.focus(&focus, cx);
                Workspace { focus }
            })
        })
        .expect("the main window failed to open");
        log::info!("app: main window open");
        cx.activate(true);
    });
}
