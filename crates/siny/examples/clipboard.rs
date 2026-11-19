// SPDX-License-Identifier: AGPL-3.0-only
//! Opt-in interactive acceptance using a fresh, self-owned child window and public fixture text only.
//! Never run while another task holds the desktop. No existing app/document is inspected or activated.
#[cfg(windows)]
mod windows_probe {
    use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering};
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    const TEXT: &str = "Siny public clipboard acceptance.";
    static EDIT: AtomicIsize = AtomicIsize::new(0);
    static PASSED: AtomicBool = AtomicBool::new(false);
    static TICKS: AtomicUsize = AtomicUsize::new(0);
    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    unsafe extern "system" fn window_proc(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_DESTROY => {
                PostQuitMessage(0);
                return 0;
            }
            WM_TIMER => {
                let mut text = [0_u16; 128];
                // EDIT was created in this process; text points to exactly 128 writable UTF-16 units.
                let count = GetWindowTextW(
                    EDIT.load(Ordering::Acquire) as HWND,
                    text.as_mut_ptr(),
                    text.len() as i32,
                );
                if count > 0 && String::from_utf16_lossy(&text[..count as usize]) == TEXT {
                    PASSED.store(true, Ordering::Release);
                    DestroyWindow(window);
                } else if TICKS.fetch_add(1, Ordering::Relaxed) > 600 {
                    DestroyWindow(window);
                }
                return 0;
            }
            _ => (),
        }
        DefWindowProcW(window, message, wparam, lparam)
    }

    fn target() -> Result<(), String> {
        let class = wide("SinyClipboardAcceptance");
        let title = wide("Siny clipboard acceptance");
        let edit_class = wide("EDIT");
        // All class/title pointers remain live throughout the message loop; handles are owned by this fixture process.
        unsafe {
            let module = GetModuleHandleW(std::ptr::null());
            let mut description: WNDCLASSW = std::mem::zeroed();
            description.lpfnWndProc = Some(window_proc);
            description.hInstance = module;
            description.lpszClassName = class.as_ptr();
            if RegisterClassW(&description) == 0 {
                return Err("Fixture class registration failed".into());
            }
            let window = CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                420,
                180,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                module,
                std::ptr::null(),
            );
            if window.is_null() {
                return Err("Fixture window creation failed".into());
            }
            let edit = CreateWindowExW(
                0,
                edit_class.as_ptr(),
                std::ptr::null(),
                WS_CHILD | WS_VISIBLE | ES_MULTILINE as u32,
                12,
                12,
                380,
                100,
                window,
                std::ptr::null_mut(),
                module,
                std::ptr::null(),
            );
            if edit.is_null() {
                DestroyWindow(window);
                return Err("Fixture editor creation failed".into());
            }
            EDIT.store(edit as isize, Ordering::Release);
            ShowWindow(window, SW_SHOW);
            let _ = SetForegroundWindow(window);
            SetFocus(edit);
            if SetTimer(window, 1, 100, None) == 0 {
                DestroyWindow(window);
                return Err("Fixture timer creation failed".into());
            }
            let mut message = std::mem::zeroed();
            while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        if PASSED.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err("Fixture did not receive the exact public paste text".into())
        }
    }

    pub fn run() -> Result<(), String> {
        if std::env::args().nth(1).as_deref() == Some("--target") {
            return target();
        }
        use siny::services::{deliver_dictation, read_clipboard, PasteTarget};
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut child = std::process::Command::new(exe)
            .arg("--target")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let result = (|| {
            let deadline = Instant::now() + Duration::from_secs(60);
            let target = loop {
                let mut process = 0;
                // Only the foreground PID is inspected; never read another app's title, content or clipboard.
                unsafe {
                    GetWindowThreadProcessId(GetForegroundWindow(), &mut process);
                }
                if process == child.id() {
                    if let Some(target) = PasteTarget::capture() {
                        break target;
                    }
                }
                if Instant::now() >= deadline
                    || child.try_wait().map_err(|e| e.to_string())?.is_some()
                {
                    return Err("The self-owned fixture window never became ready for input".into());
                }
                std::thread::sleep(Duration::from_millis(20));
            };
            if !deliver_dictation(TEXT, true, Some(target))?.contains("Copied and pasted") {
                return Err("Paste was not delivered to the fixture".into());
            }
            loop {
                if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                    if !status.success() {
                        return Err("Fixture text assertion failed".into());
                    }
                    break;
                }
                if Instant::now() >= deadline {
                    return Err("Fixture paste timed out".into());
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            let guard = "Siny public changed-destination guard.";
            if !deliver_dictation(guard, true, Some(target))?.contains("paste skipped")
                || read_clipboard()?.as_deref() != Some(guard)
            {
                return Err("Changed-destination guard failed".into());
            }
            let copy = "Siny public copy-only acceptance.";
            deliver_dictation(copy, false, None)?;
            if read_clipboard()?.as_deref() != Some(copy) {
                return Err("Copy-only assertion failed".into());
            }
            std::fs::create_dir_all("work/acceptance").map_err(|e| e.to_string())?;
            std::fs::write("work/acceptance/clipboard.toml", "auto_paste_delivery = true\nchanged_destination_skipped = true\ncopy_only = true\n").map_err(|e| e.to_string())?;
            println!("Auto-paste delivery to self-owned window, changed-destination guard and copy-only passed");
            Ok(())
        })();
        if child.try_wait().ok().flatten().is_none() {
            let _ = child.kill();
            let _ = child.wait();
        }
        result
    }
}
#[cfg(windows)]
fn main() -> Result<(), String> {
    windows_probe::run()
}
#[cfg(not(windows))]
fn main() {
    eprintln!("Interactive clipboard acceptance requires Windows");
    std::process::exit(1);
}
