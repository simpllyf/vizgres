//! Database tree browser widget
//!
//! Displays database schemas, tables, and columns in a hierarchical tree.

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
    Table,
    Column,
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
        // Auto-expand first schema
        if let Some(ref tree) = self.schema
            && let Some(first) = tree.schemas.first()
        {
            self.expanded.insert(first.name.clone());
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
            self.items.push(TreeItem {
                label: schema.name.clone(),
                kind: NodeKind::Schema,
                depth: 0,
                path: schema_path.clone(),
                expandable: !schema.tables.is_empty(),
            });

            if self.expanded.contains(&schema_path) {
                for table in &schema.tables {
                    let table_path = format!("{}.{}", schema.name, table.name);
                    self.items.push(TreeItem {
                        label: table.name.clone(),
                        kind: NodeKind::Table,
                        depth: 1,
                        path: table_path.clone(),
                        expandable: !table.columns.is_empty(),
                    });

                    if self.expanded.contains(&table_path) {
                        for col in &table.columns {
                            let col_label =
                                format!("{} ({})", col.name, col.data_type.display_name());
                            self.items.push(TreeItem {
                                label: col_label,
                                kind: NodeKind::Column,
                                depth: 2,
                                path: format!("{}.{}", table_path, col.name),
                                expandable: false,
                            });
                        }
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
                    NodeKind::Table => theme.tree_table,
                    NodeKind::Column => theme.tree_column,
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
    use crate::db::schema::{Column, Schema, Table};
    use crate::db::types::DataType;

    fn sample_schema() -> SchemaTree {
        SchemaTree {
            schemas: vec![Schema {
                name: "public".to_string(),
                tables: vec![Table {
                    name: "users".to_string(),
                    columns: vec![
                        Column {
                            name: "id".to_string(),
                            data_type: DataType::Integer,
                        },
                        Column {
                            name: "name".to_string(),
                            data_type: DataType::Text,
                        },
                    ],
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
    fn test_set_schema_auto_expands_first() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        assert!(tree.expanded.contains("public"));
        // Should show schema + table (auto-expanded)
        assert!(tree.items.len() >= 2);
    }

    #[test]
    fn test_expand_collapse() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        // First item is "public" (expanded), second is "users"
        tree.selected = 1; // select "users"
        tree.toggle_expand();
        // Should now show columns
        assert!(tree.items.len() >= 4); // schema + table + 2 columns
    }
}
