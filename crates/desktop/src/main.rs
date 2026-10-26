#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let result = solomon_desktop::config::Config::load(std::env::args_os().skip(1))
        .and_then(solomon_desktop::app::run);
    if let Err(error) = result {
        eprintln!("Solomon Darker: {error}");
        rfd::MessageDialog::new()
            .set_title("Solomon Darker could not start")
            .set_description(&error)
            .set_level(rfd::MessageLevel::Error)
            .show();
        std::process::exit(1);
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}
