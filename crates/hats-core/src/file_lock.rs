//! File locking for shared resources in multi-loop scenarios.
//!
//! Provides fine-grained file locking using `flock()` for concurrent access
//! to shared resources like `.hats/agent/tasks.jsonl` and `.hats/agent/memories.md`.
//! This enables multiple Hats loops (in worktrees) to safely read and write
//! shared state files.
//!
//! # Design
//!
//! - **Shared locks** for reading: Multiple readers can hold shared locks simultaneously
//! - **Exclusive locks** for writing: Only one writer at a time, blocks readers
//! - **Blocking by default**: Operations wait for lock availability
//! - **RAII guards**: Locks are automatically released when guards are dropped
//!
//! # Example
//!
//! ```no_run
//! use hats_core::file_lock::FileLock;
//!
//! fn read_tasks(path: &std::path::Path) -> std::io::Result<String> {
//!     let lock = FileLock::new(path)?;
//!     let _guard = lock.shared()?;  // Acquire shared lock
//!     std::fs::read_to_string(path)
//! }
//!
//! fn write_tasks(path: &std::path::Path, content: &str) -> std::io::Result<()> {
//!     let lock = FileLock::new(path)?;
//!     let _guard = lock.exclusive()?;  // Acquire exclusive lock
//!     std::fs::write(path, content)
//! }
//! ```

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

/// A file lock for coordinating concurrent access to shared files.
///
/// Uses a `.lock` file alongside the target file for locking.
/// This avoids issues with locking the target file directly
/// (which can interfere with truncation/replacement).
#[derive(Debug)]
pub struct FileLock {
    /// Path to the lock file.
    lock_path: PathBuf,
}

impl FileLock {
    /// Creates a new file lock for the given path.
    ///
    /// The lock file is created at `{path}.lock`.
    /// The parent directory is created if it doesn't exist.
    pub fn new(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        let lock_path = path.with_extension(format!(
            "{}.lock",
            path.extension()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default()
        ));

        // Ensure parent directory exists
        if let Some(parent) = lock_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        Ok(Self { lock_path })
    }

    /// Acquires a shared (read) lock.
    ///
    /// Multiple processes can hold shared locks simultaneously.
    /// Blocks until the lock is available.
    pub fn shared(&self) -> io::Result<LockGuard> {
        self.acquire(LockType::Shared)
    }

    /// Tries to acquire a shared (read) lock without blocking.
    ///
    /// Returns `Ok(None)` if the lock is not available.
    pub fn try_shared(&self) -> io::Result<Option<LockGuard>> {
        self.try_acquire(LockType::Shared)
    }

    /// Acquires an exclusive (write) lock.
    ///
    /// Only one process can hold an exclusive lock.
    /// Blocks until the lock is available.
    pub fn exclusive(&self) -> io::Result<LockGuard> {
        self.acquire(LockType::Exclusive)
    }

    /// Tries to acquire an exclusive (write) lock without blocking.
    ///
    /// Returns `Ok(None)` if the lock is not available.
    pub fn try_exclusive(&self) -> io::Result<Option<LockGuard>> {
        self.try_acquire(LockType::Exclusive)
    }

    /// Acquires a lock of the specified type (blocking).
    fn acquire(&self, lock_type: LockType) -> io::Result<LockGuard> {
        let file = self.open_lock_file()?;

        #[cfg(unix)]
        {
            use nix::fcntl::{Flock, FlockArg};

            let arg = match lock_type {
                LockType::Shared => FlockArg::LockShared,
                LockType::Exclusive => FlockArg::LockExclusive,
            };

            match Flock::lock(file, arg) {
                Ok(flock) => Ok(LockGuard {
                    _flock: flock,
                    _lock_type: lock_type,
                }),
                Err((_, errno)) => Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("flock failed: {}", errno),
                )),
            }
        }

        #[cfg(not(unix))]
        {
            let _ = (file, lock_type);
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "File locking not supported on this platform",
            ))
        }
    }

    /// Tries to acquire a lock of the specified type (non-blocking).
    fn try_acquire(&self, lock_type: LockType) -> io::Result<Option<LockGuard>> {
        let file = self.open_lock_file()?;

        #[cfg(unix)]
        {
            use nix::errno::Errno;
            use nix::fcntl::{Flock, FlockArg};

            let arg = match lock_type {
                LockType::Shared => FlockArg::LockSharedNonblock,
                LockType::Exclusive => FlockArg::LockExclusiveNonblock,
            };

            match Flock::lock(file, arg) {
                Ok(flock) => Ok(Some(LockGuard {
                    _flock: flock,
                    _lock_type: lock_type,
                })),
                Err((_, errno)) if errno == Errno::EWOULDBLOCK || errno == Errno::EAGAIN => {
                    Ok(None)
                }
                Err((_, errno)) => Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("flock failed: {}", errno),
                )),
            }
        }

        #[cfg(not(unix))]
        {
            let _ = (file, lock_type);
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "File locking not supported on this platform",
            ))
        }
    }

    /// Opens or creates the lock file.
    fn open_lock_file(&self) -> io::Result<File> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.lock_path)
    }

    /// Returns the path to the lock file.
    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }
}

/// Type of lock to acquire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LockType {
    /// Shared (read) lock - multiple holders allowed.
    Shared,
    /// Exclusive (write) lock - single holder only.
    Exclusive,
}

/// A guard that holds the file lock. The lock is released when dropped.
#[derive(Debug)]
pub struct LockGuard {
    /// The flock guard (Unix only).
    #[cfg(unix)]
    _flock: nix::fcntl::Flock<File>,

    /// The type of lock held.
    _lock_type: LockType,
}

/// A locked file that provides safe read/write access.
///
/// This is a convenience wrapper that combines file locking with
/// read/write operations in a single API.
pub struct LockedFile {
    lock: FileLock,
}

impl LockedFile {
    /// Creates a new locked file handle for the given path.
    pub fn new(path: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self {
            lock: FileLock::new(path)?,
        })
    }

    /// Reads the file contents with a shared lock.
    ///
    /// If the file doesn't exist, returns an empty string.
    pub fn read(&self, path: &Path) -> io::Result<String> {
        let _guard = self.lock.shared()?;
        if path.exists() {
            std::fs::read_to_string(path)
        } else {
            Ok(String::new())
        }
    }

    /// Writes content to the file with an exclusive lock.
    pub fn write(&self, path: &Path, content: &str) -> io::Result<()> {
        let _guard = self.lock.exclusive()?;
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)
    }

    /// Executes a read operation with a shared lock.
    pub fn with_shared_lock<T, F>(&self, f: F) -> io::Result<T>
    where
        F: FnOnce() -> io::Result<T>,
    {
        let _guard = self.lock.shared()?;
        f()
    }

    /// Executes a write operation with an exclusive lock.
    pub fn with_exclusive_lock<T, F>(&self, f: F) -> io::Result<T>
    where
        F: FnOnce() -> io::Result<T>,
    {
        let _guard = self.lock.exclusive()?;
        f()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::{Duration, Instant};
    use tempfile::TempDir;

    #[test]
    fn test_lock_file_path() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.jsonl");

        let lock = FileLock::new(&file_path).unwrap();
        assert_eq!(lock.lock_path(), temp_dir.path().join("test.jsonl.lock"));
    }

    #[test]
    fn test_lock_file_path_no_extension() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("tasks");

        let lock = FileLock::new(&file_path).unwrap();
        assert_eq!(lock.lock_path(), temp_dir.path().join("tasks..lock"));
    }

    #[test]
    fn test_shared_lock_acquired() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let lock = FileLock::new(&file_path).unwrap();
        let guard = lock.shared();
        assert!(guard.is_ok());
    }

    #[test]
    fn test_exclusive_lock_acquired() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let lock = FileLock::new(&file_path).unwrap();
        let guard = lock.exclusive();
        assert!(guard.is_ok());
    }

    #[test]
    fn test_multiple_shared_locks() {
        // Multiple shared locks should be allowed
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let lock1 = FileLock::new(&file_path).unwrap();
        let lock2 = FileLock::new(&file_path).unwrap();

        let _guard1 = lock1.shared().unwrap();
        let guard2 = lock2.try_shared();

        // Should be able to acquire second shared lock
        assert!(guard2.is_ok());
        assert!(guard2.unwrap().is_some());
    }

    #[test]
    fn test_exclusive_blocks_shared() {
        // Exclusive lock should block new shared locks
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let lock1 = FileLock::new(&file_path).unwrap();
        let lock2 = FileLock::new(&file_path).unwrap();

        let _guard1 = lock1.exclusive().unwrap();
        let guard2 = lock2.try_shared();

        // Should not be able to acquire shared lock while exclusive is held
        assert!(guard2.is_ok());
        assert!(guard2.unwrap().is_none());
    }

    #[test]
    fn test_shared_blocks_exclusive() {
        // Shared lock should block exclusive locks
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let lock1 = FileLock::new(&file_path).unwrap();
        let lock2 = FileLock::new(&file_path).unwrap();

        let _guard1 = lock1.shared().unwrap();
        let guard2 = lock2.try_exclusive();

        // Should not be able to acquire exclusive lock while shared is held
        assert!(guard2.is_ok());
        assert!(guard2.unwrap().is_none());
    }

    #[test]
    fn test_lock_released_on_drop() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let lock1 = FileLock::new(&file_path).unwrap();
        let lock2 = FileLock::new(&file_path).unwrap();

        {
            let _guard1 = lock1.exclusive().unwrap();
            // Lock is held
        }
        // Guard dropped, lock released

        // Should be able to acquire exclusive lock now
        let guard2 = lock2.try_exclusive();
        assert!(guard2.is_ok());
        assert!(guard2.unwrap().is_some());
    }

    #[test]
    fn test_locked_file_read_write() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let locked = LockedFile::new(&file_path).unwrap();

        // Write content
        locked.write(&file_path, "Hello, World!").unwrap();

        // Read content
        let content = locked.read(&file_path).unwrap();
        assert_eq!(content, "Hello, World!");
    }

    #[test]
    fn test_locked_file_read_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("nonexistent.txt");

        let locked = LockedFile::new(&file_path).unwrap();
        let content = locked.read(&file_path).unwrap();
        assert!(content.is_empty());
    }

    #[test]
    fn test_concurrent_writes_serialized() {
        // Test that concurrent writes are properly serialized
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("counter.txt");
        let file_path_clone = file_path.clone();

        // Initialize file
        std::fs::write(&file_path, "0").unwrap();

        let barrier = Arc::new(Barrier::new(2));
        let barrier_clone = barrier.clone();

        let handle1 = thread::spawn(move || {
            let locked = LockedFile::new(&file_path).unwrap();
            barrier.wait();

            locked
                .with_exclusive_lock(|| {
                    let content = std::fs::read_to_string(&file_path)?;
                    let n: i32 = content.trim().parse().unwrap_or(0);
                    // Small delay to increase chance of race condition without lock
                    thread::sleep(Duration::from_millis(10));
                    std::fs::write(&file_path, format!("{}", n + 1))
                })
                .unwrap();
        });

        let handle2 = thread::spawn(move || {
            let locked = LockedFile::new(&file_path_clone).unwrap();
            barrier_clone.wait();

            locked
                .with_exclusive_lock(|| {
                    let content = std::fs::read_to_string(&file_path_clone)?;
                    let n: i32 = content.trim().parse().unwrap_or(0);
                    thread::sleep(Duration::from_millis(10));
                    std::fs::write(&file_path_clone, format!("{}", n + 1))
                })
                .unwrap();
        });

        handle1.join().unwrap();
        handle2.join().unwrap();

        // With proper locking, final value should be 2
        let final_content = std::fs::read_to_string(temp_dir.path().join("counter.txt")).unwrap();
        assert_eq!(final_content.trim(), "2");
    }

    #[test]
    fn test_blocking_lock_waits() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("wait.txt");
        let file_path_clone = file_path.clone();

        let barrier = Arc::new(Barrier::new(2));
        let barrier_clone = barrier.clone();

        let handle1 = thread::spawn(move || {
            let lock = FileLock::new(&file_path).unwrap();
            let _guard = lock.exclusive().unwrap();

            barrier.wait();
            // Hold lock for a bit
            thread::sleep(Duration::from_millis(50));
            // Guard dropped here
        });

        let start = Instant::now();
        let handle2 = thread::spawn(move || {
            let lock = FileLock::new(&file_path_clone).unwrap();
            barrier_clone.wait();

            // This should block until handle1 releases the lock
            let _guard = lock.exclusive().unwrap();
        });

        handle1.join().unwrap();
        handle2.join().unwrap();

        // Second thread should have waited
        assert!(start.elapsed() >= Duration::from_millis(40));
    }
}
