use std::path::PathBuf;

use ely_gpui_component::{
    i18n::text,
    menus::{Menu, MenuItem},
    primitives::IconName,
};
use gpui::{App, ClipboardItem, Entity};

use super::here_folder;
use crate::workspace::Workspace;

/// What a right press on Files' `path` offers.
pub fn row(workspace: &Entity<Workspace>, path: PathBuf, cx: &App) -> Menu {
    let (open, here, copy) = (path.clone(), workspace.clone(), path.clone());
    let folder = here_folder(&path);
    Menu::new()
        .item(
            MenuItem::new(text(cx, "et.files.open", &[]))
                .icon(IconName::ExternalLink)
                .on_click(move |_, cx| {
                    log::info!("files: opening {}", open.display());
                    cx.open_with_system(&open);
                }),
        )
        .item(
            MenuItem::new(text(cx, "et.files.new_session", &[]))
                .icon(IconName::Terminal)
                .on_click(move |window, cx| {
                    here.update(cx, |this, cx| this.here(folder.clone(), window, cx))
                }),
        )
        .separator()
        .item(
            MenuItem::new(text(cx, "et.files.copy_path", &[]))
                .icon(IconName::Copy)
                .on_click(move |_, cx| {
                    log::info!("files: copied {}", copy.display());
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        copy.to_string_lossy().into_owned(),
                    ));
                }),
        )
}
