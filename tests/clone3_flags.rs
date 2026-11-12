use nix::libc;
use std::process::Command;

#[test]
fn clone3_tracee() {
    if std::env::var_os("INTENTRACE_TEST_CLONE3_TRACEE").is_none() {
        return;
    }

    for flags in [0, 0x1_0000_0000, 0x1_0000_0000 | libc::CLONE_FILES as u64] {
        let args = libc::clone_args {
            flags,
            // An invalid exit signal prevents the test from creating child processes.
            exit_signal: 255,
            ..unsafe { std::mem::zeroed() }
        };
        unsafe {
            libc::syscall(libc::SYS_clone3, &raw const args, size_of::<libc::clone_args>());
        }
    }
}

#[test]
fn clone3_clear_sighand_is_decoded_as_a_64_bit_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_intentrace"))
        .args(["--trace", "clone3", "-q", "-f"])
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", "clone3_tracee", "--nocapture"])
        .env("INTENTRACE_TEST_CLONE3_TRACEE", "1")
        .output()
        .expect("failed to start intentrace");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "intentrace failed: {stderr}");
    let calls: Vec<_> = stderr
        .lines()
        .filter(|line| line.contains("spawn a new child process"))
        .collect();
    assert_eq!(
        calls.len(),
        3,
        "expected three clone3 calls: {stderr}"
    );
    assert!(!calls[0].contains("default all inherited signal handlers"));
    assert!(calls[1].contains("default all inherited signal handlers"));
    assert!(calls[2].contains("default all inherited signal handlers"));
    assert!(
        calls[2].contains("the file descriptor table"),
        "combined lower-bit flag was not decoded: {stderr}"
    );
}
