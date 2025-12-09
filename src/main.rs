extern crate thiserror;

use std::{
    env::current_dir,
    fs::{OpenOptions, canonicalize},
    io::{self, ErrorKind},
    process::Command,
    time::Instant,
};

use anyhow::{Context, Result, bail};
use clap::Parser;
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::{
            self, DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture,
            Event, KeyboardEnhancementFlags, MouseEvent, MouseEventKind,
            PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
        },
        execute,
        terminal::{
            EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
            supports_keyboard_enhancement,
        },
    },
    layout::{Alignment, Rect},
    widgets::Paragraph,
};
use tracing::{info, trace_span};
use tracing_chrome::ChromeLayerBuilder;
use tracing_subscriber::layer::SubscriberExt;

mod app;
mod commander;
mod env;
mod keybinds;
mod ui;

use crate::{
    app::App,
    commander::Commander,
    env::Env,
    ui::{ComponentAction, ui},
};

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to jj repo. Defaults to current directory
    #[arg(short, long)]
    path: Option<String>,

    /// Default revset
    #[arg(short, long)]
    revisions: Option<String>,

    /// Path to jj binary
    #[arg(long, env = "JJ_BIN")]
    jj_bin: Option<String>,

    /// Do not exit if jj version check fails
    #[arg(long)]
    ignore_jj_version: bool,
}

fn main() -> Result<()> {
    let should_log = std::env::var("LAZYJJ_LOG")
        .map(|log| log == "1" || log.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let log_layer = if should_log {
        let log_file = OpenOptions::new()
            .append(true)
            .create(true)
            .open("lazyjj.log")
            .unwrap();

        Some(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_writer(log_file)
                // Add log when span ends with their duration
                .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE),
        )
    } else {
        None
    };

    let should_trace = std::env::var("LAZYJJ_TRACE")
        .map(|log| log == "1" || log.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let (trace_layer, _guard) = if should_trace {
        let (chrome_layer, _guard) = ChromeLayerBuilder::new().build();
        (Some(chrome_layer), Some(_guard))
    } else {
        (None, None)
    };

    let subscriber = tracing_subscriber::Registry::default()
        .with(log_layer)
        .with(trace_layer);
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting lazyjj");

    // Parse arguments and determine path
    let args = Args::parse();
    let path = match args.path {
        Some(path) => {
            canonicalize(&path).with_context(|| format!("Could not find path {}", &path))?
        }
        None => current_dir()?,
    };

    let jj_bin = args.jj_bin.unwrap_or("jj".to_string());

    // Check that jj exists
    if let Err(err) = Command::new(&jj_bin).arg("help").output()
        && err.kind() == ErrorKind::NotFound
    {
        bail!(
            "jj command not found. Please make sure it is installed: https://martinvonz.github.io/jj/latest/install-and-setup"
        );
    }

    // Setup environment
    let env = Env::new(path, args.revisions, jj_bin)?;
    let mut commander = Commander::new(&env);

    if !args.ignore_jj_version {
        commander.check_jj_version()?;
    }

    // Setup app
    let mut app = App::new(env.clone())?;

    let mut terminal = setup_terminal()?;
    install_panic_hook();

    // Run app
    let res = run_app(&mut terminal, &mut app, &mut commander);
    restore_terminal()?;
    res?;

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    commander: &mut Commander,
) -> Result<()> {
    let mut start_time = Instant::now();
    let mut drawing_popup = false;
    loop {
        // Draw
        let mut terminal_draw_res = Ok(());
        terminal.draw(|f| {
            // Update current tab
            let update_span = trace_span!("update");
            terminal_draw_res = update_span.in_scope(|| -> Result<()> {
                // Update popup if present
                if let Some(popup) = app.popup.as_mut() {
                    if let Some(component_action) = popup.update(commander)? {
                        app.handle_action(component_action, commander)?;
                    }
                    drawing_popup = true;
                } else {
                    drawing_popup = false;
                }

                // Update current tab
                if let Some(component_action) =
                    app.get_or_init_current_tab(commander)?.update(commander)?
                {
                    app.handle_action(component_action, commander)?;
                }

                Ok(())
            });
            if terminal_draw_res.is_err() {
                return;
            }

            let draw_span = trace_span!("draw");
            terminal_draw_res = draw_span.in_scope(|| -> Result<()> {
                ui(f, app)?;

                {
                    let paragraph =
                        Paragraph::new(format!("{}ms", start_time.elapsed().as_millis()))
                            .alignment(Alignment::Right);
                    let position = Rect {
                        x: 0,
                        y: 1,
                        height: 1,
                        width: f.area().width - 1,
                    };
                    f.render_widget(paragraph, position);
                }
                Ok(())
            });
        })?;
        terminal_draw_res?;

        // Input
        let input_spawn = trace_span!("input");

        // Poll for events with a timeout to enable animations
        if !drawing_popup || event::poll(std::time::Duration::from_millis(100))? {
            let event = loop {
                match event::read()? {
                    event::Event::FocusLost => continue,
                    Event::Mouse(MouseEvent {
                        kind: MouseEventKind::Moved,
                        ..
                    }) => continue,
                    event => break event,
                }
            };

            start_time = Instant::now();

            let should_stop = input_spawn.in_scope(|| -> Result<bool> {
                if app.input(event, commander)? {
                    return Ok(true);
                }

                Ok(false)
            })?;

            if should_stop {
                return Ok(());
            }
        }
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableFocusChange
    )?;

    if supports_keyboard_enhancement()? {
        execute!(
            stdout,
            // required to properly detect ctrl+shift
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )?;
    }

    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        LeaveAlternateScreen,
        DisableMouseCapture,
        DisableFocusChange
    )?;

    if supports_keyboard_enhancement()? {
        execute!(stdout, PopKeyboardEnhancementFlags)?;
    }

    Ok(())
}

fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Err(err) = restore_terminal() {
            eprintln!("Failed to restore terminal: {err}");
        }
        original_hook(info);
    }));
}

enum ComponentInputResult {
    Handled,
    HandledAction(ComponentAction),
    NotHandled,
}

impl ComponentInputResult {
    pub fn is_handled(&self) -> bool {
        match self {
            ComponentInputResult::Handled => true,
            ComponentInputResult::HandledAction(_) => true,
            ComponentInputResult::NotHandled => false,
        }
    }
}
