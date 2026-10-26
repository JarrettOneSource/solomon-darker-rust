// Run on the process's main thread: libtest worker teardown does not exercise
// Windows process-exit ordering for controller services held in thread locals.
#[cfg(target_os = "windows")]
fn main() {
    use solomon_web::platform::{ClientTasks, gamepads};

    for _ in 0..2 {
        let client = ClientTasks::install();
        gamepads().expect("native controller enumeration");
        drop(client);
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {}
