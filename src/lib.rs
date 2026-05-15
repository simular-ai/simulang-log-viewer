//! napi bindings exposing a `LogWindow` JS class.
//!
//! `LogWindow.spawn()` launches the bundled `simulang-log-viewer` binary as a
//! subprocess (the eframe app). Records, control commands, and pause-state
//! events flow over the child's stdin/stdout as NDJSON (see [`ipc`] for the
//! wire format).
//!
//! The viewer must be a separate process: cross-platform Rust GUI toolkits
//! (eframe / winit) require the OS process main thread on macOS, which Node
//! already owns. The binary lives next to this `.node` file in the npm
//! tarball; we discover its path with [`cdylib_path::viewer_binary`].

mod cdylib_path;
pub mod ipc;

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use napi::Error;
use napi::bindgen_prelude::AsyncTask;
use napi_derive::napi;

use crate::ipc::{FromChild, FromParent};

/// Shared mutable state between the napi class, the stdout reader thread,
/// and the JS-facing methods. Held inside an `Arc` so the reader thread can
/// keep its own clone alive even after the `LogWindow` handle is dropped on
/// the JS side.
struct Inner {
    child: Option<Child>,
    paused: bool,
    /// `true` once the child has exited (EOF on stdout) or `close()` was
    /// called explicitly. Wakes any pending `wait_if_paused` so callers
    /// don't hang forever on a dead viewer.
    closed: bool,
    /// `true` once `close()` has been invoked from JS. Lets the reader
    /// thread tell user-initiated closes (e.g. clicking the window's
    /// close button) apart from programmatic ones so it only logs the
    /// former.
    close_requested: bool,
}

type SharedState = Arc<(Mutex<Inner>, Condvar)>;

/// Background-thread task that blocks on the pause condvar until the
/// viewer reports `paused = false` (or it closes). Returned from
/// `wait_if_paused` so the JS caller can `await` it without blocking
/// Node's event loop.
pub struct WaitIfPausedTask {
    state: SharedState,
}

impl napi::Task for WaitIfPausedTask {
    type Output = ();
    type JsValue = ();

    fn compute(&mut self) -> napi::Result<Self::Output> {
        let (lock, cvar) = &*self.state;
        let mut inner = lock
            .lock()
            .map_err(|e| Error::from_reason(format!("LogWindow state poisoned: {e}")))?;
        while inner.paused && !inner.closed {
            inner = cvar
                .wait(inner)
                .map_err(|e| Error::from_reason(format!("LogWindow state poisoned: {e}")))?;
        }
        Ok(())
    }

    fn resolve(&mut self, _env: napi::Env, _output: ()) -> napi::Result<()> {
        Ok(())
    }
}

/// A floating, always-on-top log window driven by an external eframe
/// process. See the package README for the JS-facing usage pattern.
#[napi]
pub struct LogWindow {
    state: SharedState,
}

impl Default for LogWindow {
    fn default() -> Self {
        Self::new()
    }
}

#[napi]
impl LogWindow {
    /// Construct a handle. Does not spawn yet — call `spawn()`.
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            state: Arc::new((
                Mutex::new(Inner {
                    child: None,
                    paused: false,
                    closed: false,
                    close_requested: false,
                }),
                Condvar::new(),
            )),
        }
    }

    /// Launch the viewer subprocess. Returns immediately; the window opens
    /// asynchronously. Calling `spawn` more than once on the same handle is
    /// an error — construct a fresh `LogWindow` if you need to reopen.
    #[napi]
    pub fn spawn(&self) -> napi::Result<()> {
        let viewer_path = cdylib_path::viewer_binary().ok_or_else(|| {
            Error::from_reason(
                "could not locate the simulang-log-viewer binary next to the loaded \
         .node file (cdylib path discovery failed)",
            )
        })?;

        let mut child = Command::new(&viewer_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| {
                Error::from_reason(format!("failed to spawn viewer at {viewer_path:?}: {e}"))
            })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::from_reason("viewer stdout was not piped"))?;

        let state_for_reader = self.state.clone();
        thread::spawn(move || run_reader(state_for_reader, stdout));

        let (lock, _) = &*self.state;
        let mut inner = lock
            .lock()
            .map_err(|e| Error::from_reason(format!("LogWindow state poisoned: {e}")))?;
        if inner.child.is_some() {
            return Err(Error::from_reason(
                "LogWindow already spawned — construct a new instance to reopen",
            ));
        }
        inner.child = Some(child);
        Ok(())
    }

    /// Append a single message line to the viewer **and** mirror it
    /// to the host process's stderr so the line is always visible in
    /// the terminal too. The terminal copy keeps logs flowing for
    /// `tee`/CI capture and survives the user clicking the close
    /// button on the floating window.
    ///
    /// The viewer write is best-effort: if the viewer has already
    /// been closed, the line is dropped silently instead of throwing,
    /// so a dismissed window never crashes a long-running script.
    #[napi]
    pub fn log(&self, message: String) -> napi::Result<()> {
        let _ = writeln!(std::io::stderr(), "{message}");
        self.send_lossy(&FromParent::Log { message })
    }

    /// Drop all displayed messages. Silently no-ops when the viewer
    /// has already closed, matching `log()`.
    #[napi]
    pub fn clear(&self) -> napi::Result<()> {
        self.send_lossy(&FromParent::Clear)
    }

    /// `true` while execution is paused. The user pauses (and resumes)
    /// by pressing the global pause hotkey shown inside the window;
    /// the same hotkey also flips the viewer out of click-through mode
    /// so they can drag it. Cheap O(1) lookup against the shared state.
    #[napi(getter)]
    pub fn is_paused(&self) -> napi::Result<bool> {
        let (lock, _) = &*self.state;
        let inner = lock
            .lock()
            .map_err(|e| Error::from_reason(format!("LogWindow state poisoned: {e}")))?;
        Ok(inner.paused)
    }

    /// Block until the viewer is no longer paused. Returns immediately if not
    /// currently paused. Implemented as an async task so the JS event loop
    /// stays responsive while waiting.
    ///
    /// Returns immediately if the viewer has already closed.
    #[napi(ts_return_type = "Promise<void>")]
    pub fn wait_if_paused(&self) -> AsyncTask<WaitIfPausedTask> {
        AsyncTask::new(WaitIfPausedTask {
            state: self.state.clone(),
        })
    }

    /// Synchronously block the calling thread until the viewer is no
    /// longer paused (or has closed). Used by `@simular-ai/simulang-js`
    /// as a `setPauseHook` so every simulang-js call transparently
    /// honors the pause state.
    ///
    /// This freezes Node's event loop while blocked — that's the
    /// point: an automation script asking the OS to type, click, or
    /// capture should look synchronous from the script's perspective.
    /// Use `wait_if_paused()` (the async variant) instead when you
    /// need timers, promises, or other JS work to keep running.
    #[napi]
    pub fn wait_if_paused_sync(&self) -> napi::Result<()> {
        let (lock, cvar) = &*self.state;
        let mut inner = lock
            .lock()
            .map_err(|e| Error::from_reason(format!("LogWindow state poisoned: {e}")))?;
        while inner.paused && !inner.closed {
            inner = cvar
                .wait(inner)
                .map_err(|e| Error::from_reason(format!("LogWindow state poisoned: {e}")))?;
        }
        Ok(())
    }

    /// Ask the viewer to close gracefully, then `wait()` for the process.
    ///
    /// Closing is also automatic when the parent process dies (the child sees
    /// EOF on its stdin), so calling this is optional.
    #[napi]
    pub fn close(&self) -> napi::Result<()> {
        {
            let (lock, _) = &*self.state;
            let mut inner = lock
                .lock()
                .map_err(|e| Error::from_reason(format!("LogWindow state poisoned: {e}")))?;
            inner.close_requested = true;
        }
        let _ = self.send(&FromParent::Close);

        let (lock, cvar) = &*self.state;
        let mut inner = lock
            .lock()
            .map_err(|e| Error::from_reason(format!("LogWindow state poisoned: {e}")))?;
        if let Some(mut child) = inner.child.take() {
            drop(inner);
            let _ = child.wait();
            let mut inner = lock
                .lock()
                .map_err(|e| Error::from_reason(format!("LogWindow state poisoned: {e}")))?;
            inner.closed = true;
            inner.paused = false;
            cvar.notify_all();
        }
        Ok(())
    }

    /// Serialize a `FromParent` and write it as a NDJSON line on the child's
    /// stdin. Strict: errors out if the viewer has closed or stdin write
    /// fails. Used by `close()` where surfacing the failure matters.
    fn send(&self, msg: &FromParent) -> napi::Result<()> {
        let (lock, _) = &*self.state;
        let mut inner = lock
            .lock()
            .map_err(|e| Error::from_reason(format!("LogWindow state poisoned: {e}")))?;
        if inner.closed {
            return Err(Error::from_reason("LogWindow has been closed"));
        }
        let stdin: &mut ChildStdin = inner
            .child
            .as_mut()
            .ok_or_else(|| Error::from_reason("LogWindow not spawned — call spawn() first"))?
            .stdin
            .as_mut()
            .ok_or_else(|| Error::from_reason("viewer stdin not available"))?;

        let line = serde_json::to_string(msg)
            .map_err(|e| Error::from_reason(format!("serialize: {e}")))?;
        stdin
            .write_all(line.as_bytes())
            .map_err(|e| Error::from_reason(format!("write: {e}")))?;
        stdin
            .write_all(b"\n")
            .map_err(|e| Error::from_reason(format!("write: {e}")))?;
        Ok(())
    }

    /// Like [`send`], but silently drops the message if the viewer
    /// already closed or the write fails. Used by the data-plane log
    /// path (`log`, `clear`) so a dismissed viewer never crashes a
    /// long-running automation script.
    fn send_lossy(&self, msg: &FromParent) -> napi::Result<()> {
        let (lock, _) = &*self.state;
        let mut inner = lock
            .lock()
            .map_err(|e| Error::from_reason(format!("LogWindow state poisoned: {e}")))?;
        if inner.closed {
            return Ok(());
        }
        let Some(child) = inner.child.as_mut() else {
            return Ok(());
        };
        let Some(stdin) = child.stdin.as_mut() else {
            return Ok(());
        };

        let Ok(line) = serde_json::to_string(msg) else {
            return Ok(());
        };
        let _ = stdin.write_all(line.as_bytes());
        let _ = stdin.write_all(b"\n");
        Ok(())
    }
}

/// Read NDJSON `FromChild` messages off the viewer's stdout in a loop.
/// Updates the shared `paused` flag and wakes any pending
/// `WaitIfPausedTask`. Exits when the child closes its stdout (i.e. on
/// viewer shutdown), at which point we mark the state `closed` so any
/// pending wait unblocks instead of hanging forever.
fn run_reader(state: SharedState, stdout: std::process::ChildStdout) {
    let reader = BufReader::new(stdout);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(msg) = serde_json::from_str::<FromChild>(trimmed) else {
            continue;
        };
        let (lock, cvar) = &*state;
        if let Ok(mut inner) = lock.lock() {
            match msg {
                FromChild::State { paused } => {
                    // Only emit on actual transitions so a duplicate state
                    // message from the viewer doesn't double-log.
                    if inner.paused != paused {
                        emit_transition_log(&mut inner, paused);
                    }
                    inner.paused = paused;
                    cvar.notify_all();
                }
            }
        }
    }
    let (lock, cvar) = &*state;
    if let Ok(mut inner) = lock.lock() {
        // Mirror a user-initiated close (e.g. clicking the window's
        // close button) to stderr so the host terminal sees the
        // transition. A programmatic `close()` has already flipped
        // `close_requested`, so we stay silent in that case to avoid
        // double-logging an action the host itself triggered.
        if !inner.close_requested && !inner.closed {
            let _ = writeln!(
                std::io::stderr(),
                "[INFO] Resume: Log window closed by user"
            );
        }
        inner.closed = true;
        inner.paused = false;
        cvar.notify_all();
    }
}

/// Mirror a pause/resume transition to both the viewer (so the user
/// sees it rendered) and the host process's stderr (so it shows up in
/// the terminal alongside the records routed through `simulang-js`'s
/// logger pipeline).
///
/// Deliberately avoids the `log` crate: simulang-log-viewer is a separate
/// `cdylib` from simulang-js, so each binary holds its own `log` crate
/// global. `initLogger` on the simulang-js side cannot observe records
/// emitted from here.
fn emit_transition_log(inner: &mut Inner, paused: bool) {
    let line = if paused {
        "[INFO] Paused by user"
    } else {
        "[INFO] Resumed by user"
    };
    let _ = writeln!(std::io::stderr(), "{line}");

    let log_msg = FromParent::Log {
        message: line.to_string(),
    };
    let Ok(json) = serde_json::to_string(&log_msg) else {
        return;
    };
    if let Some(child) = inner.child.as_mut()
        && let Some(stdin) = child.stdin.as_mut()
    {
        let _ = stdin.write_all(json.as_bytes());
        let _ = stdin.write_all(b"\n");
    }
}
