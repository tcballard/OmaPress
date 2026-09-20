//! Bounded subprocess execution. Keep the unreaped leader as the process-group
//! identity until all group signals have been sent. No shell evaluates arguments.
use anyhow::{Context, Result, bail};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    os::unix::process::CommandExt,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
pub fn run(program: &str, args: &[&str], cwd: Option<&Path>, timeout: Duration) -> Result<String> {
    run_input(program, args, cwd, timeout, &[])
}
pub fn run_input(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    timeout: Duration,
    input: &[u8],
) -> Result<String> {
    let mut stdin = tempfile::tempfile()?;
    stdin.write_all(input)?;
    stdin.seek(SeekFrom::Start(0))?;
    let stdout = tempfile::tempfile()?;
    let stderr = tempfile::tempfile()?;
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::from(stdin))
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?)
        .process_group(0)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GH_PROMPT_DISABLED", "1")
        .env("GIT_CONFIG_NOSYSTEM", "1");
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    // The engine may be terminated by its desktop owner. Never leave curl/git running.
    // SAFETY: pre_exec uses only async-signal-safe libc operations, without allocation.
    let parent = unsafe { libc::getpid() };
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() != parent {
                libc::_exit(1);
            }
            Ok(())
        });
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("Cannot start {program}"))?;
    let pid = child.id() as libc::pid_t;
    let started = Instant::now();
    let mut timed_out = false;
    loop {
        // SAFETY: waitid is called for our child only; zeroed siginfo_t is valid,
        // WNOWAIT retains its PID until teardown below completes.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                pid as libc::id_t,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result < 0 {
            let error = std::io::Error::last_os_error();
            let _ = child.kill();
            let _ = child.wait();
            return Err(error.into());
        }
        if unsafe { info.si_pid() } == pid {
            break;
        }
        if started.elapsed() > timeout
            || stdout.metadata()?.len() > 8 * 1024 * 1024
            || stderr.metadata()?.len() > 1024 * 1024
        {
            timed_out = true;
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    // A browser deliberately outlives its successful opener. Other descendants are owned work.
    if timed_out || program != "xdg-open" {
        // SAFETY: our unreaped leader still pins this process-group ID.
        unsafe {
            libc::kill(-pid, libc::SIGTERM);
        }
        thread::sleep(Duration::from_millis(30));
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
    let status = child.wait()?;
    fn read(mut f: File) -> Result<String> {
        use std::io::{Seek, SeekFrom};
        f.seek(SeekFrom::Start(0))?;
        let mut b = String::new();
        f.take(8 * 1024 * 1024).read_to_string(&mut b)?;
        Ok(b)
    }
    let out = read(stdout)?;
    let err = read(stderr)?;
    if timed_out {
        bail!("{program} exceeded its time or output budget");
    }
    if !status.success() {
        bail!(
            "{program} failed ({}): {}",
            status.code().unwrap_or(-1),
            err.trim()
        );
    }
    Ok(out)
}
