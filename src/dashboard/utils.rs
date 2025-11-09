/// Helper function to suppress stdout/stderr during operation
/// This prevents CLI output from glitching behind the TUI
pub fn suppress_output<F, T>(f: F) -> T
where
    F: FnOnce() -> T,
{
    use std::fs::OpenOptions;
    use std::os::unix::io::AsRawFd;

    let original_stdout = unsafe { libc::dup(1) };
    let original_stderr = unsafe { libc::dup(2) };

    // Redirect to /dev/null
    let devnull = OpenOptions::new()
        .write(true)
        .open("/dev/null")
        .unwrap();
    let devnull_fd = devnull.as_raw_fd();

    unsafe {
        libc::dup2(devnull_fd, 1);
        libc::dup2(devnull_fd, 2);
    }

    // Run the function
    let result = f();

    // Restore stdout/stderr
    unsafe {
        libc::dup2(original_stdout, 1);
        libc::dup2(original_stderr, 2);
        libc::close(original_stdout);
        libc::close(original_stderr);
    }

    result
}
