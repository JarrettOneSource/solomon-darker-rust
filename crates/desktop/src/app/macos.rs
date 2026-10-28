//! Configure the native app menu and defer Cocoa Quit until the shared save owner decides.
//! Winit 0.30 retains its delegate; supply only its absent optional
//! applicationShouldTerminate method and remove the unused Services submenu.

use objc2::{
    MainThreadMarker,
    runtime::{AnyObject, Imp, Sel},
    sel,
};
use objc2_app_kit::{NSApplication, NSApplicationTerminateReply};
use std::sync::atomic::{AtomicBool, Ordering};

static INSTALLED: AtomicBool = AtomicBool::new(false);
static REQUESTED: AtomicBool = AtomicBool::new(false);
static WAITING: AtomicBool = AtomicBool::new(false);

extern "C-unwind" fn should_terminate(
    _: *mut AnyObject,
    _: Sel,
    _: *mut AnyObject,
) -> NSApplicationTerminateReply {
    WAITING.store(true, Ordering::Release);
    REQUESTED.store(true, Ordering::Release);
    NSApplicationTerminateReply::TerminateLater
}

pub(super) fn install() -> Result<(), String> {
    if INSTALLED.load(Ordering::Acquire) {
        return Ok(());
    }
    let main =
        MainThreadMarker::new().ok_or("The macOS application must run on its main thread")?;
    let application = NSApplication::sharedApplication(main);
    // Winit attaches Services under the application menu. This game has no
    // Services integration, and macOS can synchronously query Shortcuts while
    // routing ordinary keys through that menu, stalling gameplay input.
    if let Some(services) = application.servicesMenu() {
        application.setServicesMenu(None);
        if let Some(menu_bar) = application.mainMenu() {
            for item in menu_bar.itemArray() {
                if let Some(menu) = item.submenu() {
                    let index = menu.indexOfItemWithSubmenu(Some(&services));
                    if index >= 0 {
                        menu.removeItemAtIndex(index);
                    }
                }
            }
        }
    }
    let delegate = application.delegate().ok_or("The native window delegate is missing")?;
    let object: &AnyObject = (*delegate).as_ref();
    let class = object.class();
    let method = sel!(applicationShouldTerminate:);
    if class.instance_method(method).is_some() {
        return Err("The native window delegate already owns application termination".into());
    }
    // SAFETY: register the documented optional Cocoa delegate selector on the
    // actual winit delegate class. The callback has its exact ABI and touches only
    // atomics. Both supported macOS targets use NSUInteger's Q encoding. No winit
    // method or delegate state is replaced, and this class lives for the process.
    let installed = unsafe {
        let implementation: Imp = std::mem::transmute(
            should_terminate
                as extern "C-unwind" fn(
                    *mut AnyObject,
                    Sel,
                    *mut AnyObject,
                ) -> NSApplicationTerminateReply,
        );
        objc2::ffi::class_addMethod(
            std::ptr::from_ref(class).cast_mut(),
            method,
            implementation,
            c"Q@:@".as_ptr(),
        )
        .as_bool()
    };
    if !installed {
        return Err("Could not install save-before-quit handling".into());
    }
    INSTALLED.store(true, Ordering::Release);
    Ok(())
}

pub(super) fn requested() -> bool {
    REQUESTED.swap(false, Ordering::AcqRel)
}

pub(super) fn reply(quit: bool) {
    if WAITING.swap(false, Ordering::AcqRel) {
        let main = MainThreadMarker::new().expect("the native event loop runs on the main thread");
        NSApplication::sharedApplication(main).replyToApplicationShouldTerminate(quit);
    }
}
