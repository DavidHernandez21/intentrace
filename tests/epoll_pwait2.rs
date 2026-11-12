use nix::libc;
use std::{process::Command, ptr};

#[test]
fn epoll_tracee() {
    if std::env::var_os("INTENTRACE_TEST_EPOLL_TRACEE").is_none() {
        return;
    }

    let timeout = libc::timespec {
        tv_sec: 0,
        tv_nsec: 987_654_321,
    };
    assert!((&raw const timeout as usize) > i32::MAX as usize);

    unsafe {
        libc::syscall(
            libc::SYS_epoll_pwait2,
            -1,
            ptr::null_mut::<libc::epoll_event>(),
            1,
            &raw const timeout,
            ptr::null::<libc::sigset_t>(),
            size_of::<libc::sigset_t>(),
        );
    }
}

#[test]
fn epoll_timeout_uses_the_full_pointer() {
    let output = Command::new(env!("CARGO_BIN_EXE_intentrace"))
        .args(["--trace", "epoll_pwait2", "-q", "-f"])
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", "epoll_tracee", "--nocapture"])
        .env("INTENTRACE_TEST_EPOLL_TRACEE", "1")
        .output()
        .expect("failed to start intentrace");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "intentrace failed: {stderr}");
    assert!(
        stderr.contains("987,654,321"),
        "timeout was not decoded: {stderr}"
    );
    assert!(
        !stderr.contains("could not read timeout"),
        "timeout pointer could not be read: {stderr}"
    );
}
