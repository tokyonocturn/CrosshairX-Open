//! Small command-line helpers: render the bundled presets to PNG files.

use std::path::{Path, PathBuf};

use crosshair_core::{preset, render, store, RenderOptions};

/// `--export-previews <dir>`: write every bundled preset as a PNG at 6x so the
/// shapes can be inspected outside the app.
pub fn export_previews(dir: &Path) -> i32 {
    let mut failures = 0;
    for design in store::builtin_presets() {
        let options = RenderOptions {
            scale: 6.0,
            opacity: 1.0,
            padding: 6.0,
        };
        let Some(rendered) = render(&design, &options) else {
            failures += 1;
            println!("[FAIL] {} could not render", design.name);
            continue;
        };
        let file: PathBuf =
            dir.join(format!("{}.png", preset::file_stem(&design.name).to_lowercase()));
        match crosshair_core::save_rgba_png(
            &file,
            rendered.width(),
            rendered.height(),
            &rendered.to_unmultiplied_rgba(),
        ) {
            Ok(()) => println!(
                "{} ({}x{})",
                file.display(),
                rendered.width(),
                rendered.height()
            ),
            Err(error) => {
                failures += 1;
                println!("[FAIL] {error}");
            }
        }
    }
    failures
}
