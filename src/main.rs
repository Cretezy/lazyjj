#![feature(let_chains)]

extern crate anyhow;
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
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
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
    app::{App, Tab},
    commander::Commander,
    env::Env,
    ui::ui,
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
    if let Err(err) = Command::new("jj").arg("help").output()
        && let ErrorKind::NotFound = err.kind()
    {
        bail!("jj command not found. Please make sure it is installed: https://martinvonz.github.io/jj/latest/install-and-setup");
    }

    // Setup environment
    let env = Env::new(path)?;
    let mut commander = Commander::new(&env);

    // Check that `jj status` works
    commander.init()?;

    // Setup app
    let mut app = App::new(env.clone(), &mut commander)?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    run_app(&mut terminal, &mut app, &mut commander)?;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    commander: &mut Commander,
) -> Result<()> {
    // Loop for: Update -> Draw -> Input
    loop {
        {
            // Update current tab
            if let Some(component_action) = app.get_current_component_mut().update(commander)? {
                app.handle_action(component_action, commander)?;
            }

            // Draw
            terminal.draw(|f| ui(f, app).unwrap())?;

            // Input
            let event = event::read()?;

            // Pass through if textarea is active
            // Note: This could be refactor such that the event handling in the else block only
            // runs if nothing is returned from the current tab's handler.
            if app.textarea_active {
                if let Some(component_action) =
                    app.get_current_component_mut().input(commander, event)?
                {
                    app.handle_action(component_action, commander)?;
                }
            } else if let Event::Key(key) = event {
                // Skip events that are not KeyEventKind::Press
                if key.kind == event::KeyEventKind::Release {
                    continue;
                }

                // Close
                if key.code == KeyCode::Char('q')
                    || (key.modifiers.contains(KeyModifiers::CONTROL)
                        && (key.code == KeyCode::Char('c')))
                    || key.code == KeyCode::Esc
                {
                    return Ok(());
                }

                // Tab switching
                if let Some((_, tab)) = Tab::VALUES.iter().enumerate().find(|(i, _)| {
                    key.code
                        == KeyCode::Char(
                            char::from_digit((*i as u32) + 1u32, 10)
                                .expect("Tab index could not be converted to digit"),
                        )
                }) {
                    app.current_tab = *tab;
                    app.get_current_component_mut().reset(commander)?;
                    continue;
                }

                // Current tab input handling
                if let Some(component_action) =
                    app.get_current_component_mut().input(commander, event)?
                {
                    app.handle_action(component_action, commander)?;
                }
            }
        }
    }
}
