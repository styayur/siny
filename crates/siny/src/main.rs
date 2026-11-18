// SPDX-License-Identifier: AGPL-3.0-only
#![cfg_attr(
    all(not(debug_assertions), feature = "gui"),
    windows_subsystem = "windows"
)]
#[cfg(feature = "gui")]
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([800.0, 680.0])
            .with_min_inner_size([600.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Siny",
        options,
        Box::new(|cc| Ok(Box::new(siny::app::SinyApp::new(cc)))),
    )
}
#[cfg(not(feature = "gui"))]
fn main() {
    eprintln!("Build with the gui feature to run Siny; sherpa enables headless acceptance.");
}
