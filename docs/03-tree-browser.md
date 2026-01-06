# 03 - Tree Browser

> Database object hierarchy, lazy loading, navigation, and context actions.

---

## Overview

The Tree Browser displays PostgreSQL database objects in a hierarchical structure, similar to DataGrip's database explorer. It allows navigation through schemas, tables, views, functions, and their nested elements.

---

## Tree Structure

### Hierarchy

```
Database (root - implicit)
└── Schema
    ├── Tables
    │   └── Table
    │       ├── Columns
    │       │   └── Column (with type, nullable, default)
    │       ├── Indexes
    │       │   └── Index (with columns, unique flag)
    │       ├── Constraints
    │       │   └── Constraint (PK, FK, CHECK, UNIQUE)
    │       └── Triggers
    │           └── Trigger
    ├── Views
    │   └── View
    │       └── Columns
    │           └── Column
    ├── Materialized Views
    │   └── Materialized View
    │       └── Columns
    ├── Functions
    │   └── Function (with signature)
    ├── Sequences
    │   └── Sequence
    └── Types
        └── Custom Type
```

### Visual Representation

```
┌─ Database Objects ──────────────┐
│ ▼ public                        │
│   ▼ Tables                      │
│     ▼ users                     │
│       ▼ Columns                 │
│         ├ id (int4, PK)         │
│         ├ email (varchar, NN)   │
│         ├ name (text)           │
│         └ created_at (timestamptz) │
│       ▶ Indexes                 │
│       ▶ Constraints             │
│     ▶ orders                    │
│     ▶ products                  │
│   ▶ Views                       │
│   ▶ Functions                   │
│ ▶ auth                          │
│ ▶ pg_catalog (hidden by default)│
└─────────────────────────────────┘
```

### Icons and Prefixes

| Object Type | Collapsed | Expanded | Description |
|-------------|-----------|----------|-------------|
| Schema | `▶` | `▼` | Container for all objects |
| Tables (folder) | `▶` | `▼` | Tables group |
| Table | `▶` | `▼` | Individual table |
| Columns (folder) | `▶` | `▼` | Columns group |
| Column | `├` / `└` | — | Leaf node |
| Primary Key Column | `🔑` | — | Special indicator |
| Foreign Key Column | `→` | — | Reference indicator |
| Indexes (folder) | `▶` | `▼` | Indexes group |
| Index | `├` / `└` | — | Leaf node |
| Views (folder) | `▶` | `▼` | Views group |
| View | `▶` | `▼` | Individual view |
| Functions (folder) | `▶` | `▼` | Functions group |
| Function | `ƒ` | — | With signature |

---

## Data Model

### Tree Node Structure

```rust
pub struct TreeNode {
    pub id: NodeId,
    pub node_type: NodeType,
    pub name: String,
    pub children: Vec<TreeNode>,
    pub metadata: NodeMetadata,
    pub state: NodeState,
}

#[derive(Clone, Copy, PartialEq)]
pub enum NodeType {
    Schema,
    TablesFolder,
    Table,
    ColumnsFolder,
    Column,
    IndexesFolder,
    Index,
    ConstraintsFolder,
    Constraint,
    ViewsFolder,
    View,
    FunctionsFolder,
    Function,
    SequencesFolder,
    Sequence,
    TypesFolder,
    Type,
}

pub struct NodeMetadata {
    // For columns
    pub data_type: Option<String>,
    pub nullable: Option<bool>,
    pub default_value: Option<String>,
    pub is_primary_key: Option<bool>,
    pub is_foreign_key: Option<bool>,
    pub references: Option<ForeignKeyRef>,

    // For indexes
    pub is_unique: Option<bool>,
    pub index_columns: Option<Vec<String>>,

    // For functions
    pub signature: Option<String>,
    pub return_type: Option<String>,

    // For sequences
    pub current_value: Option<i64>,

    // For tables
    pub row_estimate: Option<u64>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum NodeState {
    Collapsed,
    Expanded,
    Loading,
}
```

### Tree Browser State

```rust
pub struct TreeBrowser {
    root: Vec<TreeNode>,          // Schema nodes
    selected: Option<NodeId>,     // Currently selected node
    scroll_offset: usize,         // Vertical scroll position
    visible_nodes: Vec<NodeId>,   // Flattened visible nodes
    filter: Option<String>,       // Search/filter text
    show_system_schemas: bool,    // Show pg_catalog, information_schema
}
```

---

## Navigation

### Keyboard Controls

| Key | Action |
|-----|--------|
| `↑` / `k` | Move selection up |
| `↓` / `j` | Move selection down |
| `→` / `l` / `Enter` | Expand node / Drill into |
| `←` / `h` | Collapse node / Go to parent |
| `Home` | Jump to first node |
| `End` | Jump to last node |
| `PageUp` | Scroll up one page |
| `PageDown` | Scroll down one page |
| `/` | Start search/filter |
| `Escape` | Clear filter |
| `r` | Refresh current node |
| `R` | Refresh entire tree |

### Navigation Logic

```rust
impl TreeBrowser {
    pub fn move_up(&mut self) {
        if let Some(current) = self.selected {
            let current_idx = self.visible_nodes.iter()
                .position(|n| n == &current);

            if let Some(idx) = current_idx {
                if idx > 0 {
                    self.selected = Some(self.visible_nodes[idx - 1].clone());
                    self.ensure_visible(idx - 1);
                }
            }
        }
    }

    pub fn expand(&mut self) {
        if let Some(current) = self.selected.as_ref() {
            if let Some(node) = self.find_node_mut(current) {
                match node.state {
                    NodeState::Collapsed => {
                        if node.children.is_empty() {
                            node.state = NodeState::Loading;
                            // Trigger async load
                        } else {
                            node.state = NodeState::Expanded;
                        }
                    }
                    NodeState::Expanded => {
                        // If on expandable child, select first child
                        if !node.children.is_empty() {
                            self.selected = Some(node.children[0].id.clone());
                        }
                    }
                    _ => {}
                }
            }
        }
        self.rebuild_visible_nodes();
    }

    pub fn collapse(&mut self) {
        if let Some(current) = self.selected.as_ref() {
            if let Some(node) = self.find_node_mut(current) {
                if node.state == NodeState::Expanded {
                    node.state = NodeState::Collapsed;
                } else {
                    // Go to parent
                    if let Some(parent_id) = self.find_parent(current) {
                        self.selected = Some(parent_id);
                    }
                }
            }
        }
        self.rebuild_visible_nodes();
    }
}
```

---

## Lazy Loading

### Strategy

Not all data is loaded upfront. Child nodes are fetched when a parent is first expanded.

| Level | Load Trigger | Data Fetched |
|-------|--------------|--------------|
| Schemas | On connect | All schema names |
| Tables/Views/Functions folders | On schema expand | All object names in schema |
| Table contents | On table expand | Columns, index names, constraint names |
| Column details | On columns folder expand | Already loaded with table |
| Index details | On index expand | Columns in index |

### Loading States

```rust
impl TreeBrowser {
    async fn load_children(&mut self, node_id: &NodeId, db: &impl DatabaseProvider) {
        let node = self.find_node_mut(node_id).unwrap();
        node.state = NodeState::Loading;

        let children = match node.node_type {
            NodeType::Schema => {
                // Load tables, views, functions folders with counts
                db.get_schema_contents(&node.name).await
            }
            NodeType::TablesFolder => {
                db.get_tables(&self.get_schema_name(node_id)).await
            }
            NodeType::Table => {
                db.get_table_details(&self.get_schema_name(node_id), &node.name).await
            }
            // ... other types
        };

        match children {
            Ok(children) => {
                node.children = children;
                node.state = NodeState::Expanded;
            }
            Err(e) => {
                node.state = NodeState::Collapsed;
                // Show error in status bar
            }
        }
    }
}
```

---

## Search and Filter

### Filter Behavior

```
User types: /user
Result:
┌─ Database Objects ──────────────┐
│ Filter: user                    │
│ ─────────────────────────────── │
│ ▼ public                        │
│   ▼ Tables                      │
│     ▶ users                     │  ← matches
│     ▶ user_roles                │  ← matches
│     ▶ user_sessions             │  ← matches
│   ▼ Views                       │
│     ▶ active_users              │  ← matches
└─────────────────────────────────┘
```

### Filter Logic

```rust
impl TreeBrowser {
    pub fn apply_filter(&mut self, filter: &str) {
        self.filter = Some(filter.to_lowercase());
        self.rebuild_visible_nodes();
    }

    fn node_matches_filter(&self, node: &TreeNode) -> bool {
        if let Some(filter) = &self.filter {
            // Check if this node or any descendant matches
            if node.name.to_lowercase().contains(filter) {
                return true;
            }
            node.children.iter().any(|c| self.node_matches_filter(c))
        } else {
            true
        }
    }

    fn rebuild_visible_nodes(&mut self) {
        self.visible_nodes.clear();
        for schema in &self.root {
            self.add_visible_nodes(schema, 0);
        }
    }
}
```

---

## Context Actions

### Quick Actions on Selected Node

| Node Type | Action | Key |
|-----------|--------|-----|
| Table | Select top 100 | `Enter` (when editor focused) |
| Table | Show CREATE statement | `Ctrl+D` |
| Column | Copy column name | `y` |
| Column | Add to query | `Enter` |
| Index | Show CREATE INDEX | `Ctrl+D` |
| Function | Show function source | `Ctrl+D` |

### Generated Queries

```rust
impl TreeBrowser {
    fn generate_select(&self, node: &TreeNode) -> Option<String> {
        match node.node_type {
            NodeType::Table | NodeType::View => {
                let schema = self.get_schema_name(&node.id);
                Some(format!(
                    "SELECT *\nFROM {}.{}\nLIMIT 100;",
                    quote_ident(&schema),
                    quote_ident(&node.name)
                ))
            }
            _ => None,
        }
    }

    fn generate_ddl(&self, node: &TreeNode, db: &impl DatabaseProvider) -> Option<String> {
        // Fetch DDL from database
        // For tables: pg_get_tabledef or reconstruct from metadata
        // For functions: pg_get_functiondef
        // For indexes: pg_get_indexdef
    }
}
```

---

## Schema Queries

### Get All Schemas

```sql
SELECT schema_name
FROM information_schema.schemata
WHERE schema_name NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
   OR $1 = true  -- show_system_schemas parameter
ORDER BY
    CASE WHEN schema_name = 'public' THEN 0 ELSE 1 END,
    schema_name;
```

### Get Tables in Schema

```sql
SELECT
    t.table_name,
    pg_stat_user_tables.n_live_tup AS row_estimate
FROM information_schema.tables t
LEFT JOIN pg_stat_user_tables
    ON t.table_name = pg_stat_user_tables.relname
   AND t.table_schema = pg_stat_user_tables.schemaname
WHERE t.table_schema = $1
  AND t.table_type = 'BASE TABLE'
ORDER BY t.table_name;
```

### Get Columns for Table

```sql
SELECT
    c.column_name,
    c.data_type,
    c.is_nullable = 'YES' AS nullable,
    c.column_default,
    c.character_maximum_length,
    c.numeric_precision,
    EXISTS (
        SELECT 1 FROM information_schema.key_column_usage kcu
        JOIN information_schema.table_constraints tc
            ON kcu.constraint_name = tc.constraint_name
        WHERE tc.constraint_type = 'PRIMARY KEY'
          AND kcu.table_schema = c.table_schema
          AND kcu.table_name = c.table_name
          AND kcu.column_name = c.column_name
    ) AS is_primary_key,
    (
        SELECT ccu.table_name || '.' || ccu.column_name
        FROM information_schema.key_column_usage kcu
        JOIN information_schema.referential_constraints rc
            ON kcu.constraint_name = rc.constraint_name
        JOIN information_schema.constraint_column_usage ccu
            ON rc.unique_constraint_name = ccu.constraint_name
        WHERE kcu.table_schema = c.table_schema
          AND kcu.table_name = c.table_name
          AND kcu.column_name = c.column_name
        LIMIT 1
    ) AS references
FROM information_schema.columns c
WHERE c.table_schema = $1
  AND c.table_name = $2
ORDER BY c.ordinal_position;
```

### Get Indexes for Table

```sql
SELECT
    i.relname AS index_name,
    ix.indisunique AS is_unique,
    ix.indisprimary AS is_primary,
    array_agg(a.attname ORDER BY array_position(ix.indkey, a.attnum)) AS columns
FROM pg_index ix
JOIN pg_class i ON i.oid = ix.indexrelid
JOIN pg_class t ON t.oid = ix.indrelid
JOIN pg_namespace n ON n.oid = t.relnamespace
JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = ANY(ix.indkey)
WHERE n.nspname = $1
  AND t.relname = $2
GROUP BY i.relname, ix.indisunique, ix.indisprimary
ORDER BY ix.indisprimary DESC, i.relname;
```

### Get Functions in Schema

```sql
SELECT
    p.proname AS function_name,
    pg_get_function_arguments(p.oid) AS arguments,
    pg_get_function_result(p.oid) AS return_type
FROM pg_proc p
JOIN pg_namespace n ON n.oid = p.pronamespace
WHERE n.nspname = $1
  AND p.prokind = 'f'  -- 'f' for functions, 'p' for procedures
ORDER BY p.proname;
```

---

## Rendering

### Display Format

```rust
impl TreeBrowser {
    fn render_node(&self, node: &TreeNode, depth: usize) -> Line {
        let indent = "  ".repeat(depth);
        let icon = match (node.node_type, node.state) {
            (_, NodeState::Loading) => "⣾",
            (_, NodeState::Expanded) => "▼",
            (NodeType::Column, _) => "├",
            (_, NodeState::Collapsed) if !node.children.is_empty() => "▶",
            _ => "─",
        };

        let suffix = self.node_suffix(node);

        let style = if Some(&node.id) == self.selected.as_ref() {
            Style::default().bg(Color::DarkBlue).fg(Color::White)
        } else {
            self.style_for_node_type(node.node_type)
        };

        Line::styled(format!("{}{} {}{}", indent, icon, node.name, suffix), style)
    }

    fn node_suffix(&self, node: &TreeNode) -> String {
        match node.node_type {
            NodeType::Column => {
                let mut parts = vec![];
                if let Some(dt) = &node.metadata.data_type {
                    parts.push(format!("({})", dt));
                }
                if node.metadata.is_primary_key == Some(true) {
                    parts.push("PK".to_string());
                }
                if node.metadata.nullable == Some(false) {
                    parts.push("NN".to_string());
                }
                if !parts.is_empty() {
                    format!(" {}", parts.join(", "))
                } else {
                    String::new()
                }
            }
            NodeType::Function => {
                node.metadata.signature.clone()
                    .map(|s| format!("({})", s))
                    .unwrap_or_default()
            }
            NodeType::Index if node.metadata.is_unique == Some(true) => {
                " (unique)".to_string()
            }
            _ => String::new(),
        }
    }
}
```

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation_move_down_selects_next_visible() {
        let mut tree = TreeBrowser::new(test_schema());
        tree.selected = Some(NodeId::new("public"));
        tree.move_down();
        assert_eq!(tree.selected, Some(NodeId::new("public/tables")));
    }

    #[test]
    fn test_expand_collapsed_node_triggers_loading() {
        let mut tree = TreeBrowser::new(test_schema());
        tree.selected = Some(NodeId::new("public/tables"));

        // Node starts collapsed with no children
        let node = tree.find_node(&NodeId::new("public/tables")).unwrap();
        assert_eq!(node.state, NodeState::Collapsed);
        assert!(node.children.is_empty());

        tree.expand();

        // Node should now be loading
        let node = tree.find_node(&NodeId::new("public/tables")).unwrap();
        assert_eq!(node.state, NodeState::Loading);
    }

    #[test]
    fn test_filter_shows_only_matching_nodes() {
        let mut tree = TreeBrowser::new(test_schema_with_tables(&["users", "orders", "user_roles"]));
        tree.apply_filter("user");

        let visible: Vec<_> = tree.visible_nodes.iter()
            .filter_map(|id| tree.find_node(id))
            .map(|n| n.name.as_str())
            .collect();

        assert!(visible.contains(&"users"));
        assert!(visible.contains(&"user_roles"));
        assert!(!visible.contains(&"orders"));
    }

    #[test]
    fn test_collapse_on_expanded_node_collapses() {
        let mut tree = TreeBrowser::new(test_schema());
        let node = tree.find_node_mut(&NodeId::new("public")).unwrap();
        node.state = NodeState::Expanded;
        tree.selected = Some(NodeId::new("public"));

        tree.collapse();

        let node = tree.find_node(&NodeId::new("public")).unwrap();
        assert_eq!(node.state, NodeState::Collapsed);
    }

    #[test]
    fn test_collapse_on_collapsed_node_selects_parent() {
        let mut tree = TreeBrowser::new(test_schema());
        tree.selected = Some(NodeId::new("public/tables/users"));

        tree.collapse();

        assert_eq!(tree.selected, Some(NodeId::new("public/tables")));
    }
}
```

### Snapshot Tests

```rust
#[test]
fn test_tree_render_with_expanded_schema() {
    let mut tree = TreeBrowser::new(test_schema());
    tree.expand_node(&NodeId::new("public"));
    tree.expand_node(&NodeId::new("public/tables"));

    let output = render_tree(&tree, Rect::new(0, 0, 40, 20));
    insta::assert_snapshot!(output);
}

#[test]
fn test_tree_render_with_selection() {
    let mut tree = TreeBrowser::new(test_schema());
    tree.selected = Some(NodeId::new("public/tables/users"));

    let output = render_tree(&tree, Rect::new(0, 0, 40, 20));
    insta::assert_snapshot!(output);
}

#[test]
fn test_tree_render_with_filter_active() {
    let mut tree = TreeBrowser::new(test_schema());
    tree.apply_filter("user");

    let output = render_tree(&tree, Rect::new(0, 0, 40, 20));
    insta::assert_snapshot!(output);
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_load_schema_from_postgres() {
    let db = setup_test_database().await;

    // Create test schema
    db.execute("CREATE TABLE users (id SERIAL PRIMARY KEY, name TEXT NOT NULL)").await.unwrap();
    db.execute("CREATE INDEX idx_users_name ON users(name)").await.unwrap();

    let provider = PostgresProvider::connect(&db.config()).await.unwrap();
    let schema = provider.get_schema().await.unwrap();

    assert!(schema.schemas.iter().any(|s| s.name == "public"));
    let public = schema.schemas.iter().find(|s| s.name == "public").unwrap();
    assert!(public.tables.iter().any(|t| t.name == "users"));
}
```

---

## Performance Considerations

1. **Virtual Scrolling**: Only render visible nodes
2. **Lazy Loading**: Don't fetch data until needed
3. **Caching**: Cache expanded subtrees, invalidate on refresh
4. **Debounced Filter**: Wait 150ms after typing before filtering

---

## Next Steps

See [04-query-editor.md](./04-query-editor.md) for query panel details.
