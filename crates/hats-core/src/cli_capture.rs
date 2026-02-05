//! CLI output capture for recording terminal sessions.
//!
//! `CliCapture` wraps a `Write` implementation to capture all bytes written
//! to stdout/stderr while forwarding them to the underlying writer. This
//! enables transparent recording without changing calling code.

use hats_proto::{FrameCapture, TerminalWrite, UxEvent};
use std::io::{self, Write};
use std::time::Instant;

/// A writer that captures all output while forwarding to an inner writer.
///
/// This wrapper implements `std::io::Write` and records every write operation
/// as a `UxEvent::TerminalWrite`. The captured events can be retrieved via
/// the `FrameCapture` trait for session recording.
///
/// # Example
///
/// ```
/// use hats_core::CliCapture;
/// use hats_proto::FrameCapture;
/// use std::io::Write;
///
/// let mut output = Vec::new();
/// let mut capture = CliCapture::new(&mut output, true);
///
/// writeln!(capture, "Hello, World!").unwrap();
///
/// // And captured as UX events
/// let events = capture.take_captures();
/// assert_eq!(events.len(), 1);
///
/// // Output was forwarded to the inner writer (checked after capture drops borrow)
/// drop(capture);
/// assert!(String::from_utf8_lossy(&output).contains("Hello"));
/// ```
pub struct CliCapture<W> {
    /// The underlying writer to forward output to.
    inner: W,

    /// Captured UX events.
    captures: Vec<UxEvent>,

    /// Time when capture started, for calculating offsets.
    start_time: Instant,

    /// Whether this captures stdout (true) or stderr (false).
    is_stdout: bool,
}

impl<W> CliCapture<W> {
    /// Creates a new capture wrapper around the given writer.
    ///
    /// # Arguments
    ///
    /// * `inner` - The writer to forward output to
    /// * `is_stdout` - `true` if capturing stdout, `false` for stderr
    pub fn new(inner: W, is_stdout: bool) -> Self {
        Self {
            inner,
            captures: Vec::new(),
            start_time: Instant::now(),
            is_stdout,
        }
    }

    /// Creates a capture wrapper with a custom start time.
    ///
    /// This is useful when coordinating multiple captures (stdout + stderr)
    /// that should share the same timing baseline.
    pub fn with_start_time(inner: W, is_stdout: bool, start_time: Instant) -> Self {
        Self {
            inner,
            captures: Vec::new(),
            start_time,
            is_stdout,
        }
    }

    /// Returns the current offset in milliseconds since capture started.
    #[allow(clippy::cast_possible_truncation)]
    fn offset_ms(&self) -> u64 {
        // Safe: milliseconds since start won't exceed u64 in practice
        self.start_time.elapsed().as_millis() as u64
    }

    /// Returns a reference to the inner writer.
    pub fn inner(&self) -> &W {
        &self.inner
    }

    /// Returns a mutable reference to the inner writer.
    pub fn inner_mut(&mut self) -> &mut W {
        &mut self.inner
    }

    /// Consumes the capture and returns the inner writer.
    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: Write> Write for CliCapture<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Forward to inner writer first
        let n = self.inner.write(buf)?;

        // Only capture the bytes that were actually written
        if n > 0 {
            self.captures
                .push(UxEvent::TerminalWrite(TerminalWrite::new(
                    &buf[..n],
                    self.is_stdout,
                    self.offset_ms(),
                )));
        }

        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<W: Send + Sync> FrameCapture for CliCapture<W> {
    fn take_captures(&mut self) -> Vec<UxEvent> {
        std::mem::take(&mut self.captures)
    }

    fn has_captures(&self) -> bool {
        !self.captures.is_empty()
    }
}

/// A pair of capture wrappers for stdout and stderr.
///
/// This struct coordinates captures for both streams with a shared start time,
/// ensuring timing offsets are consistent across stdout and stderr events.
pub struct CliCapturePair<Stdout, Stderr> {
    /// Capture wrapper for stdout.
    pub stdout: CliCapture<Stdout>,

    /// Capture wrapper for stderr.
    pub stderr: CliCapture<Stderr>,
}

impl<Stdout, Stderr> CliCapturePair<Stdout, Stderr> {
    /// Creates a new capture pair with a shared start time.
    pub fn new(stdout: Stdout, stderr: Stderr) -> Self {
        let start_time = Instant::now();
        Self {
            stdout: CliCapture::with_start_time(stdout, true, start_time),
            stderr: CliCapture::with_start_time(stderr, false, start_time),
        }
    }
}

impl<Stdout: Send + Sync, Stderr: Send + Sync> CliCapturePair<Stdout, Stderr> {
    /// Takes all captured events from both streams, merged in chronological order.
    pub fn take_all_captures(&mut self) -> Vec<UxEvent> {
        let mut stdout_events = self.stdout.take_captures();
        let mut stderr_events = self.stderr.take_captures();

        // Merge and sort by offset_ms
        stdout_events.append(&mut stderr_events);
        stdout_events.sort_by_key(|event| match event {
            UxEvent::TerminalWrite(tw) => tw.offset_ms,
            UxEvent::TerminalResize(tr) => tr.offset_ms,
            UxEvent::TerminalColorMode(cm) => cm.offset_ms,
            UxEvent::TuiFrame(tf) => tf.offset_ms,
        });

        stdout_events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_write() {
        let mut output = Vec::new();

        // Scope the capture so we can check output afterward
        let events = {
            let mut capture = CliCapture::new(&mut output, true);
            write!(capture, "Hello").unwrap();
            capture.take_captures()
        };

        // Check output was forwarded
        assert_eq!(output, b"Hello");

        // Check event was captured
        assert_eq!(events.len(), 1);

        if let UxEvent::TerminalWrite(tw) = &events[0] {
            assert!(tw.stdout);
            let decoded = tw.decode_bytes().unwrap();
            assert_eq!(decoded, b"Hello");
        } else {
            panic!("Expected TerminalWrite event");
        }
    }

    #[test]
    fn test_capture_multiple_writes() {
        let mut output = Vec::new();
        let mut capture = CliCapture::new(&mut output, true);

        writeln!(capture, "Line 1").unwrap();
        writeln!(capture, "Line 2").unwrap();

        let events = capture.take_captures();
        assert_eq!(events.len(), 2);

        // Check offsets are monotonic
        if let (UxEvent::TerminalWrite(tw1), UxEvent::TerminalWrite(tw2)) = (&events[0], &events[1])
        {
            assert!(tw2.offset_ms >= tw1.offset_ms);
        }
    }

    #[test]
    fn test_capture_stderr() {
        let mut output = Vec::new();
        let mut capture = CliCapture::new(&mut output, false);

        write!(capture, "Error!").unwrap();

        let events = capture.take_captures();
        if let UxEvent::TerminalWrite(tw) = &events[0] {
            assert!(!tw.stdout); // stderr
        }
    }

    #[test]
    fn test_capture_pair() {
        let stdout_buf = Vec::new();
        let stderr_buf = Vec::new();
        let mut pair = CliCapturePair::new(stdout_buf, stderr_buf);

        write!(pair.stdout, "out").unwrap();
        write!(pair.stderr, "err").unwrap();

        let events = pair.take_all_captures();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_take_captures_clears_buffer() {
        let mut output = Vec::new();
        let mut capture = CliCapture::new(&mut output, true);

        write!(capture, "test").unwrap();
        assert!(capture.has_captures());

        let events = capture.take_captures();
        assert_eq!(events.len(), 1);

        // Buffer should be cleared
        assert!(!capture.has_captures());
        assert!(capture.take_captures().is_empty());
    }

    #[test]
    fn test_ansi_preservation() {
        let mut output = Vec::new();
        let mut capture = CliCapture::new(&mut output, true);

        // Write ANSI escape sequence for green text
        let ansi_text = b"\x1b[32mGreen\x1b[0m";
        capture.write_all(ansi_text).unwrap();

        let events = capture.take_captures();
        if let UxEvent::TerminalWrite(tw) = &events[0] {
            let decoded = tw.decode_bytes().unwrap();
            assert_eq!(decoded, ansi_text);
        }
    }
}
