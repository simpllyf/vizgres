//! vizgres - A fast, keyboard-driven PostgreSQL client for the terminal

use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::*;
use tokio::sync::mpsc;
use vizgres::app::{Action, App, AppEvent, StatusLevel};
use vizgres::config;
use vizgres::db::{self, Database};

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
        let _ = execute!(
            std::io::stderr(),
            DisableBracketedPaste,
            LeaveAlternateScreen
        );
        original_hook(panic_info);
    }));

    // If URL provided, connect before entering TUI
    let (mut provider, mut conn_err_rx, mut app) = if let Some(ref url) = cli.url {
        let conn_config = config::ConnectionConfig::from_url(url)
            .map_err(|e| anyhow::anyhow!("Invalid connection URL: {}", e))?;

        eprintln!("Connecting to {}...", conn_config.name);
        let (prov, rx) = db::PostgresProvider::connect(&conn_config)
            .await
            .map_err(|e| anyhow::anyhow!("Connection failed: {}", e))?;
        let prov = Arc::new(prov);

        let schema = prov
            .get_schema()
            .await
            .map_err(|e| anyhow::anyhow!("Schema load failed: {}", e))?;

        let app = App::with_connection(conn_config.name.clone(), schema);
        (Some(prov), Some(rx), app)
    } else {
        // No URL — start disconnected and show connection dialog
        let mut app = App::new();
        app.show_connection_dialog();
        (None, None, app)
    };

    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the app (separated so we can always clean up)
    let result = run_app(&mut terminal, &mut app, &mut provider, &mut conn_err_rx).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableBracketedPaste,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    provider: &mut Option<Arc<db::PostgresProvider>>,
    conn_err_rx: &mut Option<mpsc::UnboundedReceiver<String>>,
) -> Result<()> {
    // Channel for async events (db results, etc.)
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AppEvent>();

    // Main event loop
    loop {
        // Draw
        terminal.draw(|frame| {
            vizgres::ui::render::render(frame, app);
        })?;

        // Poll for events
        let mut action = Action::None;
        tokio::select! {
            // Async events from spawned tasks
            Some(event) = event_rx.recv() => {
                action = app.handle_event(event)?;
            }

            // Background connection died (server restart, idle timeout, etc.)
            // Only poll when we have a receiver
            Some(msg) = async {
                match conn_err_rx.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                action = app.handle_event(AppEvent::ConnectionLost(msg))?;
            }

            // Check for terminal input; drain all buffered events before rendering
            result = tokio::task::spawn_blocking(|| {
                if event::poll(std::time::Duration::from_millis(50)).unwrap_or(false) {
                    let mut events = Vec::new();
                    while let Ok(ev) = event::read() {
                        events.push(ev);
                        if !event::poll(std::time::Duration::ZERO).unwrap_or(false) {
                            break;
                        }
                    }
                    Some(events)
                } else {
                    None
                }
            }) => {
                if let Ok(Some(events)) = result {
                    for ev in events {
                        let a = match ev {
                            Event::Key(key) if key.kind == KeyEventKind::Press => {
                                app.handle_event(AppEvent::Key(key))?
                            }
                            Event::Paste(data) => {
                                app.handle_event(AppEvent::Paste(data))?
                            }
                            Event::Resize(_, _) => {
                                app.handle_event(AppEvent::Resize)?
                            }
                            _ => Action::None,
                        };
                        if !matches!(a, Action::None) {
                            action = a;
                            break;
                        }
                    }
                }
            }
        }

        // Execute actions
        match action {
            Action::Quit => {
                app.running = false;
                break;
            }
            Action::Connect(config) => {
                // Show connecting status and render immediately
                app.set_status("Connecting...".to_string(), StatusLevel::Info);
                terminal.draw(|f| vizgres::ui::render::render(f, app))?;

                // Drop old provider
                *provider = None;
                *conn_err_rx = None;

                // Connect
                match db::PostgresProvider::connect(&config).await {
                    Ok((prov, rx)) => {
                        let prov = Arc::new(prov);
                        match prov.get_schema().await {
                            Ok(schema) => {
                                app.apply_connection(config.name.clone(), schema);
                                app.set_status(
                                    format!("Connected to {}", config.name),
                                    StatusLevel::Success,
                                );
                                *provider = Some(prov);
                                *conn_err_rx = Some(rx);
                            }
                            Err(e) => {
                                app.set_status(
                                    format!("Schema load failed: {}", e),
                                    StatusLevel::Error,
                                );
                            }
                        }
                    }
                    Err(e) => {
                        app.set_status(format!("Connection failed: {}", e), StatusLevel::Error);
                    }
                }
            }
            Action::ExecuteQuery { sql, tab_id } => {
                if let Some(prov) = provider.as_ref() {
                    let db = Arc::clone(prov);
                    let tx = event_tx.clone();
                    tokio::spawn(async move {
                        match db.execute_query(&sql).await {
                            Ok(results) => {
                                let _ = tx.send(AppEvent::QueryCompleted { results, tab_id });
                            }
                            Err(e) => {
                                let _ = tx.send(AppEvent::QueryFailed {
                                    error: e.to_string(),
                                    tab_id,
                                });
                            }
                        }
                    });
                } else {
                    app.set_status("Not connected".to_string(), StatusLevel::Warning);
                }
            }
            Action::CancelQuery => {
                if let Some(prov) = provider.as_ref() {
                    let db = Arc::clone(prov);
                    tokio::spawn(async move {
                        let _ = db.cancel_query().await;
                    });
                }
            }
            Action::LoadSchema => {
                if let Some(prov) = provider.as_ref() {
                    let db = Arc::clone(prov);
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
                } else {
                    app.set_status("Not connected".to_string(), StatusLevel::Warning);
                }
            }
            Action::None => {}
        }
    }

    Ok(())
}
