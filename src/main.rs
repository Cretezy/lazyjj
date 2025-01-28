extern crate thiserror;

use std::{
    env::current_dir,
    fs::{canonicalize, OpenOptions},
    io::{self, ErrorKind},
    process::Command,
};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, MouseEvent, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
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
mod ui;

use crate::{
    app::App,
    commander::Commander,
    env::Env,
    ui::{ui, ComponentAction},
};

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to jj repo. Defaults to current directory
    #[arg(short, long)]
    path: Option<String>,

    // Default revset
    #[arg(short, long)]
    revisions: Option<String>,
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

    // Check that jj exists
    if let Err(err) = Command::new("jj").arg("help").output() {
        if err.kind() == ErrorKind::NotFound {
            bail!("jj command not found. Please make sure it is installed: https://martinvonz.github.io/jj/latest/install-and-setup");
        }
    }

    // Setup environment
    let env = Env::new(path, args.revisions)?;
    let mut commander = Commander::new(&env);

    // Check that `jj status` works
    commander.init()?;

    // Setup app
    let mut app = App::new(env.clone())?;

    let mut terminal = setup_terminal()?;

    // Run app
    let res = run_app(&mut terminal, &mut app, &mut commander);
    restore_terminal(terminal)?;
    res?;

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    commander: &mut Commander,
) -> Result<()> {
    let mut start_time = Utc::now().time();
    loop {
        // Draw
        terminal.draw(|f| {
            // Update current tab
            let update_span = trace_span!("update");
            update_span
                .in_scope(|| -> Result<()> {
                    if let Some(component_action) =
                        app.get_or_init_current_tab(commander)?.update(commander)?
                    {
                        app.handle_action(component_action, commander)?;
                    }

                    Ok(())
                })
                .unwrap();

            let draw_span = trace_span!("draw");
            draw_span
                .in_scope(|| -> Result<()> {
                    ui(f, app).unwrap();

                    let end_time = Utc::now().time();
                    let diff = end_time - start_time;

                    {
                        let paragraph = Paragraph::new(format!("{}ms", diff.num_milliseconds()))
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
                })
                .unwrap();
        })?;

        start_time = Utc::now().time();

        // Input
        let input_spawn = trace_span!("input");
        let event = loop {
            match event::read()? {
                Event::Mouse(MouseEvent {
                    kind: MouseEventKind::Moved,
                    ..
                }) => continue,
                event => break event,
            }
        };
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

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(mut terminal: Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

enum ComponentInputResult {
    Handled,
    HandledAction(ComponentAction),
    NotHandled,
}
