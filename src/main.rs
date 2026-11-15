#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! CrosshairX-Open: a free, offline, open-source crosshair overlay for Windows.

mod cli;
mod overlay;
mod selftest;
mod startup;
mod ui;
mod win32;

fn main() {
    win32::set_dpi_aware();

    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        win32::attach_console();
        println!("CrosshairX-Open {}", crosshair_core::VERSION);
        return;
    }

    if let Some(index) = args.iter().position(|a| a == "--export-previews") {
        win32::attach_console();
        let dir = args
            .get(index + 1)
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("previews"));
        let failures = cli::export_previews(&dir);
        std::process::exit(if failures == 0 { 0 } else { 1 });
    }

    let snapshot = args
        .iter()
        .position(|a| a == "--ui-snapshot")
        .and_then(|index| args.get(index + 1))
        .map(std::path::PathBuf::from);
    let initial_page = args
        .iter()
        .position(|a| a == "--ui-page")
        .and_then(|index| args.get(index + 1))
        .and_then(|name| ui::Page::parse(name));

    if args.iter().any(|a| a == "--self-test") {
        win32::attach_console();
        let failures = selftest::run();
        std::process::exit(if failures == 0 { 0 } else { 1 });
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        win32::attach_console();
        println!("CrosshairX-Open {}", crosshair_core::VERSION);
        println!("  (no arguments)   open the editor and overlay");
        println!("  --self-test      run the built-in checks and exit");
        println!("  --version        print the version and exit");
        println!("  --export-previews <dir>  write the bundled presets as PNG files");
        println!("  --ui-snapshot <file>     open the editor, save a screenshot, exit");
        println!("  --ui-page <name>         designer|presets|position|settings|about");
        return;
    }

    // One instance per user session: two overlays would fight over hotkeys.
    let mutex_name = win32::wide("Local\\CrosshairX-Open-SingleInstance");
    let mutex = unsafe { win32::CreateMutexW(std::ptr::null(), 0, mutex_name.as_ptr()) };
    if mutex != 0 {
        let already_running =
            std::io::Error::last_os_error().raw_os_error() == Some(win32::ERROR_ALREADY_EXISTS as i32);
        if already_running {
            win32::attach_console();
            eprintln!("CrosshairX-Open is already running.");
            return;
        }
    }

    if let Err(error) = ui::run(snapshot, initial_page) {
        win32::attach_console();
        eprintln!("CrosshairX-Open could not start: {error}");
    }
}
