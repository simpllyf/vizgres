# Examples

This directory contains example configurations and usage demonstrations for vizgres.

## Configuration

### Setting Up

1. Create the config directory:
   ```bash
   mkdir -p ~/.vizgres
   ```

2. Copy example configurations:
   ```bash
   cp examples/config/connections.toml ~/.vizgres/
   cp examples/config/config.toml ~/.vizgres/
   ```

3. Edit `~/.vizgres/connections.toml` with your database details

### Configuration Files

- **connections.toml** - Database connection profiles
- **config.toml** - Application settings and preferences

## Security Best Practices

### Storing Passwords

**DO NOT** store passwords in plain text in `connections.toml`.

Phase 8 will implement keychain integration. Until then:

1. Use `.pgpass` file (PostgreSQL standard)
2. Use environment variables
3. Use connection strings with password prompts

### SSL Connections

Always use `ssl_mode = "require"` for production databases:

```toml
[[connections]]
name = "production"
ssl_mode = "require"  # Enforce SSL
```

## Usage Examples

### Basic Workflow

```bash
# Start vizgres
vizgres

# Connect to a database
:connect local

# Write and execute a query
SELECT * FROM users WHERE active = true;
# Press Ctrl+Enter to execute

# Format the query
# Press Ctrl+Shift+F

# Export results
:export csv

# Disconnect
:disconnect

# Quit
:quit
```

### Keyboard Shortcuts

See `docs/07-keyboard-shortcuts.md` for complete reference.

Essential shortcuts:
- `Tab` - Cycle between panels
- `Ctrl+Enter` - Execute query
- `Ctrl+Shift+F` - Format SQL
- `:` - Open command bar
- `Ctrl+Q` - Quit

## Sample Queries

See `examples/queries/` (to be added) for example SQL queries demonstrating:
- Common patterns
- Complex joins
- Window functions
- JSON operations
- And more
