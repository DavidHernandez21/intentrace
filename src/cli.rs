use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::{
        atomic::{AtomicBool, AtomicUsize},
        LazyLock,
    },
};

// TODO! Time blocks feature
// pub static TIME_BLOCKS: Cell<bool> = Cell::new(false);
pub static FOLLOW_FORKS: LazyLock<bool> = LazyLock::new(|| INTENTRACE_ARGS.follow_forks);
pub static STRING_LIMIT: AtomicUsize = AtomicUsize::new(36);
pub static FAILED_ONLY: LazyLock<bool> = LazyLock::new(|| INTENTRACE_ARGS.failed_only);
pub static QUIET: LazyLock<bool> = LazyLock::new(|| INTENTRACE_ARGS.mute_stdout);
pub static ANNOT: AtomicBool = AtomicBool::new(false);
pub static ATTACH_PID: LazyLock<Option<i32>> = LazyLock::new(|| INTENTRACE_ARGS.pid);
pub static SUMMARY_ONLY: LazyLock<bool> = LazyLock::new(|| INTENTRACE_ARGS.summary_only);
pub static SUMMARY: LazyLock<bool> = LazyLock::new(|| INTENTRACE_ARGS.summary);
pub static SYSCALLS_TO_TRACE: LazyLock<SysnoSet> = LazyLock::new(|| {
    if INTENTRACE_ARGS.trace.is_empty() {
        SysnoSet::all()
    } else {
        let mut sysno_set = SysnoSet::empty();
        for syscall in &INTENTRACE_ARGS.trace {
            if let Ok(sysno) = Sysno::from_str(syscall) {
                sysno_set.insert(sysno);
            } else {
                eprintln!("Invalid syscall: {syscall}");
                std::process::exit(100);
            }
        }
        sysno_set
    }
});
pub static OUTPUT_FILE: LazyLock<Option<&Path>> = LazyLock::new(|| {
    INTENTRACE_ARGS
        .file.as_deref()
});
//
pub static INTENTRACE_ARGS: LazyLock<IntentraceArgs> = LazyLock::new(IntentraceArgs::parse);
pub static BINARY_AND_ARGS: LazyLock<Option<&'static [String]>> =
    LazyLock::new(|| match INTENTRACE_ARGS.binary {
        Some(Binary::Command(ref regs)) if !regs.is_empty() => Some(regs),
        _ => None,
    });

use clap::{Parser, Subcommand};
use syscalls::{Sysno, SysnoSet};

#[derive(Parser)]
#[command(
    about = "intentrace is a strace for everyone.",
    version,
    allow_external_subcommands = true
)]
pub struct IntentraceArgs {
    /// print a summary table
    #[arg(short = 'c', long, conflicts_with = "summary")]
    pub summary_only: bool,

    /// print a summary table in addition to the normal output
    #[arg(short = 'C', long)]
    pub summary: bool,

    /// attach to an already running process by its positive PID
    #[arg(short = 'p', long = "attach", value_parser = clap::value_parser!(i32).range(1..))]
    pub pid: Option<i32>,

    /// redirect intentrace's output to a provided file
    #[arg(short = 'o', long = "output")]
    pub file: Option<PathBuf>,

    /// trace a specific syscall or a group of syscalls delimited by ','
    #[arg(long, value_delimiter = ',')]
    pub trace: Vec<String>,

    /// trace child processes when traced programs create them
    #[arg(
        short = 'f',
        long = "follow-forks",
        conflicts_with = "pid",
        conflicts_with = "failed_only"
    )]
    pub follow_forks: bool,

    /// only print failed syscalls
    #[arg(short = 'Z', long = "failed-only")]
    pub failed_only: bool,

    /// mute the traced program's std output
    #[arg(short = 'q', long = "mute-stdout")]
    pub mute_stdout: bool,

    #[command(subcommand)]
    pub binary: Option<Binary>,
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum Binary {
    #[command(external_subcommand)]
    Command(Vec<String>),
}

#[cfg(test)]
mod tests {
    use super::IntentraceArgs;
    use clap::Parser;

    #[test]
    fn attach_pid_must_be_positive_and_fit_in_i32() {
        for value in ["1", "2147483647"] {
            let args = IntentraceArgs::try_parse_from(["intentrace", "--attach", value])
                .expect("valid PID should be accepted");
            assert_eq!(args.pid, Some(value.parse().unwrap()));
        }

        for value in ["0", "-1", "2147483648"] {
            assert!(
                IntentraceArgs::try_parse_from(["intentrace", "--attach", value]).is_err(),
                "invalid PID {value} should be rejected"
            );
        }
    }
}
