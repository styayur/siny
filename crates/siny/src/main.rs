// SPDX-License-Identifier: AGPL-3.0-only
#![cfg_attr(
    all(not(debug_assertions), feature = "gui"),
    windows_subsystem = "windows"
)]
#[cfg(feature = "gui")]
fn run_gui() -> Result<(), String> {
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
    .map_err(|error| error.to_string())
}
#[cfg(not(feature = "gui"))]
fn run_gui() -> Result<(), String> {
    Err("Build with the gui feature to run the desktop app".into())
}

fn main() {
    use siny::cli::{self, Command};
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        attach_console();
    }
    let result = match cli::parse(&args) {
        Ok(Command::Help) => {
            println!("{}", cli::HELP);
            Ok(())
        }
        Ok(Command::Version) => {
            println!("Siny {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Ok(Command::Gui) => run_gui(),
        Ok(Command::File(options)) => {
            #[cfg(feature = "sherpa")]
            {
                cli::recognize(&options).map(|text| println!("{text}"))
            }
            #[cfg(not(feature = "sherpa"))]
            {
                let _ = options;
                Err("Build with the sherpa feature for file recognition".into())
            }
        }
        Err(error) => {
            eprintln!("Siny: {error}");
            std::process::exit(2);
        }
    };
    if let Err(error) = result {
        eprintln!("Siny: {error}");
        std::process::exit(1);
    }
}

#[cfg(all(windows, feature = "sherpa"))]
fn attach_console() {
    use windows_sys::Win32::System::Console::{
        AttachConsole, GetStdHandle, ATTACH_PARENT_PROCESS, STD_OUTPUT_HANDLE,
    };
    // Preserve inherited redirected stdout; only attach a GUI-subsystem process without an output handle.
    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if handle.is_null() || handle as isize == -1 {
            let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}
#[cfg(not(all(windows, feature = "sherpa")))]
fn attach_console() {}
