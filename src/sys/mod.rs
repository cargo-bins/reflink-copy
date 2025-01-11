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
        pub use self::windows_impl::check_reflink_support;
        pub(crate) use self::windows_impl::reflink_block;
    } else {
        pub use self::reflink_not_supported as reflink;
    }
}

#[allow(dead_code)]
pub fn reflink_not_supported(_from: &Path, _to: &Path) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}
