//! The zip CargoFrame keeps when a share cannot be delivered, and sends where a service takes
//! a file: the screenshot, the text the person wrote, the files they ticked, and a manifest.
//!
//! * [`entry`]: names inside the zip, made safe and unique.
//! * [`build`]: write the zip (streaming, with a checksum for every file) and its manifest.
//! * [`save`]: copy a finished zip into a folder atomically, for the fallback ladder.
//!
//! Only what the person chose goes in. Settings, connections and secrets are never read here.

pub mod build;
pub mod entry;
pub mod save;

pub use build::{
    build_zip, BundleError, Created, Item, Limits, Manifest, ManifestFile, ManifestInfo, Source,
};
pub use save::copy_atomic;
