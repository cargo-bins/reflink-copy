use std::path::Path;

use cfg_if::cfg_if;

mod utility;

cfg_if! {
    if #[cfg(unix)] {
        mod unix;
        pub use self::unix::reflink;
    } else if #[cfg(windows)] {
        mod windows_impl;
        pub use self::windows_impl::reflink;
    } else {
        use self::reflink_not_supported as reflink;
    }
}

#[allow(dead_code)]
fn reflink_not_supported(_from: &Path, _to: &Path) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}
