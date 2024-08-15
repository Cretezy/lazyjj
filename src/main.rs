extern crate lazy_static;
extern crate thiserror;

use std::{
    env::current_dir,
    fs::canonicalize,
    io::{self, ErrorKind},
    process::Command,
};

use anyhow::{bail, Context, Result};
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};

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
}

fn main() -> Result<()> {
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
    let env = Env::new(path)?;
    let mut commander = Commander::new(&env);

    // Check that `jj status` works
    commander.init()?;

    // Setup app
    let mut app = App::new(env.clone(), &mut commander)?;

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
    loop {
        // Update current tab
        if let Some(component_action) = app.get_current_component_mut().update(commander)? {
            app.handle_action(component_action, commander)?;
        }

        // Draw
        terminal.draw(|f| ui(f, app).unwrap())?;

        // Input
        if app.input(event::read()?, commander)? {
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
