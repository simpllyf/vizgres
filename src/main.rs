//! vizgres - A fast, keyboard-driven PostgreSQL client for the terminal

use std::sync::Arc;

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
mod history;
mod keymap;
mod ui;

use app::{Action, App, AppEvent};
use db::Database;

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

    // Get connection URL: from CLI arg or prompt interactively
    let url = match cli.url {
        Some(u) => u,
        None => prompt_for_url()?,
    };

    // Parse and validate the URL before entering TUI
    let conn_config = config::ConnectionConfig::from_url(&url)
        .map_err(|e| anyhow::anyhow!("Invalid connection URL: {}", e))?;

    // Connect to the database before entering TUI
    eprintln!("Connecting to {}...", conn_config.name);
    let (provider, conn_err_rx) = db::PostgresProvider::connect(&conn_config)
        .await
        .map_err(|e| anyhow::anyhow!("Connection failed: {}", e))?;
    let provider = Arc::new(provider);

    // Load schema
    let schema = provider
        .get_schema()
        .await
        .map_err(|e| anyhow::anyhow!("Schema load failed: {}", e))?;

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
    let result = run_app(
        &mut terminal,
        provider,
        conn_config.name,
        schema,
        conn_err_rx,
    )
    .await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

/// Prompt the user for a connection URL on stdin (before TUI starts)
fn prompt_for_url() -> Result<String> {
    use std::io::Write;
    eprint!("PostgreSQL URL: ");
    std::io::stderr().flush()?;
    let mut url = String::new();
    std::io::stdin().read_line(&mut url)?;
    let url = url.trim().to_string();
    if url.is_empty() {
        anyhow::bail!("No connection URL provided. Usage: vizgres <postgres://...>");
    }
    Ok(url)
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    provider: Arc<db::PostgresProvider>,
    connection_name: String,
    schema: db::schema::SchemaTree,
    mut conn_err_rx: mpsc::UnboundedReceiver<String>,
) -> Result<()> {
    let mut app = App::with_connection(connection_name, schema);

    // Channel for async events (db results, etc.)
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AppEvent>();

    // Main event loop
    loop {
        // Draw
        terminal.draw(|frame| {
            ui::render::render(frame, &app);
        })?;

        // Poll for events
        let action = tokio::select! {
            // Async events from spawned tasks
            Some(event) = event_rx.recv() => {
                app.handle_event(event)?
            }

            // Background connection died (server restart, idle timeout, etc.)
            Some(msg) = conn_err_rx.recv() => {
                app.handle_event(AppEvent::ConnectionLost(msg))?
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
                        if key.kind == KeyEventKind::Press {
                            app.handle_event(AppEvent::Key(key))?
                        } else {
                            Action::None
                        }
                    }
                    Ok(Some(Some(Event::Resize(_, _)))) => {
                        app.handle_event(AppEvent::Resize)?
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
                app.query_running = true;
                let db = Arc::clone(&provider);
                let tx = event_tx.clone();
                tokio::spawn(async move {
                    match db.execute_query(&sql).await {
                        Ok(results) => {
                            let _ = tx.send(AppEvent::QueryCompleted(results));
                        }
                        Err(e) => {
                            let _ = tx.send(AppEvent::QueryFailed(e.to_string()));
                        }
                    }
                });
            }
            Action::CancelQuery => {
                let db = Arc::clone(&provider);
                tokio::spawn(async move {
                    let _ = db.cancel_query().await;
                });
            }
            Action::LoadSchema => {
                let db = Arc::clone(&provider);
                let tx = event_tx.clone();
                tokio::spawn(async move {
                    match db.get_schema().await {
                        Ok(schema) => {
                            let _ = tx.send(AppEvent::SchemaLoaded(schema));
                        }
                        Err(e) => {
                            let _ = tx.send(AppEvent::SchemaFailed(e.to_string()));
                        }
                    }
                });
            }
            Action::None => {}
        }
    }

    Ok(())
}
