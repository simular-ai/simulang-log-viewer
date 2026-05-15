//! NDJSON wire format between the napi parent and the viewer subprocess.
//!
//! One JSON object per line. The napi side (in `lib.rs`) writes
//! [`FromParent`] to the child's stdin. The viewer (in `bin/`) writes
//! [`FromChild`] to its stdout when the pause state changes.
//!
//! Both halves of the wire live in this single module so the format is
//! defined exactly once.

use serde::{Deserialize, Serialize};

/// Messages from the napi parent to the viewer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FromParent {
    /// Append a message line to the viewer's display.
    Log { message: String },
    /// Drop all displayed messages.
    Clear,
    /// Ask the viewer to exit gracefully. The viewer also exits when its
    /// stdin reaches EOF (i.e. the parent process died).
    Close,
}

/// Messages from the viewer back to the napi parent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FromChild {
    /// Pause state changed. Pause is entered/left exclusively by the user
    /// pressing the global grab hotkey (which also flips the viewer out
    /// of click-through mode so they can drag the window).
    State { paused: bool },
}
