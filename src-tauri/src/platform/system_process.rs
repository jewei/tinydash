use std::{
    io::{Read, Seek, SeekFrom},
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use crate::error::{Error, Result};

/// Run a fixed OS utility on the action worker, without a shell or an unbounded wait.
pub(super) fn output(program: impl AsRef<Path>, arguments: &[&str]) -> Result<String> {
    let program = program.as_ref();
    let failure = |error| Error::SystemCommand(format!("{}: {error}", program.display()));
    // Files avoid pipe deadlocks while waiting for the process. Only a bounded
    // diagnostic is loaded, and tempfile removes both files when this returns.
    let mut stdout = tempfile::tempfile().map_err(failure)?;
    let mut stderr = tempfile::tempfile().map_err(failure)?;
    let mut child = Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(stdout.try_clone().map_err(failure)?)
        .stderr(stderr.try_clone().map_err(failure)?)
        .spawn()
        .map_err(failure)?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < Duration::from_secs(30) => {
                std::thread::sleep(Duration::from_millis(20));
            }
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::SystemCommand(match result {
                    Err(error) => error.to_string(),
                    _ => "The system command timed out. It may still complete. Check the system state before trying again.".into(),
                }));
            }
        }
    };
    let read = |file: &mut std::fs::File| -> Result<String> {
        file.seek(SeekFrom::Start(0)).map_err(failure)?;
        let mut bytes = Vec::new();
        file.take(4096).read_to_end(&mut bytes).map_err(failure)?;
        Ok(String::from_utf8_lossy(&bytes).trim().to_owned())
    };
    if status.success() {
        read(&mut stdout)
    } else {
        let message = read(&mut stderr)?;
        Err(Error::SystemCommand(format!(
            "{} returned {status}. {message}",
            program.display()
        )))
    }
}
