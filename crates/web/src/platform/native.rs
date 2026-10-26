use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::{Rc, Weak};
use std::sync::OnceLock;
use std::task::{Poll, Waker};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use async_executor::LocalExecutor;

thread_local! {
    static LOCATION: RefCell<Option<(url::Url, Option<crate::hosted::WebsiteMount>)>> = const { RefCell::new(None) };
    static EXECUTOR: RefCell<Weak<LocalExecutor<'static>>> = const { RefCell::new(Weak::new()) };
    static COARSE_POINTER: Cell<bool> = const { Cell::new(false) };
    static NEXT_FRAME: RefCell<Vec<Waker>> = const { RefCell::new(Vec::new()) };
    static FRAME: Cell<u64> = const { Cell::new(0) };
    static CLIPBOARD: RefCell<Option<arboard::Clipboard>> = const { RefCell::new(None) };
    static GAMEPADS: RefCell<Option<Result<gilrs::Gilrs, String>>> = const { RefCell::new(None) };
}

pub fn gamepads() -> Result<Vec<solomon_sim::input::StandardGamepadSnapshot>, String> {
    use gilrs::{Axis, Button, Filter};
    GAMEPADS.with_borrow_mut(|current| {
        let pads = current
            .get_or_insert_with(|| {
                gilrs::GilrsBuilder::new()
                    .with_default_filters(false)
                    .set_update_state(false)
                    .with_force_feedback(false)
                    .build()
                    .map_err(|error| format!("Game controllers: {error}"))
            })
            .as_mut()
            .map_err(|error| error.clone())?;
        // The simulation owns dead zones; retain only the platform D-pad normalization.
        while let Some(event) =
            pads.next_event().filter_ev(&gilrs::ev::filter::axis_dpad_to_button, pads)
        {
            pads.update(&event);
        }
        Ok(pads
            .gamepads()
            .map(|(id, pad)| {
                let buttons = [
                    Button::South,
                    Button::East,
                    Button::West,
                    Button::North,
                    Button::LeftTrigger,
                    Button::RightTrigger,
                    Button::LeftTrigger2,
                    Button::RightTrigger2,
                    Button::Select,
                    Button::Start,
                    Button::LeftThumb,
                    Button::RightThumb,
                    Button::DPadUp,
                    Button::DPadDown,
                    Button::DPadLeft,
                    Button::DPadRight,
                ]
                .map(|button| {
                    pad.is_pressed(button)
                        || pad.button_data(button).is_some_and(|data| data.value() > 0.5)
                });
                solomon_sim::input::StandardGamepadSnapshot {
                    index: usize::from(id) as u32,
                    connected: pad.is_connected(),
                    standard: pad.mapping_source() != gilrs::MappingSource::None,
                    left: solomon_sim::Vec2::new(
                        f64::from(pad.value(Axis::LeftStickX)),
                        -f64::from(pad.value(Axis::LeftStickY)),
                    ),
                    right: solomon_sim::Vec2::new(
                        f64::from(pad.value(Axis::RightStickX)),
                        -f64::from(pad.value(Axis::RightStickY)),
                    ),
                    buttons,
                }
            })
            .collect())
    })
}

/// The native event loop owns this executor. Dropping it cancels all remaining
/// client futures, releasing their game/GPU owners before the GL window closes.
pub struct ClientTasks(Rc<LocalExecutor<'static>>);

impl ClientTasks {
    pub fn install() -> Self {
        let executor = Rc::new(LocalExecutor::new());
        EXECUTOR.with(|current| {
            assert!(current.borrow().upgrade().is_none(), "one native client executor per thread");
            *current.borrow_mut() = Rc::downgrade(&executor);
        });
        Self(executor)
    }

    pub fn tick(&self) {
        FRAME.set(FRAME.get().wrapping_add(1));
        for waker in NEXT_FRAME.take() {
            waker.wake();
        }
        while self.0.try_tick() {}
    }
}

impl Drop for ClientTasks {
    fn drop(&mut self) {
        NEXT_FRAME.with_borrow_mut(Vec::clear);
        CLIPBOARD.with_borrow_mut(|clipboard| *clipboard = None);
        EXECUTOR.with_borrow_mut(|executor| *executor = Weak::new());
    }
}

/// Continue tiled loading at the next event-loop frame, keeping the window responsive.
pub async fn next_frame() -> Result<(), String> {
    let frame = FRAME.get();
    let mut waiting = false;
    std::future::poll_fn(move |context| {
        if FRAME.get() != frame {
            return Poll::Ready(Ok(()));
        }
        if !waiting {
            waiting = true;
            NEXT_FRAME.with_borrow_mut(|waiters| waiters.push(context.waker().clone()));
        }
        Poll::Pending
    })
    .await
}

pub fn spawn_local(future: impl Future<Output = ()> + 'static) {
    EXECUTOR.with(|executor| {
        executor
            .borrow()
            .upgrade()
            .expect("native client executor is installed")
            .spawn(future)
            .detach();
    });
}

pub fn epoch_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).expect("clock after UNIX epoch").as_secs_f64()
        * 1000.0
}

pub fn monotonic_ms() -> f64 {
    static STARTED: OnceLock<Instant> = OnceLock::new();
    STARTED.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

pub fn iso_now() -> String {
    time::OffsetDateTime::now_utc()
        .format(time::macros::format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
        ))
        .expect("UTC timestamp format")
}

pub fn random_unit() -> f64 {
    fastrand::f64()
}

pub fn random_bytes(bytes: &mut [u8]) -> Result<(), String> {
    getrandom::fill(bytes).map_err(|error| format!("random source: {error}"))
}

pub fn error(message: &str) {
    eprintln!("{message}");
}

pub fn warn(message: &str) {
    eprintln!("{message}");
}

pub fn coarse_pointer() -> bool {
    COARSE_POINTER.get()
}

/// winit reports actual touch and mouse input to the same client owner.
pub fn set_coarse_pointer(coarse: bool) {
    COARSE_POINTER.set(coarse);
}

pub fn reduced_motion() -> bool {
    #[cfg(target_os = "macos")]
    {
        objc2_app_kit::NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
    }
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW,
        };
        let mut animate = 1_i32;
        // SAFETY: this SPI action writes one BOOL into the live stack variable.
        let read = unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                (&mut animate as *mut i32).cast(),
                0,
            )
        };
        read != 0 && animate == 0
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        static REDUCED: OnceLock<bool> = OnceLock::new();
        *REDUCED.get_or_init(|| {
            std::process::Command::new("gsettings")
                .args(["get", "org.gnome.desktop.interface", "enable-animations"])
                .output()
                .is_ok_and(|result| result.status.success() && result.stdout == b"false\n")
        })
    }
}

// Linux serves clipboard contents from their owning client; retain this owner for
// the whole window lifetime and share it with native text edits and menu copies.
fn with_clipboard<T>(
    operation: impl FnOnce(&mut arboard::Clipboard) -> Result<T, arboard::Error>,
) -> Result<T, String> {
    CLIPBOARD.with_borrow_mut(|current| {
        if current.is_none() {
            *current =
                Some(arboard::Clipboard::new().map_err(|error| format!("Clipboard: {error}"))?);
        }
        operation(current.as_mut().expect("clipboard initialized"))
            .map_err(|error| format!("Clipboard: {error}"))
    })
}

pub(crate) fn clipboard_text() -> Result<String, String> {
    with_clipboard(|clipboard| clipboard.get_text())
}

pub(crate) fn set_clipboard_text(text: &str) -> Result<(), String> {
    with_clipboard(|clipboard| clipboard.set_text(text))
}

pub fn copy_text(
    text: &str,
) -> Result<futures_util::future::LocalBoxFuture<'static, Result<(), String>>, String> {
    set_clipboard_text(text)?;
    Ok(Box::pin(async { Ok(()) }))
}

/// The native app supplies its actual local authority URL and Website mount.
pub fn configure_native_location(
    base_url: &str,
    mount: Option<crate::hosted::WebsiteMount>,
) -> Result<(), String> {
    let url = url::Url::parse(base_url).map_err(|error| format!("Invalid game URL: {error}"))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Invalid game URL".into());
    }
    LOCATION.with_borrow_mut(|location| *location = Some((url, mount)));
    Ok(())
}

pub fn location() -> Result<url::Url, String> {
    LOCATION.with_borrow(|location| {
        location
            .as_ref()
            .map(|(url, _)| url.clone())
            .ok_or_else(|| "The native game URL is not configured".into())
    })
}

pub fn website_mount() -> Result<Option<crate::hosted::WebsiteMount>, String> {
    LOCATION.with_borrow(|location| {
        location
            .as_ref()
            .map(|(_, mount)| mount.clone())
            .ok_or_else(|| "The native game URL is not configured".into())
    })
}

pub(super) fn replace_location(url: &url::Url) -> Result<(), String> {
    LOCATION.with_borrow_mut(|location| {
        let (current, _) = location.as_mut().ok_or("The native game URL is not configured")?;
        if current.origin() != url.origin() {
            return Err("The replacement URL has another origin".into());
        }
        *current = url.clone();
        Ok(())
    })
}

pub fn navigate(url: &str) -> Result<(), String> {
    let url = location()?.join(url).map_err(|_| "Invalid navigation URL")?;
    crate::desktop::request(crate::desktop::launcher::Action::OpenUrl(url.into()))
}

pub fn reload() -> Result<(), String> {
    crate::desktop::request(crate::desktop::launcher::Action::ReloadGame)
}

/// The native application retains the installed desktop origin admitted by the
/// authority and encrypted peer bridge. Its file/HTTP base is configured separately.
pub const NATIVE_ORIGIN: &str = "solomon-darker://app";

pub fn network_origin() -> Result<String, String> {
    Ok(NATIVE_ORIGIN.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_loading_yields_across_frames_and_shutdown_drops_pending_owners() {
        let tasks = ClientTasks::install();
        let frames = Rc::new(Cell::new(0));
        let destroyed = Rc::new(Cell::new(false));
        struct Owner(Rc<Cell<bool>>);
        impl Drop for Owner {
            fn drop(&mut self) {
                self.0.set(true);
            }
        }
        let owner = Owner(destroyed.clone());
        let observed = frames.clone();
        spawn_local(async move {
            let _owner = owner;
            for _ in 0..3 {
                next_frame().await.unwrap();
                observed.set(observed.get() + 1);
            }
        });
        tasks.tick();
        assert_eq!(frames.get(), 0);
        tasks.tick();
        assert_eq!(frames.get(), 1);
        tasks.tick();
        assert_eq!(frames.get(), 2);
        assert!(!destroyed.get());
        drop(tasks);
        assert!(destroyed.get(), "pending graphics owners retire before the native window");
        let next = ClientTasks::install();
        next.tick();
        assert_eq!(
            frames.get(),
            2,
            "a replacement window cannot resume the cancelled loading task"
        );
    }
}
