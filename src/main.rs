//! vizgres - A fast, keyboard-driven PostgreSQL client for the terminal

use std::sync::Arc;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use crossterm::{
    event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::*;
use tokio::sync::mpsc;
use vizgres::app::{Action, App, AppEvent, LoadMoreItems, StatusLevel};
use vizgres::config::{self, ConnectionConfig, Settings};
use vizgres::connection_manager::ConnectionManager;
use vizgres::db::{self, Database};
use vizgres::error::DbError;

/// A fast, keyboard-driven PostgreSQL client for the terminal
#[derive(Parser)]
#[command(name = "vizgres", version, about)]
#[command(args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<CliCommand>,

    #[command(flatten)]
    connect: ConnectArgs,
}

#[derive(Args)]
struct ConnectArgs {
    /// Connection URL (postgres://...) or saved connection name
    target: Option<String>,
}

#[derive(Subcommand)]
enum CliCommand {
    /// Manage vizgres configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current settings and saved connections
    List,
    /// Open config file in $EDITOR
    Edit,
    /// Print config directory path
    Path,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle config subcommands (non-TUI, print to stdout and exit)
    if let Some(CliCommand::Config { action }) = cli.command {
        return handle_config_action(action);
    }

    // Load settings
    let settings = Settings::load();

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

    // Resolve connection target (URL or saved name)
    let (mut conn_mgr, mut app) = if let Some(ref target) = cli.connect.target {
        let conn_config = resolve_connection(target)?;

        eprintln!("Connecting to {}...", conn_config.name);
        let (prov, rx) =
            db::PostgresProvider::connect(&conn_config, settings.settings.statement_timeout_ms)
                .await
                .map_err(|e| anyhow::anyhow!("Connection failed: {}", e))?;
        let prov = Arc::new(prov);

        let schema = prov
            .get_schema(settings.settings.tree_category_limit)
            .await
            .map_err(|e| anyhow::anyhow!("Schema load failed: {}", e))?;

        let app = App::with_connection(conn_config.name.clone(), schema, &settings);

        // Seed tab 0 with the initial connection
        let mut mgr =
            ConnectionManager::new(Some(conn_config), settings.settings.statement_timeout_ms);
        mgr.insert(0, prov, rx);
        (mgr, app)
    } else {
        // No target — start disconnected and show connection dialog
        let mut app = App::new_with_settings(&settings);
        app.show_connection_dialog();
        let mgr = ConnectionManager::new(None, settings.settings.statement_timeout_ms);
        (mgr, app)
    };

    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the app (separated so we can always clean up)
    let result = run_app(&mut terminal, &mut app, &mut conn_mgr).await;

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

/// Resolve a connection target string to a ConnectionConfig.
/// Tries URL parsing first, then falls back to saved connection name lookup.
fn resolve_connection(target: &str) -> Result<ConnectionConfig> {
    // Try as a URL first
    if target.starts_with("postgres://") || target.starts_with("postgresql://") {
        return ConnectionConfig::from_url(target)
            .map_err(|e| anyhow::anyhow!("Invalid connection URL: {}", e));
    }

    // Fall back to saved connection name
    config::find_connection(target).map_err(|e| {
        anyhow::anyhow!(
            "Could not resolve '{}': not a valid postgres:// URL and {}",
            target,
            e
        )
    })
}

/// Handle `vizgres config <action>` subcommands
fn handle_config_action(action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Path => {
            let dir = ConnectionConfig::config_dir().map_err(|e| anyhow::anyhow!("{}", e))?;
            println!("{}", dir.display());
        }
        ConfigAction::List => {
            print_config_list()?;
        }
        ConfigAction::Edit => {
            open_config_in_editor()?;
        }
    }
    Ok(())
}

/// Print current settings and saved connections
fn print_config_list() -> Result<()> {
    let config_path = Settings::config_file().map_err(|e| anyhow::anyhow!("{}", e))?;
    let settings = Settings::load();
    let defaults = Settings::default();

    // Settings section
    let exists = config_path.exists();
    let path_display = if exists {
        format!("({})", config_path.display())
    } else {
        format!("({} — not found, using defaults)", config_path.display())
    };
    let default_tag = |val: usize, def: usize| if val == def { "(default)" } else { "" };
    println!("Settings {}:", path_display);
    println!(
        "  {:<20} {:<8} {}",
        "preview_rows",
        settings.settings.preview_rows,
        default_tag(
            settings.settings.preview_rows,
            defaults.settings.preview_rows
        ),
    );
    println!(
        "  {:<20} {:<8} {}",
        "max_tabs",
        settings.settings.max_tabs,
        default_tag(settings.settings.max_tabs, defaults.settings.max_tabs),
    );
    println!(
        "  {:<20} {:<8} {}",
        "history_size",
        settings.settings.history_size,
        default_tag(
            settings.settings.history_size,
            defaults.settings.history_size
        ),
    );
    let timeout_default = |val: u64, def: u64| if val == def { "(default)" } else { "" };
    println!(
        "  {:<20} {:<8} {}",
        "query_timeout_ms",
        settings.settings.query_timeout_ms,
        timeout_default(
            settings.settings.query_timeout_ms,
            defaults.settings.query_timeout_ms
        ),
    );
    println!(
        "  {:<20} {:<8} {}",
        "max_result_rows",
        settings.settings.max_result_rows,
        default_tag(
            settings.settings.max_result_rows,
            defaults.settings.max_result_rows
        ),
    );
    println!(
        "  {:<20} {:<8} {}",
        "statement_timeout_ms",
        settings.settings.statement_timeout_ms,
        timeout_default(
            settings.settings.statement_timeout_ms,
            defaults.settings.statement_timeout_ms
        ),
    );

    // Keybinding overrides
    let total_overrides = settings.keybindings.global.len()
        + settings.keybindings.editor.len()
        + settings.keybindings.results.len()
        + settings.keybindings.tree.len();
    if total_overrides == 0 {
        println!("\nKeybinding overrides: (none)");
    } else {
        println!("\nKeybinding overrides ({}):", total_overrides);
        print_keybinding_section("global", &settings.keybindings.global);
        print_keybinding_section("editor", &settings.keybindings.editor);
        print_keybinding_section("results", &settings.keybindings.results);
        print_keybinding_section("tree", &settings.keybindings.tree);
    }

    // Saved connections
    match config::load_connections() {
        Ok(connections) if connections.is_empty() => {
            println!("\nSaved connections: (none)");
        }
        Ok(connections) => {
            println!("\nSaved connections:");
            for conn in &connections {
                println!("  {:<20} {}", conn.name, conn.to_url_masked());
            }
        }
        Err(e) => {
            println!("\nSaved connections: (error: {})", e);
        }
    }

    Ok(())
}

fn print_keybinding_section(name: &str, bindings: &std::collections::HashMap<String, String>) {
    for (key, action) in bindings {
        println!("  [{}] \"{}\" = \"{}\"", name, key, action);
    }
}

/// Open config file in $EDITOR, creating with defaults if missing
fn open_config_in_editor() -> Result<()> {
    let config_path = Settings::config_file().map_err(|e| anyhow::anyhow!("{}", e))?;

    // Create with commented defaults if missing
    if !config_path.exists() {
        Settings::write_defaults(&config_path)
            .map_err(|e| anyhow::anyhow!("Failed to create config file: {}", e))?;
        eprintln!("Created {}", config_path.display());
    }

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let status = std::process::Command::new(&editor)
        .arg(&config_path)
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to launch {}: {}", editor, e))?;

    if !status.success() {
        anyhow::bail!("{} exited with status {}", editor, status);
    }

    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    conn_mgr: &mut ConnectionManager,
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

            // Background connection died on any tab
            result = std::future::poll_fn(|cx| conn_mgr.poll_connection_errors(cx)) => {
                let (_tab_id, msg) = result;
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

                // Drop all existing connections
                conn_mgr.disconnect_all();

                // Connect under tab_id 0 — apply_connection() resets tabs to [Tab::new(0)]
                conn_mgr.set_config(config.clone(), app.statement_timeout_ms);
                match conn_mgr.ensure_connected(0).await {
                    Ok(prov) => {
                        let limit = app.tree_browser.category_limit();
                        match prov.get_schema(limit).await {
                            Ok(schema) => {
                                app.apply_connection(config.name.clone(), schema);
                                app.set_status(
                                    format!("Connected to {}", config.name),
                                    StatusLevel::Success,
                                );
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
            Action::ExecuteQuery {
                sql,
                tab_id,
                timeout_ms,
                max_rows,
            } => {
                // Lazily connect this tab if needed
                match conn_mgr.ensure_connected(tab_id).await {
                    Ok(db) => {
                        let tx = event_tx.clone();
                        tokio::spawn(async move {
                            match db.execute_query(&sql, timeout_ms, max_rows).await {
                                Ok(results) => {
                                    let _ = tx.send(AppEvent::QueryCompleted { results, tab_id });
                                }
                                Err(e) => {
                                    let (error, position) = match e {
                                        DbError::QueryFailed { message, position } => {
                                            (message, position)
                                        }
                                        DbError::Timeout(ms) => {
                                            let msg = if ms >= 1000 {
                                                format!("Query timed out after {}s", ms / 1000)
                                            } else {
                                                format!("Query timed out after {}ms", ms)
                                            };
                                            (msg, None)
                                        }
                                        other => (other.to_string(), None),
                                    };
                                    let _ = tx.send(AppEvent::QueryFailed {
                                        error,
                                        position,
                                        tab_id,
                                    });
                                }
                            }
                        });
                    }
                    Err(e) => {
                        // Connection failed for this tab — clear running state
                        app.handle_event(AppEvent::QueryFailed {
                            error: e,
                            position: None,
                            tab_id,
                        })?;
                    }
                }
            }
            Action::CancelQuery { tab_id, terminate } => {
                if let Some(prov) = conn_mgr.get(tab_id) {
                    let db = Arc::clone(prov);
                    tokio::spawn(async move {
                        let _ = db.cancel_query_enhanced(terminate).await;
                    });
                }
            }
            Action::LoadSchema => {
                if let Some(prov) = conn_mgr.any_provider() {
                    let db = Arc::clone(prov);
                    let tx = event_tx.clone();
                    let limit = app.tree_browser.category_limit();
                    tokio::spawn(async move {
                        match db.get_schema(limit).await {
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
            Action::SearchSchema { pattern } => {
                if let Some(prov) = conn_mgr.any_provider() {
                    let db = Arc::clone(prov);
                    let tx = event_tx.clone();
                    tokio::spawn(async move {
                        match db.search_schema(&pattern).await {
                            Ok(results) => {
                                let _ = tx.send(AppEvent::SchemaSearchCompleted(results));
                            }
                            Err(e) => {
                                let _ = tx.send(AppEvent::SchemaSearchFailed(e.to_string()));
                            }
                        }
                    });
                } else {
                    app.set_status("Not connected".to_string(), StatusLevel::Warning);
                }
            }
            Action::LoadMoreCategory {
                schema_name,
                category,
                offset,
                limit,
            } => {
                if let Some(prov) = conn_mgr.any_provider() {
                    let db = Arc::clone(prov);
                    let tx = event_tx.clone();
                    let schema = schema_name.clone();
                    let cat = category.clone();
                    tokio::spawn(async move {
                        let result = match category.as_str() {
                            "Tables" => db
                                .load_more_tables(&schema_name, offset, limit)
                                .await
                                .map(LoadMoreItems::Tables),
                            "Views" => db
                                .load_more_views(&schema_name, offset, limit)
                                .await
                                .map(LoadMoreItems::Views),
                            "Functions" => db
                                .load_more_functions(&schema_name, offset, limit)
                                .await
                                .map(LoadMoreItems::Functions),
                            "Indexes" => db
                                .load_more_indexes(&schema_name, offset, limit)
                                .await
                                .map(LoadMoreItems::Indexes),
                            _ => return,
                        };
                        match result {
                            Ok(items) => {
                                let _ = tx.send(AppEvent::LoadMoreCompleted {
                                    schema_name: schema,
                                    category: cat,
                                    items,
                                });
                            }
                            Err(e) => {
                                let _ = tx.send(AppEvent::LoadMoreFailed(e.to_string()));
                            }
                        }
                    });
                } else {
                    app.set_status("Not connected".to_string(), StatusLevel::Warning);
                }
            }
            Action::TabClosed { tab_id } => {
                conn_mgr.remove(tab_id);
            }
            Action::Disconnect => {
                conn_mgr.disconnect_all();
            }
            Action::None => {}
        }
    }

    Ok(())
}
