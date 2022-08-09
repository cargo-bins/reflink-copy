use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(unix)] {
        mod unix;
        pub use unix::reflink;
    } else if #[cfg(windows)] {
        mod windows;
        pub use windows::reflink;
    } else {
        mod others;
        pub use others::reflink;
    }
}

#[allow(dead_code)]
fn reflink_not_supported() -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}
