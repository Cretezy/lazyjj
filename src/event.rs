//! Event handling for lazyjj.
//!
//! This module provides an event source that combines:
//! - User input events (keyboard, mouse) from crossterm
//! - File system events from notify (watching .jj/repo/op_heads/heads)
//!
//! The file watcher triggers a refresh when jj operations complete,
//! enabling real-time updates when working with multiple terminals
//! or when LLMs make changes to the repository.

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};

use notify::{RecursiveMode, Watcher};
use ratatui::crossterm;
use tracing::{error, trace};

/// Minimum time between idle events (reduces CPU when nothing is happening)
const IDLE_TIMEOUT: Duration = Duration::from_secs(1);

/// Debounce delay for filesystem events.
/// jj operations may touch multiple files rapidly, so we wait for things to settle.
/// 250ms works well for LLMs making rapid changes.
const NOTIFY_DEBOUNCE: Duration = Duration::from_millis(250);

/// Events that can be sent to the application
#[derive(Debug, Clone, PartialEq)]
pub enum AppEvent {
    /// User input from keyboard or mouse
    Input(crossterm::event::Event),
    /// The jj repository state changed (operation completed)
    RepoChanged,
}

/// Manages event sources and provides a unified event stream
pub struct EventSource {
    /// Channel for receiving events
    rx: mpsc::Receiver<AppEvent>,
    /// Channel for sending events (cloned to background threads)
    tx: mpsc::Sender<AppEvent>,
    /// File watcher handle (kept alive)
    _watcher: Option<notify::RecommendedWatcher>,
    /// Flag to temporarily disable watcher events during our own operations
    watcher_enabled: Arc<AtomicBool>,
    /// Track when we last received an event for debouncing
    last_event_time: Instant,
    /// Track if last recv returned None (for idle timeout)
    was_idle: bool,
}

impl EventSource {
    /// Create a new event source
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            rx,
            tx,
            _watcher: None,
            watcher_enabled: Arc::new(AtomicBool::new(true)),
            last_event_time: Instant::now(),
            was_idle: false,
        }
    }

    /// Start the user input listener thread
    pub fn start_input_listener(&self) {
        let tx = self.tx.clone();
        thread::Builder::new()
            .name("input-listener".to_string())
            .spawn(move || {
                trace!("input listener started");
                loop {
                    match crossterm::event::read() {
                        Ok(event) => {
                            if tx.send(AppEvent::Input(event)).is_err() {
                                break; // Channel closed
                            }
                        }
                        Err(e) => {
                            error!("input read error: {:?}", e);
                            break;
                        }
                    }
                }
                trace!("input listener stopped");
            })
            .expect("failed to spawn input listener thread");
    }

    /// Start watching the jj repository for changes
    pub fn start_repo_watcher(&mut self, repo_root: PathBuf) {
        let op_heads_dir = repo_root
            .join(".jj")
            .join("repo")
            .join("op_heads")
            .join("heads");

        // Create channel for notify events
        let (notify_tx, notify_rx) = mpsc::channel();

        // Create watcher
        let watcher = match notify::recommended_watcher(notify_tx) {
            Ok(w) => w,
            Err(e) => {
                error!("failed to create file watcher: {:?}", e);
                return;
            }
        };

        // Store watcher to keep it alive
        self._watcher = Some(watcher);

        // Start watching (get mutable reference after storing)
        if let Some(ref mut watcher) = self._watcher {
            if let Err(e) = watcher.watch(&op_heads_dir, RecursiveMode::NonRecursive) {
                error!("failed to watch {:?}: {:?}", op_heads_dir, e);
                self._watcher = None;
                return;
            }
        }

        // Spawn thread to forward notify events
        let tx = self.tx.clone();
        let watcher_enabled = self.watcher_enabled.clone();
        thread::Builder::new()
            .name("repo-watcher".to_string())
            .spawn(move || {
                trace!("repo watcher started, watching {:?}", op_heads_dir);
                for _event in notify_rx {
                    // Only forward if watcher is enabled
                    if watcher_enabled.load(Ordering::Relaxed) {
                        if tx.send(AppEvent::RepoChanged).is_err() {
                            break; // Channel closed
                        }
                        trace!("repo change event forwarded");
                    } else {
                        trace!("repo change event ignored (watcher disabled)");
                    }
                }
                trace!("repo watcher stopped");
            })
            .expect("failed to spawn repo watcher thread");
    }

    /// Temporarily disable the repo watcher (e.g., during our own jj operations)
    pub fn disable_watcher(&self) {
        self.watcher_enabled.store(false, Ordering::Relaxed);
    }

    /// Re-enable the repo watcher
    pub fn enable_watcher(&self) {
        self.watcher_enabled.store(true, Ordering::Relaxed);
    }

    /// Try to receive an event.
    /// Returns Some(event) if an event is available, None for idle timeout.
    /// Handles debouncing of rapid RepoChanged events.
    pub fn try_recv(&mut self) -> Option<AppEvent> {
        // Use longer timeout if we were idle (reduces CPU usage)
        let timeout = if self.was_idle {
            IDLE_TIMEOUT
        } else {
            Duration::ZERO
        };

        // Enable watcher while waiting for events
        self.enable_watcher();

        let result = loop {
            match self.rx.recv_timeout(timeout) {
                Ok(event) => {
                    // Debounce rapid RepoChanged events
                    if event == AppEvent::RepoChanged {
                        if self.last_event_time.elapsed() < NOTIFY_DEBOUNCE {
                            trace!("debouncing RepoChanged event");
                            continue;
                        }
                    }
                    break Some(event);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => break None,
                Err(mpsc::RecvTimeoutError::Disconnected) => break None,
            }
        };

        // Disable watcher while processing
        self.disable_watcher();

        // Update state
        if let Some(ref event) = result {
            self.last_event_time = Instant::now();
            self.was_idle = false;
            trace!("received event: {:?}", event);
        } else {
            self.was_idle = true;
        }

        result
    }
}

impl Default for EventSource {
    fn default() -> Self {
        Self::new()
    }
}
