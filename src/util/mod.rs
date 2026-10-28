pub mod cli;
pub mod error;
pub mod serde;
pub mod style;

use std::env;
use std::path::{Path, PathBuf};

/// The absolute path, without the \\?\ that Windows puts in front of it (it reads badly in
/// messages, and Java doesn't like it as a working directory).
pub fn canonicalize(path: &Path) -> Option<PathBuf> {
    let path = path.canonicalize().ok()?;
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => Some(PathBuf::from(rest)),
        _ => Some(path),
    }
}

/// Get the name of the executable that was invoked.
///
/// When a symbolic or hard link is used, the name of the link is returned.
///
/// This attempts to obtain the binary name in the following order:
/// - name in first item of program arguments via `std::env::args`
/// - current executable name via `std::env::current_exe`
/// - crate name
pub fn bin_name() -> String {
    env::args_os()
        .next()
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::current_exe().ok())
        .and_then(|p| p.file_name().map(|n| n.to_owned()))
        .and_then(|n| n.into_string().ok())
        .unwrap_or_else(|| crate_name!().into())
}
