//! vizgres - A fast, keyboard-driven PostgreSQL client for the terminal

#![allow(dead_code)]

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::*;
use tokio::sync::mpsc;

mod app;
mod commands;
mod config;
mod db;
mod error;
mod ui;

use app::{Action, App, AppEvent};

/// A fast, keyboard-driven PostgreSQL client for the terminal
#[derive(Parser)]
#[command(name = "vizgres", version, about)]
struct Cli {
    /// PostgreSQL connection URL (postgres://user:pass@host:port/dbname)
    url: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Set up panic hook to restore terminal before panic message
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stderr(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the app (separated so we can always clean up)
    let result = run_app(&mut terminal, cli).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    cli: Cli,
) -> Result<()> {
    let mut app = App::new();

    // Channel for async events (db results, etc.)
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AppEvent>();

    // If a URL was provided on the command line, connect immediately
    if let Some(ref url) = cli.url {
        match config::ConnectionConfig::from_url(url) {
            Ok(config) => {
                let name = config.name.clone();
                app.set_status(format!("Connecting to {}...", name), app::StatusLevel::Info);

                match db::PostgresProvider::connect(&config).await {
                    Ok(provider) => {
                        app.connection = Some(provider);
                        app.connection_name = Some(name.clone());
                        app.set_status(format!("Connected to {}", name), app::StatusLevel::Success);

                        // Load schema immediately
                        if let Some(ref mut conn) = app.connection {
                            match conn.get_schema().await {
                                Ok(schema) => {
                                    app.tree_browser.set_schema(schema);
                                    app.set_status(
                                        "Schema loaded".to_string(),
                                        app::StatusLevel::Info,
                                    );
                                }
                                Err(e) => {
                                    app.set_status(
                                        format!("Schema load failed: {}", e),
                                        app::StatusLevel::Error,
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        app.set_status(
                            format!("Connection failed: {}", e),
                            app::StatusLevel::Error,
                        );
                    }
                }
            }
            Err(e) => {
                app.set_status(format!("Invalid URL: {}", e), app::StatusLevel::Error);
            }
        }
    }

    // Main event loop
    loop {
        // Draw
        terminal.draw(|frame| {
            ui::render::render(frame, &app);
        })?;

        // Clear status after timeout
        if app.should_clear_status() {
            app.status_message = None;
        }

        // Poll for events
        let action = tokio::select! {
            // Async events from spawned tasks
            Some(event) = event_rx.recv() => {
                app.handle_event(event)?
            }

            // Check for terminal input using a small timeout
            result = tokio::task::spawn_blocking(|| {
                if event::poll(std::time::Duration::from_millis(50)).unwrap_or(false) {
                    Some(event::read().ok())
                } else {
                    None
                }
            }) => {
                match result {
                    Ok(Some(Some(Event::Key(key)))) => {
                        // Only handle key press events (not release/repeat)
                        if key.kind == KeyEventKind::Press {
                            app.handle_event(AppEvent::Key(key))?
                        } else {
                            Action::None
                        }
                    }
                    Ok(Some(Some(Event::Resize(w, h)))) => {
                        app.handle_event(AppEvent::Resize(w, h))?
                    }
                    _ => Action::None,
                }
            }
        };

        // Execute actions
        match action {
            Action::Quit => {
                app.running = false;
                break;
            }
            Action::ExecuteQuery(sql) => {
                if let Some(ref conn) = app.connection {
                    let tx = event_tx.clone();
                    match conn.execute_query(&sql).await {
                        Ok(results) => {
                            let _ = tx.send(AppEvent::QueryCompleted(results));
                        }
                        Err(e) => {
                            let _ = tx.send(AppEvent::QueryFailed(e.to_string()));
                        }
                    }
                } else {
                    app.set_status(
                        "Not connected to a database".to_string(),
                        app::StatusLevel::Error,
                    );
                }
            }
            Action::Connect(config) => {
                let name = config.name.clone();
                match db::PostgresProvider::connect(&config).await {
                    Ok(provider) => {
                        app.connection = Some(provider);
                        app.connection_name = Some(name.clone());
                        app.set_status(format!("Connected to {}", name), app::StatusLevel::Success);

                        // Auto-load schema
                        if let Some(ref mut conn) = app.connection {
                            match conn.get_schema().await {
                                Ok(schema) => {
                                    app.tree_browser.set_schema(schema);
                                    app.set_status(
                                        "Schema loaded".to_string(),
                                        app::StatusLevel::Info,
                                    );
                                }
                                Err(e) => {
                                    app.set_status(
                                        format!("Schema load failed: {}", e),
                                        app::StatusLevel::Error,
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        app.set_status(
                            format!("Connection failed: {}", e),
                            app::StatusLevel::Error,
                        );
                    }
                }
            }
            Action::Disconnect => {
                app.connection = None;
                app.connection_name = None;
            }
            Action::LoadSchema => {
                if let Some(ref mut conn) = app.connection {
                    conn.invalidate_cache();
                    match conn.get_schema().await {
                        Ok(schema) => {
                            app.tree_browser.set_schema(schema);
                            app.set_status("Schema loaded".to_string(), app::StatusLevel::Info);
                        }
                        Err(e) => {
                            app.set_status(
                                format!("Schema load failed: {}", e),
                                app::StatusLevel::Error,
                            );
                        }
                    }
                }
            }
            Action::None => {}
        }
    }

    Ok(())
}
