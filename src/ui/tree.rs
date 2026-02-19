//! Database tree browser widget
//!
//! Displays database schemas, tables, views, functions, indexes, and columns
//! in a hierarchical tree grouped by category.

use crate::db::schema::SchemaTree;
use crate::ui::Component;
use crate::ui::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use std::collections::HashSet;

/// Node kind in the flattened tree
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NodeKind {
    Schema,
    Category,
    Table,
    View,
    Column,
    Function,
    Index,
}

/// A single item in the flattened tree view
#[derive(Debug, Clone)]
struct TreeItem {
    label: String,
    kind: NodeKind,
    depth: usize,
    /// Path used for expand/collapse tracking (e.g. "public" or "public.users")
    path: String,
    /// Whether this node can be expanded
    expandable: bool,
}

/// Tree browser component
pub struct TreeBrowser {
    schema: Option<SchemaTree>,
    /// Flattened visible items
    items: Vec<TreeItem>,
    /// Currently selected item index
    selected: usize,
    /// Scroll offset
    scroll_offset: usize,
    /// Set of expanded node paths
    expanded: HashSet<String>,
}

impl TreeBrowser {
    pub fn new() -> Self {
        Self {
            schema: None,
            items: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            expanded: HashSet::new(),
        }
    }

    pub fn set_schema(&mut self, schema: SchemaTree) {
        self.schema = Some(schema);
        self.selected = 0;
        self.scroll_offset = 0;
        // Auto-expand first schema and its "Tables" category
        if let Some(ref tree) = self.schema
            && let Some(first) = tree.schemas.first()
        {
            self.expanded.insert(first.name.clone());
            if !first.tables.is_empty() {
                self.expanded.insert(format!("{}.Tables", first.name));
            }
        }
        self.rebuild_items();
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.schema = None;
        self.items.clear();
        self.selected = 0;
        self.scroll_offset = 0;
        self.expanded.clear();
    }

    fn rebuild_items(&mut self) {
        self.items.clear();
        let schema_tree = match &self.schema {
            Some(s) => s,
            None => return,
        };

        for schema in &schema_tree.schemas {
            let schema_path = schema.name.clone();
            let has_children = !schema.tables.is_empty()
                || !schema.views.is_empty()
                || !schema.functions.is_empty()
                || !schema.indexes.is_empty();

            self.items.push(TreeItem {
                label: schema.name.clone(),
                kind: NodeKind::Schema,
                depth: 0,
                path: schema_path.clone(),
                expandable: has_children,
            });

            if !self.expanded.contains(&schema_path) {
                continue;
            }

            // ── Tables category ──
            if !schema.tables.is_empty() {
                let cat_path = format!("{}.Tables", schema.name);
                self.items.push(TreeItem {
                    label: "Tables".to_string(),
                    kind: NodeKind::Category,
                    depth: 1,
                    path: cat_path.clone(),
                    expandable: true,
                });

                if self.expanded.contains(&cat_path) {
                    for table in &schema.tables {
                        let table_path = format!("{}.{}", cat_path, table.name);
                        self.items.push(TreeItem {
                            label: table.name.clone(),
                            kind: NodeKind::Table,
                            depth: 2,
                            path: table_path.clone(),
                            expandable: !table.columns.is_empty(),
                        });

                        if self.expanded.contains(&table_path) {
                            push_columns(&mut self.items, &table.columns, &table_path, 3);
                        }
                    }
                }
            }

            // ── Views category ──
            if !schema.views.is_empty() {
                let cat_path = format!("{}.Views", schema.name);
                self.items.push(TreeItem {
                    label: "Views".to_string(),
                    kind: NodeKind::Category,
                    depth: 1,
                    path: cat_path.clone(),
                    expandable: true,
                });

                if self.expanded.contains(&cat_path) {
                    for view in &schema.views {
                        let view_path = format!("{}.{}", cat_path, view.name);
                        self.items.push(TreeItem {
                            label: view.name.clone(),
                            kind: NodeKind::View,
                            depth: 2,
                            path: view_path.clone(),
                            expandable: !view.columns.is_empty(),
                        });

                        if self.expanded.contains(&view_path) {
                            push_columns(&mut self.items, &view.columns, &view_path, 3);
                        }
                    }
                }
            }

            // ── Functions category ──
            if !schema.functions.is_empty() {
                let cat_path = format!("{}.Functions", schema.name);
                self.items.push(TreeItem {
                    label: "Functions".to_string(),
                    kind: NodeKind::Category,
                    depth: 1,
                    path: cat_path.clone(),
                    expandable: true,
                });

                if self.expanded.contains(&cat_path) {
                    for func in &schema.functions {
                        let label = if func.return_type.is_empty() {
                            format!("{}({})", func.name, func.args)
                        } else {
                            format!("{}({}) → {}", func.name, func.args, func.return_type)
                        };
                        self.items.push(TreeItem {
                            label,
                            kind: NodeKind::Function,
                            depth: 2,
                            path: format!("{}.{}", cat_path, func.name),
                            expandable: false,
                        });
                    }
                }
            }

            // ── Indexes category ──
            if !schema.indexes.is_empty() {
                let cat_path = format!("{}.Indexes", schema.name);
                self.items.push(TreeItem {
                    label: "Indexes".to_string(),
                    kind: NodeKind::Category,
                    depth: 1,
                    path: cat_path.clone(),
                    expandable: true,
                });

                if self.expanded.contains(&cat_path) {
                    for idx in &schema.indexes {
                        let label = format!("{} ({})", idx.name, idx.columns.join(", "));
                        self.items.push(TreeItem {
                            label,
                            kind: NodeKind::Index,
                            depth: 2,
                            path: format!("{}.{}", cat_path, idx.name),
                            expandable: false,
                        });
                    }
                }
            }
        }

        // Clamp selected index
        if !self.items.is_empty() && self.selected >= self.items.len() {
            self.selected = self.items.len() - 1;
        }
    }

    pub fn toggle_expand(&mut self) {
        if let Some(item) = self.items.get(self.selected)
            && item.expandable
        {
            let path = item.path.clone();
            if self.expanded.contains(&path) {
                self.expanded.remove(&path);
            } else {
                self.expanded.insert(path);
            }
            self.rebuild_items();
        }
    }

    pub fn expand_current(&mut self) {
        if let Some(item) = self.items.get(self.selected)
            && item.expandable
            && !self.expanded.contains(&item.path)
        {
            let path = item.path.clone();
            self.expanded.insert(path);
            self.rebuild_items();
        }
    }

    pub fn move_up(&mut self) {
        if !self.items.is_empty() && self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if !self.items.is_empty() && self.selected < self.items.len() - 1 {
            self.selected += 1;
        }
    }

    /// If the selected node is a table or view, return a preview query for it.
    pub fn preview_query(&self) -> Option<String> {
        let item = self.items.get(self.selected)?;
        match item.kind {
            NodeKind::Table | NodeKind::View => {
                // Path format: "schema.Tables.tablename" or "schema.Views.viewname"
                let parts: Vec<&str> = item.path.splitn(3, '.').collect();
                if parts.len() == 3 {
                    let schema = parts[0];
                    let name = parts[2];
                    Some(format!(
                        "SELECT * FROM \"{}\".\"{}\" LIMIT 100",
                        schema, name
                    ))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Expose the loaded schema tree for use by the completer.
    pub fn schema(&self) -> Option<&SchemaTree> {
        self.schema.as_ref()
    }

    pub fn collapse_current(&mut self) {
        if let Some(item) = self.items.get(self.selected) {
            let path = item.path.clone();
            if self.expanded.contains(&path) {
                self.expanded.remove(&path);
                self.rebuild_items();
            } else if item.depth > 0 {
                // Move to parent
                let parent_path = path.rsplit_once('.').map(|(p, _)| p.to_string());
                if let Some(parent) = parent_path {
                    // Find parent item
                    if let Some(idx) = self.items.iter().position(|i| i.path == parent) {
                        self.selected = idx;
                    }
                }
            }
        }
    }
}

/// Push column items into the flat item list
fn push_columns(
    items: &mut Vec<TreeItem>,
    columns: &[crate::db::schema::Column],
    parent_path: &str,
    depth: usize,
) {
    for col in columns {
        let col_label = format_column_label(col);
        items.push(TreeItem {
            label: col_label,
            kind: NodeKind::Column,
            depth,
            path: format!("{}.{}", parent_path, col.name),
            expandable: false,
        });
    }
}

/// Format a column label with PK/FK annotations
fn format_column_label(col: &crate::db::schema::Column) -> String {
    let prefix = if col.is_primary_key { "* " } else { "" };
    let suffix = if let Some(ref fk) = col.foreign_key {
        format!(" → {}.{}", fk.target_table, fk.target_column)
    } else {
        String::new()
    };
    format!(
        "{}{} ({}){}",
        prefix,
        col.name,
        col.data_type.display_name(),
        suffix
    )
}

impl Default for TreeBrowser {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for TreeBrowser {
    fn render(&self, frame: &mut Frame, area: Rect, focused: bool, theme: &Theme) {
        if self.items.is_empty() {
            let msg = if self.schema.is_some() {
                "No schemas found"
            } else {
                "Not connected"
            };
            let p = Paragraph::new(msg).style(theme.tree_empty);
            frame.render_widget(p, area);
            return;
        }

        let visible_height = area.height as usize;
        let viewer = self;

        // Ensure selected is visible
        let scroll_offset = if viewer.selected < viewer.scroll_offset {
            viewer.selected
        } else if viewer.selected >= viewer.scroll_offset + visible_height {
            viewer.selected - visible_height + 1
        } else {
            viewer.scroll_offset
        };

        for vis_row in 0..visible_height {
            let item_idx = scroll_offset + vis_row;
            let y = area.y + vis_row as u16;

            if item_idx >= self.items.len() {
                break;
            }

            let item = &self.items[item_idx];
            let is_selected = focused && item_idx == viewer.selected;

            // Build display string with indentation and expand indicator
            let indent = "  ".repeat(item.depth);
            let indicator = if item.expandable {
                if self.expanded.contains(&item.path) {
                    "▼ "
                } else {
                    "▶ "
                }
            } else {
                "  "
            };

            let display = format!("{}{}{}", indent, indicator, item.label);
            let truncated = if display.len() > area.width as usize {
                format!("{}...", &display[..area.width as usize - 3])
            } else {
                format!("{:<width$}", display, width = area.width as usize)
            };

            let style = if is_selected {
                theme.tree_selected
            } else {
                match item.kind {
                    NodeKind::Schema => theme.tree_schema,
                    NodeKind::Category => theme.tree_category,
                    NodeKind::Table => theme.tree_table,
                    NodeKind::View => theme.tree_view,
                    NodeKind::Column => theme.tree_column,
                    NodeKind::Function => theme.tree_function,
                    NodeKind::Index => theme.tree_index,
                }
            };

            frame.render_widget(
                Paragraph::new(truncated).style(style),
                Rect::new(area.x, y, area.width, 1),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::{Column, ForeignKey, Function, Index, Schema, Table};
    use crate::db::types::DataType;

    fn sample_schema() -> SchemaTree {
        SchemaTree {
            schemas: vec![Schema {
                name: "public".to_string(),
                tables: vec![
                    Table {
                        name: "users".to_string(),
                        columns: vec![
                            Column {
                                name: "id".to_string(),
                                data_type: DataType::Integer,
                                is_primary_key: true,
                                foreign_key: None,
                            },
                            Column {
                                name: "name".to_string(),
                                data_type: DataType::Text,
                                is_primary_key: false,
                                foreign_key: None,
                            },
                        ],
                    },
                    Table {
                        name: "orders".to_string(),
                        columns: vec![
                            Column {
                                name: "id".to_string(),
                                data_type: DataType::Integer,
                                is_primary_key: true,
                                foreign_key: None,
                            },
                            Column {
                                name: "user_id".to_string(),
                                data_type: DataType::Integer,
                                is_primary_key: false,
                                foreign_key: Some(ForeignKey {
                                    target_table: "users".to_string(),
                                    target_column: "id".to_string(),
                                }),
                            },
                        ],
                    },
                ],
                views: vec![Table {
                    name: "active_users".to_string(),
                    columns: vec![Column {
                        name: "id".to_string(),
                        data_type: DataType::Integer,
                        is_primary_key: false,
                        foreign_key: None,
                    }],
                }],
                indexes: vec![Index {
                    name: "users_pkey".to_string(),
                    columns: vec!["id".to_string()],
                    is_unique: true,
                    is_primary: true,
                    table_name: "users".to_string(),
                }],
                functions: vec![Function {
                    name: "get_user".to_string(),
                    args: "integer".to_string(),
                    return_type: "users".to_string(),
                }],
            }],
        }
    }

    #[test]
    fn test_tree_browser_new() {
        let tree = TreeBrowser::new();
        assert!(tree.schema.is_none());
        assert!(tree.items.is_empty());
    }

    #[test]
    fn test_set_schema_auto_expands_first_and_tables() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        assert!(tree.expanded.contains("public"));
        assert!(tree.expanded.contains("public.Tables"));
        // Schema + Tables category + 2 tables (auto-expanded) + Views/Functions/Indexes categories
        assert!(tree.items.len() >= 5);
    }

    #[test]
    fn test_category_nodes_appear() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        let labels: Vec<&str> = tree.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"Tables"));
        assert!(labels.contains(&"Views"));
        assert!(labels.contains(&"Functions"));
        assert!(labels.contains(&"Indexes"));
    }

    #[test]
    fn test_expand_table_shows_columns() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        // Find "users" table and expand it
        let users_idx = tree.items.iter().position(|i| i.label == "users").unwrap();
        tree.selected = users_idx;
        tree.toggle_expand();
        // Should show columns under users
        let labels: Vec<&str> = tree.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.iter().any(|l| l.starts_with("* id")));
        assert!(labels.iter().any(|l| l.starts_with("name")));
    }

    #[test]
    fn test_pk_column_label() {
        let col = Column {
            name: "id".to_string(),
            data_type: DataType::Integer,
            is_primary_key: true,
            foreign_key: None,
        };
        assert_eq!(format_column_label(&col), "* id (integer)");
    }

    #[test]
    fn test_fk_column_label() {
        let col = Column {
            name: "user_id".to_string(),
            data_type: DataType::Integer,
            is_primary_key: false,
            foreign_key: Some(ForeignKey {
                target_table: "users".to_string(),
                target_column: "id".to_string(),
            }),
        };
        assert_eq!(format_column_label(&col), "user_id (integer) → users.id");
    }

    #[test]
    fn test_expand_views_category() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        let views_idx = tree.items.iter().position(|i| i.label == "Views").unwrap();
        tree.selected = views_idx;
        tree.toggle_expand();
        let labels: Vec<&str> = tree.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"active_users"));
    }

    #[test]
    fn test_expand_functions_category() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        let func_idx = tree
            .items
            .iter()
            .position(|i| i.label == "Functions")
            .unwrap();
        tree.selected = func_idx;
        tree.toggle_expand();
        let labels: Vec<&str> = tree.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.iter().any(|l| l.contains("get_user")));
        assert!(labels.iter().any(|l| l.contains("→ users")));
    }

    #[test]
    fn test_expand_indexes_category() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        let idx_idx = tree
            .items
            .iter()
            .position(|i| i.label == "Indexes")
            .unwrap();
        tree.selected = idx_idx;
        tree.toggle_expand();
        let labels: Vec<&str> = tree.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.iter().any(|l| l.contains("users_pkey")));
    }

    #[test]
    fn test_preview_query_for_table() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        // Select the "users" table
        let users_idx = tree.items.iter().position(|i| i.label == "users").unwrap();
        tree.selected = users_idx;
        assert_eq!(
            tree.preview_query(),
            Some("SELECT * FROM \"public\".\"users\" LIMIT 100".to_string())
        );
    }

    #[test]
    fn test_preview_query_for_view() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        // Expand Views category first
        let views_idx = tree.items.iter().position(|i| i.label == "Views").unwrap();
        tree.selected = views_idx;
        tree.toggle_expand();
        // Select the view
        let view_idx = tree
            .items
            .iter()
            .position(|i| i.label == "active_users")
            .unwrap();
        tree.selected = view_idx;
        assert_eq!(
            tree.preview_query(),
            Some("SELECT * FROM \"public\".\"active_users\" LIMIT 100".to_string())
        );
    }

    #[test]
    fn test_preview_query_none_for_schema() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        // First item is the schema node
        tree.selected = 0;
        assert_eq!(tree.items[0].label, "public");
        assert_eq!(tree.preview_query(), None);
    }

    #[test]
    fn test_preview_query_none_for_category() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        let tables_idx = tree.items.iter().position(|i| i.label == "Tables").unwrap();
        tree.selected = tables_idx;
        assert_eq!(tree.preview_query(), None);
    }

    #[test]
    fn test_preview_query_none_for_column() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        // Expand users table to get columns
        let users_idx = tree.items.iter().position(|i| i.label == "users").unwrap();
        tree.selected = users_idx;
        tree.toggle_expand();
        // Select a column
        let col_idx = tree
            .items
            .iter()
            .position(|i| i.label.starts_with("* id"))
            .unwrap();
        tree.selected = col_idx;
        assert_eq!(tree.preview_query(), None);
    }

    #[test]
    fn test_empty_categories_hidden() {
        let schema = SchemaTree {
            schemas: vec![Schema {
                name: "empty".to_string(),
                tables: vec![Table {
                    name: "t".to_string(),
                    columns: vec![],
                }],
                views: vec![],
                indexes: vec![],
                functions: vec![],
            }],
        };
        let mut tree = TreeBrowser::new();
        tree.set_schema(schema);
        let labels: Vec<&str> = tree.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"Tables"));
        assert!(!labels.contains(&"Views"));
        assert!(!labels.contains(&"Functions"));
        assert!(!labels.contains(&"Indexes"));
    }
}
