//! Standalone log viewer window.
//!
//! Reads NDJSON [`FromParent`] messages on stdin (log lines + control), and
//! writes NDJSON [`FromChild`] state events to stdout when the pause state
//! changes. The viewer exits when stdin reaches EOF (parent died) or when
//! it receives a [`FromParent::Close`] message.
//!
//! Window UX:
//! - Always-on-top, transparent, click-through (mouse passthrough on)
//!   100% of the time except while the user has explicitly grabbed the
//!   window with the global grab hotkey.
//! - Pressing the global grab hotkey (see [`GRAB_HOTKEY_LABEL`]) toggles
//!   "grab mode": passthrough flips off so the user can drag the window
//!   via its native title bar, and the parent is notified that execution
//!   should pause. Pressing the hotkey again leaves grab mode (passthrough
//!   back on, parent un-paused).
//! - macOS: excluded from screen capture (NSWindowSharingNone).
//!
//! There is intentionally no hover-to-pause: the cursor passing over the
//! window — whether driven by a human or by automation — never grabs
//! input. The only way the viewer ever stops being click-through is the
//! deliberate, OS-wide grab hotkey.

use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

// Include the shared IPC module directly — going through the lib's rlib
// would try to link against the napi-using parts of `lib.rs`, which can't
// resolve their `napi_*` symbols outside Node.
#[path = "../ipc.rs"]
mod ipc;

use ipc::{FromChild, FromParent};

/// The key (without modifiers) that activates "grab mode."
const GRAB_HOTKEY_CODE: Code = Code::KeyL;

/// Human-readable rendering of the grab hotkey for the in-window hint.
const GRAB_HOTKEY_LABEL: &str = if cfg!(target_os = "macos") {
    "Ctrl+Shift+Option+L"
} else {
    "Ctrl+Shift+Alt+L"
};

/// Three modifiers plus a letter is intentionally unusual: this is meant
/// to be press-by-mistake-proof, since the entire point of the viewer is
/// to stay out of the user's way.
fn build_grab_hotkey() -> HotKey {
    let mods = Modifiers::CONTROL | Modifiers::SHIFT | Modifiers::ALT;
    HotKey::new(Some(mods), GRAB_HOTKEY_CODE)
}

/// Shared state between the stdin reader thread and the eframe app.
struct LogState {
    messages: Vec<String>,
    /// Set by either `FromParent::Close` or stdin EOF (parent died). The
    /// eframe `update` loop sends `ViewportCommand::Close` next frame.
    should_close: bool,
    /// Tracks whether the macOS screenshot-exclusion call has run yet.
    /// Window handles only become available after the first frame.
    initialized: bool,
}

/// Configure the macOS NSWindow to be excluded from screen capture
/// (`NSWindowSharingNone = 0`). No-op on other platforms.
#[cfg(target_os = "macos")]
fn setup_macos_window() {
    use objc2_app_kit::NSApplication;
    use objc2_foundation::MainThreadMarker;

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    for window in app.windows() {
        unsafe {
            let _: () = objc2::msg_send![&window, setSharingType: 0u64];
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn setup_macos_window() {}

fn main() -> eframe::Result<()> {
    let state = Arc::new(Mutex::new(LogState {
        messages: Vec::new(),
        should_close: false,
        initialized: false,
    }));

    // Stdin reader: parses FromParent messages and updates shared state.
    // On EOF (parent died), sets `should_close = true` so the eframe loop
    // exits cleanly next frame.
    let state_for_stdin = state.clone();
    thread::spawn(move || {
        let stdin = std::io::stdin();
        let reader = BufReader::new(stdin.lock());
        for line in reader.lines() {
            let Ok(line) = line else { break };
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Ok(msg) = serde_json::from_str::<FromParent>(trimmed) else {
                continue;
            };
            if let Ok(mut guard) = state_for_stdin.lock() {
                match msg {
                    FromParent::Log { message } => guard.messages.push(message),
                    FromParent::Clear => guard.messages.clear(),
                    FromParent::Close => {
                        guard.should_close = true;
                        break;
                    }
                }
            }
        }
        if let Ok(mut guard) = state_for_stdin.lock() {
            guard.should_close = true;
        }
    });

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([500.0, 280.0])
            .with_position([40.0, 40.0])
            .with_always_on_top()
            .with_decorations(true)
            .with_transparent(true)
            .with_mouse_passthrough(true), // click-through by default
        ..Default::default()
    };

    eframe::run_native(
        "Simulang Log Viewer",
        options,
        Box::new(|cc| {
            let mut visuals = eframe::egui::Visuals::light();
            visuals.window_fill = eframe::egui::Color32::from_rgba_unmultiplied(255, 255, 255, 230);
            visuals.panel_fill = eframe::egui::Color32::from_rgba_unmultiplied(255, 255, 255, 230);
            cc.egui_ctx.set_visuals(visuals);

            // Register the global grab hotkey on the main thread (required
            // on macOS, where the Carbon hotkey API binds to the main
            // run-loop). The manager is stashed on the app struct so its
            // Drop runs on shutdown and unregisters the hotkey.
            let hotkey_manager = GlobalHotKeyManager::new().map_err(Box::new)?;
            let grab_hotkey = build_grab_hotkey();
            hotkey_manager.register(grab_hotkey).map_err(Box::new)?;

            Ok(Box::new(LogViewerApp {
                state,
                _hotkey_manager: hotkey_manager,
                grab_hotkey_id: grab_hotkey.id(),
                is_grabbed: false,
                last_paused_sent: false,
            }))
        }),
    )
}

struct LogViewerApp {
    state: Arc<Mutex<LogState>>,
    /// Owns the OS hotkey registration. Dropped at shutdown so the
    /// hotkey is cleanly unregistered.
    _hotkey_manager: GlobalHotKeyManager,
    /// ID of the registered grab hotkey, used to filter incoming events
    /// against any other future hotkeys.
    grab_hotkey_id: u32,
    /// True while the user has grabbed the window: passthrough is off,
    /// the title bar is draggable, and the parent is paused.
    is_grabbed: bool,
    /// Last `paused` value we wrote to stdout. We only emit a
    /// `FromChild::State` when this changes, so the parent doesn't get a
    /// flood of identical events on every frame.
    last_paused_sent: bool,
}

impl LogViewerApp {
    /// Drain any pending global-hotkey events. Returns `true` if our
    /// grab hotkey was pressed (key-down) at least once during this
    /// frame; multiple presses in one frame collapse to a single toggle.
    fn poll_grab_hotkey(&self) -> bool {
        let mut toggled = false;
        let receiver = GlobalHotKeyEvent::receiver();
        while let Ok(event) = receiver.try_recv() {
            if event.id == self.grab_hotkey_id && event.state == HotKeyState::Pressed {
                toggled = true;
            }
        }
        toggled
    }
}

impl eframe::App for LogViewerApp {
    fn logic(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(50));

        // First frame: install screenshot-exclusion. Window handles aren't
        // available before the first frame, so we can't do this in `main`.
        if let Ok(mut guard) = self.state.lock()
            && !guard.initialized
        {
            guard.initialized = true;
            drop(guard);
            setup_macos_window();
        }

        // Close on `FromParent::Close` or stdin EOF.
        if let Ok(guard) = self.state.lock()
            && guard.should_close
        {
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
            return;
        }

        if self.poll_grab_hotkey() {
            self.is_grabbed = !self.is_grabbed;
            // Passthrough is the inverse of grabbed: grabbed = interactive.
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::MousePassthrough(
                !self.is_grabbed,
            ));
            // Bring the window forward when the user grabs it so the
            // native title bar is immediately ready to drag.
            if self.is_grabbed {
                ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Focus);
            }
        }

        // Effective paused state mirrors grab mode: grab = pause. Emit
        // `FromChild::State` to the parent on transitions.
        let effective_paused = self.is_grabbed;
        if effective_paused != self.last_paused_sent {
            self.last_paused_sent = effective_paused;
            let line = serde_json::to_string(&FromChild::State {
                paused: effective_paused,
            })
            .unwrap_or_default();
            let mut stdout = std::io::stdout().lock();
            let _ = stdout.write_all(line.as_bytes());
            let _ = stdout.write_all(b"\n");
            let _ = stdout.flush();
        }
    }

    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        let bg_alpha: u8 = if self.is_grabbed { 250 } else { 200 };
        let bg_color = eframe::egui::Color32::from_rgba_unmultiplied(255, 255, 255, bg_alpha);
        let content_bg = eframe::egui::Color32::from_rgba_unmultiplied(245, 245, 245, bg_alpha);

        eframe::egui::CentralPanel::default()
            .frame(eframe::egui::Frame::new().fill(bg_color).inner_margin(12.0))
            .show_inside(ui, |ui| {
                // Title bar.
                ui.horizontal(|ui| {
                    ui.label(
                        eframe::egui::RichText::new("Simulang")
                            .size(14.0)
                            .color(eframe::egui::Color32::from_rgb(60, 60, 60)),
                    );
                    ui.with_layout(
                        eframe::egui::Layout::right_to_left(eframe::egui::Align::Center),
                        |ui| {
                            if self.is_grabbed {
                                ui.label(
                                    eframe::egui::RichText::new(format!(
                                        "GRABBED — drag to move • {GRAB_HOTKEY_LABEL} to release"
                                    ))
                                    .size(11.0)
                                    .strong()
                                    .color(eframe::egui::Color32::from_rgb(140, 70, 0)),
                                );
                            } else {
                                // Name the surprising behavior — clicks
                                // pass through — rather than the parent's
                                // run state, so a first-time user isn't
                                // baffled by the window seeming to ignore
                                // their mouse.
                                ui.label(
                                    eframe::egui::RichText::new(format!(
                                        "CLICKS PASS THROUGH — {GRAB_HOTKEY_LABEL} to grab & pause"
                                    ))
                                    .size(11.0)
                                    .strong()
                                    .color(eframe::egui::Color32::from_rgb(30, 30, 30)),
                                );
                            }
                        },
                    );
                });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);

                // Log content area.
                let content_frame = eframe::egui::Frame::new()
                    .fill(content_bg)
                    .corner_radius(8.0)
                    .inner_margin(12.0);

                content_frame.show(ui, |ui| {
                    eframe::egui::ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if let Ok(guard) = self.state.lock() {
                                for msg in &guard.messages {
                                    ui.label(
                                        eframe::egui::RichText::new(msg)
                                            .monospace()
                                            .size(12.0)
                                            .color(eframe::egui::Color32::from_rgb(30, 30, 30)),
                                    );
                                }
                                // Blinking cursor, anchored at the end of the log so the
                                // viewport doesn't jump on every blink.
                                let cursor_visible =
                                    (ui.ctx().input(|i| i.time) * 2.0) as i32 % 2 == 0;
                                let cursor_color = if cursor_visible {
                                    eframe::egui::Color32::from_rgb(100, 100, 100)
                                } else {
                                    eframe::egui::Color32::TRANSPARENT
                                };
                                ui.label(
                                    eframe::egui::RichText::new("|")
                                        .monospace()
                                        .size(12.0)
                                        .color(cursor_color),
                                );
                            }
                        });
                });
            });
    }
}
