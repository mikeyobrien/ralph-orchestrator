use std::fs::OpenOptions;
use std::path::{Path, PathBuf};

use anyhow::Context;

pub(crate) const LOOP_LOCK_FILE: &str = ".hats/loop.lock";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LockState {
    Active,
    Inactive,
    Stale,
}

pub(crate) fn lock_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(LOOP_LOCK_FILE)
}

pub(crate) fn lock_state(workspace_root: &Path) -> anyhow::Result<LockState> {
    let path = lock_path(workspace_root);

    if !path.exists() {
        return Ok(LockState::Inactive);
    }

    #[cfg(unix)]
    {
        use nix::errno::Errno;
        use nix::fcntl::{Flock, FlockArg};

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .with_context(|| format!("open lock file at {}", path.display()))?;

        match Flock::lock(file, FlockArg::LockExclusiveNonblock) {
            Ok(_flock) => Ok(LockState::Stale),
            Err((_, errno)) if errno == Errno::EWOULDBLOCK || errno == Errno::EAGAIN => {
                Ok(LockState::Active)
            }
            Err((_, errno)) => Err(anyhow::anyhow!(
                "flock failed for {}: {}",
                path.display(),
                errno
            )),
        }
    }

    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(LockState::Active)
    }
}
