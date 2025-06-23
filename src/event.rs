//! The event module is where events are generated. This means
//! capture keyboard and mouse events from user, as well as listening
//! for file system notifications in case somebody used jj on this
//! repository.

use std::{
    path::PathBuf,
    sync::{mpsc, Arc, RwLock},
    thread,
    time::{Duration, Instant},
};

use notify::{RecursiveMode, Watcher};
use ratatui::crossterm;
use tracing::{error, trace};

/// Minimum time between idle-events
const IDLE_TIMEOUT: Duration = Duration::from_secs(1);

/// Minimum time between notify events and actually notifying the app
const NOTIFY_DELAY: Duration = Duration::from_millis(100);

/// Input event to the app
#[derive(PartialEq, Debug)]
pub enum AppEvent {
    /// Keyboard or mouse input from user
    UserInput(crossterm::event::Event),
    /// The .jj folder was touched, so the app must redraw
    DirtyJj,
}

/// Generator of events to the app
pub struct EventSource {
    // Channel for app events
    app_event_sender: mpsc::Sender<AppEvent>,
    app_event_receiver: mpsc::Receiver<AppEvent>,

    // Producer data
    repo_watcher: Option<notify::RecommendedWatcher>,
    enable_watcher: Arc<RwLock<bool>>,

    // Consumer data
    last_event_recv: Instant,
    last_event_none: bool,
}

impl EventSource {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            app_event_sender: tx,
            app_event_receiver: rx,
            repo_watcher: None,
            enable_watcher: Arc::new(RwLock::new(false)),
            last_event_recv: Instant::now(),
            last_event_none: false,
        }
    }

    /// Launch a user input event source
    pub fn launch_user_input(&mut self) {
        // Spawn user input thread
        let app_event_tx = self.app_event_sender.clone();
        trace!("spawn crossterm reader");
        thread::Builder::new()
            .name("crossterm reader".to_string())
            .spawn(move || {
                trace!("crossterm reader - started");
                loop {
                    // Block until an event arrives
                    let Ok(event) = crossterm::event::read() else {
                        error!("crossterm reader - read abort");
                        break;
                    };
                    // Send event to main thread
                    let Ok(_) = app_event_tx.send(AppEvent::UserInput(event)) else {
                        error!("crossterm reader - send abort");
                        break;
                    };
                    trace!("crossterm reader - event forwarded");
                }
                trace!("crossterm reader - stopped");
            })
            .unwrap();
    }

    /// Launch a file system watcher as event source
    pub fn launch_watcher(&mut self, jj_folder: PathBuf) {
        // The notify watcher uses a channel to send file system events
        let (tx, rx) = mpsc::channel();

        // Create a watcher attached to the channel
        let watch_result = notify::recommended_watcher(tx);
        let Ok(mut watcher) = watch_result else {
            let Err(e) = watch_result else {
                unreachable!();
            };
            error!("watcher abort on create: {:?}", e);
            return;
        };

        // Start watching
        if let Err(e) = watcher.watch(&jj_folder, RecursiveMode::Recursive) {
            error!("watcher abort on start: {:?}", e);
            return;
        }

        // Spawn thread that forwards events to main event loop
        trace!("spawn notify reader");
        let enable_watcher = self.enable_watcher.clone();
        let app_event_tx = self.app_event_sender.clone();
        thread::Builder::new()
            .name("notify reader".to_string())
            .spawn(move || {
                trace!("notify reader - started");
                loop {
                    // Block until an event arrives
                    let Ok(_event) = rx.recv() else {
                        error!("notify reader - abort on recv from watcher");
                        break;
                    };
                    // Ignore event that arrives when blocked
                    let enabled = *enable_watcher.read().unwrap();
                    if !enabled {
                        trace!("notify reader - event blocked");
                        continue;
                    }
                    // Send event to app
                    let Ok(_) = app_event_tx.send(AppEvent::DirtyJj) else {
                        error!("notify reader abort on send to app");
                        break;
                    };
                    trace!("notify reader - event forwarded");
                }
                trace!("notify reader - stopped");
            })
            .unwrap();

        // Store the watcher to keep it alive
        self.repo_watcher = Some(watcher);
    }

    /// Receive an AppEvent if one is waiting.
    /// If no event is waiting, it will return None which represents
    /// an idle event. There will be at least IDLE_TIMEOUT between two
    /// consecutive idle events. Ordinary events are returned immediately.
    pub fn try_recv(&mut self) -> Option<AppEvent> {
        // Introduce timeout if app is idle.
        // This will reduce CPU load
        let timeout = if self.last_event_none {
            IDLE_TIMEOUT
        } else {
            Duration::ZERO
        };

        // Get event
        let result = loop {
            // Wait for event. While waiting the watcher thread is allowed to
            // trigger a redraw.
            *self.enable_watcher.write().unwrap() = true;
            let result = self.app_event_receiver.recv_timeout(timeout);
            *self.enable_watcher.write().unwrap() = false;

            // Ignore notify events too soon after a user event
            // It was probably generated by the command that the user
            // asked for.
            if let Ok(ref event) = &result {
                if *event == AppEvent::DirtyJj && self.last_event_recv.elapsed() < NOTIFY_DELAY {
                    trace!("try_recv - ignore DirtyJj");
                    continue;
                }
            }
            break result;
        };

        // Check for app event
        if let Ok(event) = result {
            trace!("try_recv - received app event");
            self.last_event_recv = Instant::now();
            self.last_event_none = false;
            return Some(event);
        }

        // No event found. This will trigger a redraw in the main loop
        self.last_event_none = true;
        trace!("try_recv - no event");
        None
    }
}
