extern crate thiserror;

use std::{
    env::current_dir,
    fs::{canonicalize, OpenOptions},
    io::{self, ErrorKind},
    process::Command,
    time::{Duration, Instant},
};

use anyhow::{bail, Context, Result};
use clap::Parser;
use ratatui::{
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::{
            self, DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture,
            KeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
        },
        execute,
        terminal::{
            disable_raw_mode, enable_raw_mode, supports_keyboard_enhancement, EnterAlternateScreen,
            LeaveAlternateScreen,
        },
    },
    layout::{Alignment, Rect},
    widgets::Paragraph,
    Terminal,
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
    ui::{ui, ComponentAction},
};

/// Command line arguments
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
    if let Err(err) = Command::new(&jj_bin).arg("help").output() {
        if err.kind() == ErrorKind::NotFound {
            bail!("jj command not found. Please make sure it is installed: https://martinvonz.github.io/jj/latest/install-and-setup");
        }
    }

    // Setup environment
    let env = Env::new(path, args.revisions, jj_bin)?;
    let mut commander = Commander::new(&env);

    // Check that `jj status` works
    commander.init()?;

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
    loop {
        // Draw
        draw_app(app, terminal, commander, &start_time)?;

        // Input
        start_time = Instant::now();

        let should_stop = input_to_app(app, commander)?;

        if should_stop {
            return Ok(());
        }
    }
}

fn draw_app<B: Backend>(
    app: &mut App,
    terminal: &mut Terminal<B>,
    commander: &mut Commander,
    start_time: &Instant,
) -> Result<()> {
    let mut terminal_draw_res = Ok(());
    terminal.draw(|f| {
        // Update current tab
        let update_span = trace_span!("update");
        terminal_draw_res = update_span.in_scope(|| -> Result<()> {
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
                let paragraph = Paragraph::new(format!("{}ms", start_time.elapsed().as_millis()))
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
    terminal_draw_res
}

/// Let app process all input events in queue before returning
/// Return true if application should stop
fn input_to_app(app: &mut App, commander: &mut Commander) -> Result<bool> {
    let input_spawn = trace_span!("input");
    let mut should_stop: bool = false;
    while event::poll(Duration::ZERO)? && !should_stop {
        let event = event::read()?;
        should_stop = input_spawn.in_scope(|| app.input(event, commander))?;
    }
    Ok(should_stop)
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
    execute!(
        io::stdout(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        DisableFocusChange
    )?;
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
