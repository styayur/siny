// SPDX-License-Identifier: AGPL-3.0-only
//! Windows tray controls, adapted from the preserved AGPL TinySTT tray implementation.
#[derive(Debug, Clone, Copy)]
pub enum TrayAction {
    Open,
    ToggleShortcuts,
    Stop,
    Exit,
}

#[cfg(windows)]
mod platform {
    use super::TrayAction;
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

    pub struct SystemTray {
        _menu: Menu,
        _tray: TrayIcon,
        events: std::sync::mpsc::Receiver<TrayAction>,
        toggle: MenuItem,
    }
    impl SystemTray {
        pub fn new(enabled: bool, ctx: eframe::egui::Context) -> siny_core::Result<Self> {
            let menu = Menu::new();
            let open = MenuItem::with_id("siny-open", "Open Siny", true, None);
            let toggle = MenuItem::with_id("siny-shortcuts", label(enabled), true, None);
            let stop = MenuItem::with_id("siny-stop", "Stop / Cancel", true, None);
            let separator = PredefinedMenuItem::separator();
            let exit = MenuItem::with_id("siny-exit", "Exit Siny", true, None);
            menu.append_items(&[&open, &toggle, &stop, &separator, &exit])
                .map_err(|e| e.to_string())?;
            let ids = [
                open.id().clone(),
                toggle.id().clone(),
                stop.id().clone(),
                exit.id().clone(),
            ];
            let (sender, events) = std::sync::mpsc::sync_channel(16);
            MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
                let action = ids.iter().position(|id| id == event.id()).map(|index| {
                    [
                        TrayAction::Open,
                        TrayAction::ToggleShortcuts,
                        TrayAction::Stop,
                        TrayAction::Exit,
                    ][index]
                });
                if let Some(action) = action {
                    let _ = sender.try_send(action);
                    ctx.request_repaint();
                }
            }));
            let mut rgba = vec![0; 32 * 32 * 4];
            for y in 0..32 {
                for x in 0..32 {
                    if (x as i32 - 16).pow(2) + (y as i32 - 16).pow(2) <= 14 * 14 {
                        rgba[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4]
                            .copy_from_slice(&[14, 116, 144, 255]);
                    }
                }
            }
            for (x, height) in [(10, 10), (14, 18), (18, 24), (22, 14)] {
                for y in 16 - height / 2..16 + height / 2 {
                    rgba[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4]
                        .copy_from_slice(&[245, 250, 252, 255]);
                }
            }
            let icon = Icon::from_rgba(rgba, 32, 32).map_err(|e| e.to_string())?;
            let tray = TrayIconBuilder::new()
                .with_menu(Box::new(menu.clone()))
                .with_tooltip("Siny · offline dictation and speech")
                .with_icon(icon)
                .build()
                .map_err(|e| e.to_string())?;
            Ok(Self {
                _menu: menu,
                _tray: tray,
                events,
                toggle,
            })
        }
        pub fn poll(&self) -> Option<TrayAction> {
            self.events.try_recv().ok()
        }
        pub fn set_enabled(&self, enabled: bool) {
            self.toggle.set_text(label(enabled));
        }
    }
    fn label(enabled: bool) -> &'static str {
        if enabled {
            "Disable shortcuts"
        } else {
            "Enable shortcuts"
        }
    }
}
#[cfg(not(windows))]
mod platform {
    use super::TrayAction;
    pub struct SystemTray;
    impl SystemTray {
        pub fn new(_: bool, _: eframe::egui::Context) -> siny_core::Result<Self> {
            Err("System tray requires Windows".into())
        }
        pub fn poll(&self) -> Option<TrayAction> {
            None
        }
        pub fn set_enabled(&self, _: bool) {}
    }
}
pub use platform::SystemTray;
