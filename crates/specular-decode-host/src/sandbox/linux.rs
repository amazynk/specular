//! Linux confinement: no-new-privs, Landlock (no filesystem or network access at all, signals
//! and abstract sockets scoped), then a seccomp-bpf allowlist.
//!
//! What remains possible: computing, allocating (bounded by `RLIMIT_AS`), creating threads,
//! talking over the descriptors already open (the socket to the viewer and the input files it
//! sends, which UnRAR reads with `pread` through Specular's handle-I/O patch), and creating
//! anonymous shared memory for results. `openat` is not allowed at all (so not even
//! `/proc/self/fd`, which Landlock would let through), and Landlock denies every filesystem
//! right besides. Opening any file, creating sockets, running programs, signalling other
//! processes and mapping executable memory are denied.

use super::Status;
use seccompiler::{
    BpfProgram, SeccompAction, SeccompCmpArgLen as Len, SeccompCmpOp as Op, SeccompCondition,
    SeccompFilter, SeccompRule, TargetArch,
};
use specular_sandbox::protocol::SandboxLevel;
use std::collections::BTreeMap;

pub fn apply() -> Status {
    let mut detail = Vec::new();
    let nnp = rustix::thread::set_no_new_privs(true).is_ok();
    if !nnp {
        detail.push("no_new_privs failed".to_string());
    }
    let landlock = landlock();
    match &landlock {
        Ok(s) => detail.push(s.clone()),
        Err(e) => detail.push(format!("landlock: {e}")),
    }
    let seccomp = seccomp();
    match &seccomp {
        Ok(()) => detail.push("seccomp".into()),
        Err(e) => detail.push(format!("seccomp failed: {e}")),
    }
    let level = match (landlock.as_deref(), seccomp.is_ok()) {
        (Ok("landlock"), true) if nnp => SandboxLevel::Full,
        (_, true) => SandboxLevel::Partial,
        (Ok(_), false) => SandboxLevel::Partial,
        (Err(_), false) => SandboxLevel::None,
    };
    Status {
        level,
        detail: detail.join(", "),
    }
}

/// Handles every filesystem and network right and every scope, grants none.
fn landlock() -> Result<String, String> {
    use landlock::{ABI, Access, AccessFs, AccessNet, RulesetAttr, RulesetStatus, Scope};
    let abi = ABI::V9;
    let status = landlock::Ruleset::default()
        .handle_access(AccessFs::from_all(abi))
        .and_then(|r| r.handle_access(AccessNet::from_all(abi)))
        .and_then(|r| r.scope(Scope::from_all(abi)))
        .and_then(|r| r.create())
        .and_then(|r| r.restrict_self())
        .map_err(|e| e.to_string())?;
    match status.ruleset {
        RulesetStatus::FullyEnforced => Ok("landlock".into()),
        RulesetStatus::PartiallyEnforced => Ok(format!("landlock partial ({:?})", status.landlock)),
        RulesetStatus::NotEnforced => Err(format!("not enforced ({:?})", status.landlock)),
    }
}

fn arch() -> Result<TargetArch, String> {
    if cfg!(target_arch = "x86_64") {
        Ok(TargetArch::x86_64)
    } else if cfg!(target_arch = "aarch64") {
        Ok(TargetArch::aarch64)
    } else if cfg!(target_arch = "riscv64") {
        Ok(TargetArch::riscv64)
    } else {
        Err("unsupported architecture".into())
    }
}

fn cond(arg: u8, len: Len, op: Op, value: u64) -> Result<SeccompCondition, String> {
    SeccompCondition::new(arg, len, op, value).map_err(|e| e.to_string())
}

fn rule(conds: Vec<SeccompCondition>) -> Result<SeccompRule, String> {
    SeccompRule::new(conds).map_err(|e| e.to_string())
}

/// Syscalls allowed whatever their arguments.
const ALWAYS: &[i64] = &[
    libc::SYS_read,
    libc::SYS_readv,
    libc::SYS_pread64,
    libc::SYS_write,
    libc::SYS_writev,
    libc::SYS_pwrite64,
    libc::SYS_lseek,
    libc::SYS_close,
    libc::SYS_fstat,
    libc::SYS_newfstatat,
    libc::SYS_statx,
    libc::SYS_ftruncate,
    libc::SYS_fcntl,
    libc::SYS_dup3,
    libc::SYS_memfd_create,
    libc::SYS_recvmsg,
    libc::SYS_sendmsg,
    libc::SYS_munmap,
    libc::SYS_mremap,
    libc::SYS_madvise,
    libc::SYS_brk,
    libc::SYS_futex,
    libc::SYS_set_robust_list,
    libc::SYS_get_robust_list,
    libc::SYS_rseq,
    libc::SYS_set_tid_address,
    libc::SYS_sched_yield,
    libc::SYS_sched_getaffinity,
    libc::SYS_getrandom,
    libc::SYS_clock_gettime,
    libc::SYS_clock_getres,
    libc::SYS_clock_nanosleep,
    libc::SYS_nanosleep,
    libc::SYS_gettimeofday,
    libc::SYS_getpid,
    libc::SYS_gettid,
    libc::SYS_getppid,
    libc::SYS_getuid,
    libc::SYS_geteuid,
    libc::SYS_getgid,
    libc::SYS_getegid,
    libc::SYS_exit,
    libc::SYS_exit_group,
    libc::SYS_rt_sigaction,
    libc::SYS_rt_sigprocmask,
    libc::SYS_rt_sigreturn,
    libc::SYS_sigaltstack,
    libc::SYS_restart_syscall,
    libc::SYS_ppoll,
    libc::SYS_membarrier,
    libc::SYS_sysinfo,
    libc::SYS_uname,
];

#[cfg(target_arch = "x86_64")]
const ALWAYS_ARCH: &[i64] = &[libc::SYS_poll, libc::SYS_dup, libc::SYS_time];
#[cfg(not(target_arch = "x86_64"))]
const ALWAYS_ARCH: &[i64] = &[libc::SYS_dup];

fn seccomp() -> Result<(), String> {
    let arch = arch()?;
    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
    for &s in ALWAYS.iter().chain(ALWAYS_ARCH) {
        rules.insert(s, Vec::new());
    }
    let prot_exec = libc::PROT_EXEC as u64;
    // Memory, never executable (no code is loaded after start-up).
    for s in [libc::SYS_mmap, libc::SYS_mprotect] {
        rules.insert(
            s,
            vec![rule(vec![cond(
                2,
                Len::Dword,
                Op::MaskedEq(prot_exec),
                0,
            )?])?],
        );
    }
    // Threads only, no processes (glibc tries clone3 first and falls back on ENOSYS).
    let thread = libc::CLONE_THREAD as u64;
    rules.insert(
        libc::SYS_clone,
        vec![rule(vec![cond(
            0,
            Len::Qword,
            Op::MaskedEq(thread),
            thread,
        )?])?],
    );
    // No `openat`: every file the host reads arrives as a descriptor.
    // Signals only to itself (abort).
    let pid = u64::from(std::process::id());
    rules.insert(
        libc::SYS_tgkill,
        vec![rule(vec![cond(0, Len::Dword, Op::Eq, pid)?])?],
    );
    // Reading its own limits, not changing them.
    rules.insert(
        libc::SYS_prlimit64,
        vec![rule(vec![
            cond(0, Len::Dword, Op::Eq, 0)?,
            cond(2, Len::Qword, Op::Eq, 0)?,
        ])?],
    );
    // Naming threads.
    rules.insert(
        libc::SYS_prctl,
        vec![
            rule(vec![cond(0, Len::Dword, Op::Eq, libc::PR_SET_NAME as u64)?])?,
            rule(vec![cond(0, Len::Dword, Op::Eq, libc::PR_GET_NAME as u64)?])?,
        ],
    );
    // Writes on the connected socket (no destination address).
    rules.insert(
        libc::SYS_sendto,
        vec![rule(vec![cond(4, Len::Qword, Op::Eq, 0)?])?],
    );
    rules.insert(
        libc::SYS_recvfrom,
        vec![rule(vec![cond(4, Len::Qword, Op::Eq, 0)?])?],
    );
    let filter = SeccompFilter::new(
        rules,
        // Everything else fails with ENOSYS ("not implemented"), which C libraries treat as
        // a plain error (and glibc as "use the older call", e.g. clone3 → clone).
        SeccompAction::Errno(libc::ENOSYS as u32),
        SeccompAction::Allow,
        arch,
    )
    .map_err(|e| e.to_string())?;
    let prog: BpfProgram = filter
        .try_into()
        .map_err(|e: seccompiler::BackendError| e.to_string())?;
    seccompiler::apply_filter_all_threads(&prog).map_err(|e| e.to_string())
}
