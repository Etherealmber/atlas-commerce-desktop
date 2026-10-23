//! Build-time resources for the application binary.
//!
//! On Windows this embeds the multi-size `.ico` (Explorer, taskbar,
//! Alt+Tab) and the product description Task Manager displays, exactly
//! like Spotifast's build.rs. The icon lives at the repository root in
//! `packaging/windows/`, two directories above this crate.

fn main() {
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let icon = "../../packaging/windows/atlas-commerce-desktop.ico";
        println!("cargo:rerun-if-changed={icon}");
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon(icon)
            .set("ProductName", "Atlas Commerce Desktop")
            // Task Manager and the volume mixer name the process by this.
            .set("FileDescription", "Atlas Commerce Desktop");
        if let Err(error) = resource.compile() {
            println!("cargo:warning=Windows resources not embedded: {error}");
        }
    }
}
