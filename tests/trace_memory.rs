use nix::libc;
use std::process::Command;

#[test]
fn pipe_tracee() {
    if std::env::var_os("INTENTRACE_TEST_PIPE_TRACEE").is_none() {
        return;
    }

    let mut pipe_fds = [-1_i32; 2];
    let result = unsafe { libc::syscall(libc::SYS_pipe2, pipe_fds.as_mut_ptr(), libc::O_CLOEXEC) };
    assert_eq!(result, 0);
    for fd in pipe_fds {
        assert_eq!(unsafe { libc::close(fd) }, 0);
    }
}

#[test]
fn pipe2_displays_both_tracee_file_descriptors() {
    let output = Command::new(env!("CARGO_BIN_EXE_intentrace"))
        .args(["--trace", "pipe2", "-q", "-f"])
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", "pipe_tracee", "--nocapture"])
        .env("INTENTRACE_TEST_PIPE_TRACEE", "1")
        .output()
        .expect("failed to start intentrace");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "intentrace failed: {stderr}");
    assert!(stderr.contains("read end:"), "missing read end: {stderr}");
    assert!(stderr.contains("write end:"), "missing write end: {stderr}");
    assert_eq!(
        stderr.matches("Unix Pipe").count(),
        2,
        "expected both descriptors to resolve to the pipe: {stderr}"
    );
}
