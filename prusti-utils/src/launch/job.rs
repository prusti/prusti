// Copied and modified from cargo/rustup
// https://github.com/rust-lang/rustup/blob/affbdfe0a37d92444e7c3c111043cd336e2f7728/src/cli/job.rs

// Copyright Diggory Blake, the Mozilla Corporation, and Rustup contributors.
//
// Licensed under either of
//
// Apache License, Version 2.0, (LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0)
// MIT license (LICENSE-MIT or https://opensource.org/licenses/MIT)
// at your option.

//! # Job management on Windows:
//!
//! Most of the time when you're running cargo you expect Ctrl-C to actually
//! terminate the entire tree of processes in play, not just the one at the top
//! (cargo). This currently works "by default" on Unix platforms because Ctrl-C
//! actually sends a signal to the *process group* rather than the parent
//! process, so everything will get torn down. On Windows, however, this does
//! not happen and Ctrl-C just kills cargo.
//!
//! To achieve the same semantics on Windows we use Job Objects to ensure that
//! all processes die at the same time. Job objects have a mode of operation
//! where when all handles to the object are closed it causes all child
//! processes associated with the object to be terminated immediately.
//! Conveniently whenever a process in the job object spawns a new process the
//! child will be associated with the job object as well. This means if we add
//! ourselves to the job object we create then everything will get torn down!
//!
//! # Job management on Unix:
//!
//! A launcher and everything it starts stay in the caller's process group, so
//! signals that the terminal, a shell or `timeout` send to that group reach
//! all of them, and the terminal treats them as one job. A SIGINT, SIGTERM or
//! SIGHUP sent to the launcher alone is passed on to every process still among
//! its descendants, found in the process table, so `kill` stops the whole
//! tree but no other member of the group, such as the other commands of a
//! pipeline. The launcher passes on the signal it received itself, so that
//! launchers among its descendants pass it on in turn and the drivers' JVMs
//! shut down cleanly.
//! The launcher then terminates with that signal, once the child it runs with
//! `status` has exited, or at once if there is no such child (e.g. during the
//! `java` and `rustup` queries that precede it). If one of these signals or
//! SIGQUIT terminated the child, the launcher terminates with it too. While
//! that child runs, the launcher ignores SIGQUIT, which Ctrl-\ sends to the
//! child too: the driver's JVM then prints a thread dump and continues.
//! A signal that is ignored when the launcher starts (e.g. SIGHUP under
//! `nohup`) stays ignored. SIGKILL cannot be handled, so sent to the launcher
//! alone, it leaves the processes the launcher started running.

#![allow(clippy::missing_safety_doc)]

use std::{
    io,
    process::{Command, ExitStatus},
};

pub use self::imp::Setup;

pub fn setup() -> Option<Setup> {
    unsafe { imp::setup() }
}

/// Runs `cmd` like `Command::status`. On Unix, a SIGINT, SIGTERM or SIGHUP
/// that arrives while the child runs terminates the launcher only after the
/// child has exited, and so does one of these signals or SIGQUIT that
/// terminated the child. Only one call may run at a time.
pub fn status(cmd: &mut Command) -> io::Result<ExitStatus> {
    imp::status(cmd)
}

#[cfg(unix)]
mod imp {
    use std::{
        io, mem,
        os::unix::process::ExitStatusExt,
        process::{self, Command, ExitStatus},
        ptr,
        sync::{Mutex, MutexGuard, PoisonError},
        thread,
    };

    use nix::libc;
    use rustc_hash::{FxHashMap, FxHashSet};
    use signal_hook::iterator::Signals;

    pub type Setup = bool;

    struct State {
        /// The SIGINT, SIGTERM or SIGHUP received while `status` waits.
        received: Option<libc::c_int>,
        /// The child that `status` waits for. While there is one, `status`,
        /// not the signal thread, terminates the launcher.
        child: Option<libc::pid_t>,
    }

    /// The signal thread holds the lock while handling a signal, and `status`
    /// while spawning, so a signal either reaches the new child or terminates
    /// the launcher before it is spawned.
    static STATE: Mutex<State> = Mutex::new(State {
        received: None,
        child: None,
    });

    fn state() -> MutexGuard<'static, State> {
        STATE.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Terminates the launcher by the default action of `signal`, so that its
    /// caller sees which signal ended it.
    fn terminate(signal: libc::c_int) -> ! {
        unsafe { libc::signal(signal, libc::SIG_DFL) };
        let _ = signal_hook::low_level::raise(signal);
        // The default action does not terminate e.g. the init process of a pid namespace.
        process::exit(128 + signal)
    }

    fn is_ignored(signal: libc::c_int) -> bool {
        let mut action: libc::sigaction = unsafe { mem::zeroed() };
        // With a null new action, `sigaction` only reads the current one.
        let read = unsafe { libc::sigaction(signal, ptr::null(), &mut action) };
        read == 0 && action.sa_sigaction == libc::SIG_IGN
    }

    /// The signals that the launcher handles, unless they are ignored at start.
    const HANDLED: [libc::c_int; 4] = [libc::SIGINT, libc::SIGTERM, libc::SIGHUP, libc::SIGQUIT];

    pub(super) unsafe fn setup() -> Option<Setup> {
        let signals = HANDLED.into_iter().filter(|&signal| !is_ignored(signal));
        // signal-hook's handler only wakes the thread spawned below, so that thread may allocate
        // and read the process table.
        let mut signals = Signals::new(signals).expect("Error setting signal handlers");
        thread::spawn(move || {
            for signal in signals.forever() {
                let mut state = state();
                if signal == libc::SIGQUIT {
                    // Ctrl-\ also reaches the child, whose JVM prints a thread dump and
                    // continues, so the launcher keeps waiting for it.
                    if state.child.is_none() {
                        terminate(signal);
                    }
                    continue;
                }
                signal_descendants(signal, state.child);
                if state.child.is_none() {
                    terminate(signal);
                }
                state.received = Some(signal);
            }
        });
        Some(false)
    }

    pub(super) fn status(cmd: &mut Command) -> io::Result<ExitStatus> {
        let mut child = {
            let mut state = state();
            let child = cmd.spawn()?;
            state.child = Some(child.id() as libc::pid_t);
            child
        };
        let status = child.wait();
        let mut state = state();
        state.child = None;
        // A signal sent to the whole group, such as Ctrl-C, can terminate the child before the
        // signal thread has handled it, so a child terminated by a handled signal counts too. A
        // child that catches the signal and exits with a status instead, like the JVM, is not
        // recognised this way, but it normally exits only after the signal thread has recorded
        // the signal.
        let child_signal = status.as_ref().ok().and_then(|status| status.signal());
        if let Some(signal) = state
            .received
            .or(child_signal.filter(|signal| HANDLED.contains(signal)))
        {
            terminate(signal);
        }
        status
    }

    /// Sends `signal` to `child`, if any, and every other descendant of this
    /// process, parents first. The descendants are looked up before any of them
    /// is signalled, since the children of a process that terminates are
    /// reparented and so leave the tree. `child` is signalled even if the
    /// process table cannot be read. A second lookup catches processes that the
    /// recipients started meanwhile.
    fn signal_descendants(signal: libc::c_int, child: Option<libc::pid_t>) {
        let root = process::id() as libc::pid_t;
        let descendants_before = descendants(root);
        let mut signalled = FxHashSet::default();
        let mut send = |pid| {
            if signalled.insert(pid) {
                unsafe { libc::kill(pid, signal) };
            }
        };
        child.into_iter().for_each(&mut send);
        descendants_before.into_iter().for_each(&mut send);
        descendants(root).into_iter().for_each(&mut send);
    }

    /// The descendants of `root` in the process table, in breadth-first order.
    fn descendants(root: libc::pid_t) -> Vec<libc::pid_t> {
        let mut children: FxHashMap<libc::pid_t, Vec<libc::pid_t>> = FxHashMap::default();
        for (pid, parent) in process_table() {
            children.entry(parent).or_default().push(pid);
        }
        let mut result = children.remove(&root).unwrap_or_default();
        let mut next = 0;
        while let Some(&pid) = result.get(next) {
            result.extend(children.remove(&pid).unwrap_or_default());
            next += 1;
        }
        result
    }

    /// The (pid, parent pid) of every process that can be inspected.
    #[cfg(target_os = "linux")]
    fn process_table() -> Vec<(libc::pid_t, libc::pid_t)> {
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return Vec::new();
        };
        entries
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let pid = entry.file_name().to_str()?.parse().ok()?;
                // The parent pid is the second field after the parenthesised command name,
                // which may itself contain spaces and parentheses.
                let stat = std::fs::read_to_string(entry.path().join("stat")).ok()?;
                let parent = stat.rsplit_once(')')?.1.split_whitespace().nth(1)?;
                Some((pid, parent.parse().ok()?))
            })
            .collect()
    }

    /// The (pid, parent pid) of every process that can be inspected.
    #[cfg(target_vendor = "apple")]
    fn process_table() -> Vec<(libc::pid_t, libc::pid_t)> {
        // Called without a buffer, `proc_listallpids` returns the number of processes. With a
        // buffer, it returns the number of pids it stored, which equals the buffer's capacity if
        // the buffer was too small.
        let mut capacity = unsafe { libc::proc_listallpids(ptr::null_mut(), 0) }.max(0) as usize;
        let mut pids: Vec<libc::pid_t> = Vec::new();
        loop {
            capacity += 64;
            pids.resize(capacity, 0);
            let size = (capacity * mem::size_of::<libc::pid_t>()) as libc::c_int;
            let count = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), size) };
            let count = count.max(0) as usize;
            if count < capacity {
                pids.truncate(count);
                break;
            }
        }
        pids.into_iter()
            .filter_map(|pid| {
                let mut info: libc::proc_bsdinfo = unsafe { mem::zeroed() };
                let size = mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
                let pointer = (&mut info as *mut libc::proc_bsdinfo).cast();
                let filled =
                    unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDTBSDINFO, 0, pointer, size) };
                (filled == size).then_some((pid, info.pbi_ppid as libc::pid_t))
            })
            .collect()
    }
}

#[cfg(windows)]
mod imp {
    use std::{
        io, mem,
        process::{Command, ExitStatus},
        ptr,
    };

    use winapi::{
        shared::minwindef::*,
        um::{
            handleapi::*,
            jobapi2::*,
            processthreadsapi::*,
            winnt::{HANDLE, *},
        },
    };

    pub struct Setup {
        job: Handle,
    }

    struct Handle {
        inner: HANDLE,
    }

    fn last_err() -> io::Error {
        io::Error::last_os_error()
    }

    pub(super) unsafe fn setup() -> Option<Setup> {
        // Creates a new job object for us to use and then adds ourselves to it.
        // Note that all errors are basically ignored in this function,
        // intentionally. Job objects are "relatively new" in Windows,
        // particularly the ability to support nested job objects. Older
        // Windows installs don't support this ability. We probably don't want
        // to force Cargo to abort in this situation or force others to *not*
        // use job objects, so we instead just ignore errors and assume that
        // we're otherwise part of someone else's job object in this case.

        let job = CreateJobObjectW(ptr::null_mut(), ptr::null());
        if job.is_null() {
            return None;
        }
        let job = Handle { inner: job };

        // Indicate that when all handles to the job object are gone that all
        // process in the object should be killed. Note that this includes our
        // entire process tree by default because we've added ourselves and
        // our children will reside in the job once we spawn a process.
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
        info = mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let r = SetInformationJobObject(
            job.inner,
            JobObjectExtendedLimitInformation,
            &mut info as *mut _ as LPVOID,
            mem::size_of_val(&info) as DWORD,
        );
        if r == 0 {
            return None;
        }

        // Assign our process to this job object, meaning that our children will
        // now live or die based on our existence.
        let me = GetCurrentProcess();
        let r = AssignProcessToJobObject(job.inner, me);
        if r == 0 {
            return None;
        }

        Some(Setup { job })
    }

    /// The child is in our job object, together with everything it starts.
    pub(super) fn status(cmd: &mut Command) -> io::Result<ExitStatus> {
        cmd.status()
    }

    impl Drop for Setup {
        fn drop(&mut self) {
            // On normal exits (not ctrl-c), we don't want to kill any child
            // processes. The destructor here configures our job object to
            // *not* kill everything on close, then closes the job object.
            unsafe {
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
                info = mem::zeroed();
                let r = SetInformationJobObject(
                    self.job.inner,
                    JobObjectExtendedLimitInformation,
                    &mut info as *mut _ as LPVOID,
                    mem::size_of_val(&info) as DWORD,
                );
                if r == 0 {
                    log::info!("failed to configure job object to defaults: {}", last_err());
                }
            }
        }
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.inner);
            }
        }
    }
}
