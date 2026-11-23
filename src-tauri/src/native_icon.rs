//! Set the Dock icon from embedded bytes, including unbundled development launches.
#[cfg(target_os = "macos")]
pub fn install() -> Result<(), &'static str> {
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;
    let main = MainThreadMarker::new().ok_or("Dock icon must be set on the main thread")?;
    let data = NSData::with_bytes(include_bytes!("../icons/icon.icns"));
    let icon = NSImage::initWithData(NSImage::alloc(), &data).ok_or("Invalid application icon")?;
    if !icon.isValid() { return Err("Application icon could not be decoded"); }
    // AppKit retains the image; setup runs on the application's main thread.
    unsafe { NSApplication::sharedApplication(main).setApplicationIconImage(Some(&icon)); }
    Ok(())
}
