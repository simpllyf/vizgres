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
    LoadMore,
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
    /// Whether this item matches the current filter (for highlighting)
    matches_filter: bool,
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
    /// Number of rows for table/view preview queries
    preview_rows: usize,
    /// Category limit for pagination (0 = unlimited)
    category_limit: usize,
    /// Whether filter mode is active
    filter_active: bool,
    /// Current filter text
    filter_text: String,
    /// Cursor position within filter text
    filter_cursor: usize,
    /// Paths that directly match the current filter (for highlighting)
    filter_match_paths: HashSet<String>,
    /// Expanded state before filtering started (to restore on clear)
    pre_filter_expanded: Option<HashSet<String>>,
    /// Original schema before search (to restore on clear)
    pre_search_schema: Option<SchemaTree>,
    /// Whether a backend search is in progress
    searching: bool,
}

impl TreeBrowser {
    pub fn new() -> Self {
        Self::with_settings(100, 500)
    }

    pub fn with_preview_rows(preview_rows: usize) -> Self {
        Self::with_settings(preview_rows, 500)
    }

    pub fn with_settings(preview_rows: usize, category_limit: usize) -> Self {
        Self {
            schema: None,
            items: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            expanded: HashSet::new(),
            preview_rows,
            category_limit,
            filter_active: false,
            filter_text: String::new(),
            filter_cursor: 0,
            filter_match_paths: HashSet::new(),
            pre_filter_expanded: None,
            pre_search_schema: None,
            searching: false,
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
            if !first.tables.items.is_empty() {
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

        // If filtering, we collect items then filter based on the filter text
        let filter_lower = self.filter_text.to_lowercase();

        // Show schema count if there are multiple or if truncated
        let show_schema_count =
            schema_tree.schemas.total_count > 1 || schema_tree.schemas.is_truncated();

        for schema in &schema_tree.schemas {
            let schema_path = schema.name.clone();
            let has_children = !schema.tables.items.is_empty()
                || !schema.views.items.is_empty()
                || !schema.functions.items.is_empty()
                || !schema.indexes.items.is_empty();

            // Schema label with optional count indicator
            let label = if show_schema_count && schema_tree.schemas.is_truncated() {
                // Only show "X of Y" format if schemas are truncated
                schema.name.clone()
            } else {
                schema.name.clone()
            };

            self.items.push(TreeItem {
                label,
                kind: NodeKind::Schema,
                depth: 0,
                path: schema_path.clone(),
                expandable: has_children,
                matches_filter: self.filter_match_paths.contains(&schema_path),
            });

            if !self.expanded.contains(&schema_path) {
                continue;
            }

            // ── Tables category ──
            if !schema.tables.items.is_empty() {
                let cat_path = format!("{}.Tables", schema.name);
                let label = if schema.tables.is_truncated() {
                    format!(
                        "Tables ({} of {})",
                        schema.tables.len(),
                        schema.tables.total_count
                    )
                } else {
                    format!("Tables ({})", schema.tables.total_count)
                };
                self.items.push(TreeItem {
                    label,
                    kind: NodeKind::Category,
                    depth: 1,
                    path: cat_path.clone(),
                    expandable: true,
                    matches_filter: false, // Categories don't match directly
                });

                if self.expanded.contains(&cat_path) {
                    for table in schema.tables.iter() {
                        let table_path = format!("{}.{}", cat_path, table.name);
                        self.items.push(TreeItem {
                            label: table.name.clone(),
                            kind: NodeKind::Table,
                            depth: 2,
                            path: table_path.clone(),
                            expandable: !table.columns.is_empty(),
                            matches_filter: self.filter_match_paths.contains(&table_path),
                        });

                        if self.expanded.contains(&table_path) {
                            push_columns(
                                &mut self.items,
                                &table.columns,
                                &table_path,
                                3,
                                &self.filter_match_paths,
                            );
                        }
                    }
                    // Add "Load more" item if truncated
                    if schema.tables.is_truncated() {
                        self.items.push(TreeItem {
                            label: format!(
                                "[Load {} more...]",
                                (schema.tables.total_count - schema.tables.len()).min(500)
                            ),
                            kind: NodeKind::LoadMore,
                            depth: 2,
                            path: format!("{}.Tables.__load_more__", schema.name),
                            expandable: false,
                            matches_filter: false,
                        });
                    }
                }
            }

            // ── Views category ──
            if !schema.views.items.is_empty() {
                let cat_path = format!("{}.Views", schema.name);
                let label = if schema.views.is_truncated() {
                    format!(
                        "Views ({} of {})",
                        schema.views.len(),
                        schema.views.total_count
                    )
                } else {
                    format!("Views ({})", schema.views.total_count)
                };
                self.items.push(TreeItem {
                    label,
                    kind: NodeKind::Category,
                    depth: 1,
                    path: cat_path.clone(),
                    expandable: true,
                    matches_filter: false,
                });

                if self.expanded.contains(&cat_path) {
                    for view in schema.views.iter() {
                        let view_path = format!("{}.{}", cat_path, view.name);
                        self.items.push(TreeItem {
                            label: view.name.clone(),
                            kind: NodeKind::View,
                            depth: 2,
                            path: view_path.clone(),
                            expandable: !view.columns.is_empty(),
                            matches_filter: self.filter_match_paths.contains(&view_path),
                        });

                        if self.expanded.contains(&view_path) {
                            push_columns(
                                &mut self.items,
                                &view.columns,
                                &view_path,
                                3,
                                &self.filter_match_paths,
                            );
                        }
                    }
                    // Add "Load more" item if truncated
                    if schema.views.is_truncated() {
                        self.items.push(TreeItem {
                            label: format!(
                                "[Load {} more...]",
                                (schema.views.total_count - schema.views.len()).min(500)
                            ),
                            kind: NodeKind::LoadMore,
                            depth: 2,
                            path: format!("{}.Views.__load_more__", schema.name),
                            expandable: false,
                            matches_filter: false,
                        });
                    }
                }
            }

            // ── Functions category ──
            if !schema.functions.items.is_empty() {
                let cat_path = format!("{}.Functions", schema.name);
                let label = if schema.functions.is_truncated() {
                    format!(
                        "Functions ({} of {})",
                        schema.functions.len(),
                        schema.functions.total_count
                    )
                } else {
                    format!("Functions ({})", schema.functions.total_count)
                };
                self.items.push(TreeItem {
                    label,
                    kind: NodeKind::Category,
                    depth: 1,
                    path: cat_path.clone(),
                    expandable: true,
                    matches_filter: false,
                });

                if self.expanded.contains(&cat_path) {
                    for func in schema.functions.iter() {
                        let func_path = format!("{}.{}", cat_path, func.name);
                        let label = if func.return_type.is_empty() {
                            format!("{}({})", func.name, func.args)
                        } else {
                            format!("{}({}) → {}", func.name, func.args, func.return_type)
                        };
                        self.items.push(TreeItem {
                            label,
                            kind: NodeKind::Function,
                            depth: 2,
                            path: func_path.clone(),
                            expandable: false,
                            matches_filter: self.filter_match_paths.contains(&func_path),
                        });
                    }
                    // Add "Load more" item if truncated
                    if schema.functions.is_truncated() {
                        self.items.push(TreeItem {
                            label: format!(
                                "[Load {} more...]",
                                (schema.functions.total_count - schema.functions.len()).min(500)
                            ),
                            kind: NodeKind::LoadMore,
                            depth: 2,
                            path: format!("{}.Functions.__load_more__", schema.name),
                            expandable: false,
                            matches_filter: false,
                        });
                    }
                }
            }

            // ── Indexes category ──
            if !schema.indexes.items.is_empty() {
                let cat_path = format!("{}.Indexes", schema.name);
                let label = if schema.indexes.is_truncated() {
                    format!(
                        "Indexes ({} of {})",
                        schema.indexes.len(),
                        schema.indexes.total_count
                    )
                } else {
                    format!("Indexes ({})", schema.indexes.total_count)
                };
                self.items.push(TreeItem {
                    label,
                    kind: NodeKind::Category,
                    depth: 1,
                    path: cat_path.clone(),
                    expandable: true,
                    matches_filter: false,
                });

                if self.expanded.contains(&cat_path) {
                    for idx in schema.indexes.iter() {
                        let idx_path = format!("{}.{}", cat_path, idx.name);
                        let label = format!("{} ({})", idx.name, idx.columns.join(", "));
                        self.items.push(TreeItem {
                            label,
                            kind: NodeKind::Index,
                            depth: 2,
                            path: idx_path.clone(),
                            expandable: false,
                            matches_filter: self.filter_match_paths.contains(&idx_path),
                        });
                    }
                    // Add "Load more" item if truncated
                    if schema.indexes.is_truncated() {
                        self.items.push(TreeItem {
                            label: format!(
                                "[Load {} more...]",
                                (schema.indexes.total_count - schema.indexes.len()).min(500)
                            ),
                            kind: NodeKind::LoadMore,
                            depth: 2,
                            path: format!("{}.Indexes.__load_more__", schema.name),
                            expandable: false,
                            matches_filter: false,
                        });
                    }
                }
            }
        }

        // Add "Load more schemas" item if truncated
        if schema_tree.schemas.is_truncated() {
            self.items.push(TreeItem {
                label: format!(
                    "[Load {} more schemas...]",
                    (schema_tree.schemas.total_count - schema_tree.schemas.len()).min(500)
                ),
                kind: NodeKind::LoadMore,
                depth: 0,
                path: "__schemas_load_more__".to_string(),
                expandable: false,
                matches_filter: false,
            });
        }

        // Apply filter if filter text is not empty - keep only matching items and ancestors
        if !filter_lower.is_empty() && !self.filter_match_paths.is_empty() {
            // Build set of paths to keep (matches + their ancestors)
            let mut paths_to_keep: HashSet<String> = HashSet::new();

            for path in &self.filter_match_paths {
                paths_to_keep.insert(path.clone());
                // Add all ancestor paths
                let mut p = path.as_str();
                while let Some((parent, _)) = p.rsplit_once('.') {
                    paths_to_keep.insert(parent.to_string());
                    p = parent;
                }
            }

            // Filter items to only those in paths_to_keep
            self.items.retain(|item| {
                // Keep LoadMore items if their parent category has matches
                if item.kind == NodeKind::LoadMore {
                    let parent_path = item.path.rsplit_once('.').map(|(p, _)| p.to_string());
                    return parent_path
                        .map(|p| paths_to_keep.contains(&p))
                        .unwrap_or(false);
                }
                // Keep items in paths_to_keep (includes ancestors and matches)
                paths_to_keep.contains(&item.path)
            });
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
                        "SELECT * FROM \"{}\".\"{}\" LIMIT {}",
                        schema, name, self.preview_rows
                    ))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Return the qualified name of the selected item for copying.
    /// Returns schema.table, schema.table.column, function name, etc.
    pub fn selected_qualified_name(&self) -> Option<String> {
        let item = self.items.get(self.selected)?;
        match item.kind {
            NodeKind::Schema => Some(format!("\"{}\"", item.label)),
            NodeKind::Table | NodeKind::View => {
                // Path format: "schema.Tables.tablename" or "schema.Views.viewname"
                let parts: Vec<&str> = item.path.splitn(3, '.').collect();
                if parts.len() == 3 {
                    Some(format!("\"{}\".\"{}\"", parts[0], parts[2]))
                } else {
                    None
                }
            }
            NodeKind::Column => {
                // Path format: "schema.Tables.tablename.columnname"
                let parts: Vec<&str> = item.path.splitn(4, '.').collect();
                if parts.len() == 4 {
                    // Return just the column name (most common use case)
                    // User can copy table separately if they need qualified
                    Some(format!("\"{}\"", parts[3]))
                } else {
                    None
                }
            }
            NodeKind::Function => {
                // Path format: "schema.Functions.funcname"
                let parts: Vec<&str> = item.path.splitn(3, '.').collect();
                if parts.len() == 3 {
                    Some(format!("\"{}\".\"{}\"", parts[0], parts[2]))
                } else {
                    None
                }
            }
            NodeKind::Index => {
                // Path format: "schema.Indexes.indexname"
                let parts: Vec<&str> = item.path.splitn(3, '.').collect();
                if parts.len() == 3 {
                    Some(format!("\"{}\"", parts[2]))
                } else {
                    None
                }
            }
            NodeKind::Category | NodeKind::LoadMore => None,
        }
    }

    /// Return schema and table name if a table/view is selected (for DDL lookup).
    pub fn selected_table_info(&self) -> Option<(String, String)> {
        let item = self.items.get(self.selected)?;
        match item.kind {
            NodeKind::Table | NodeKind::View => {
                let parts: Vec<&str> = item.path.splitn(3, '.').collect();
                if parts.len() == 3 {
                    Some((parts[0].to_string(), parts[2].to_string()))
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

    // ── Filter mode methods ──────────────────────────────────────

    /// Check if filter mode is currently active
    pub fn is_filter_active(&self) -> bool {
        self.filter_active
    }

    /// Get the current filter text
    pub fn filter_text(&self) -> &str {
        &self.filter_text
    }

    /// Get the cursor position in the filter text
    pub fn filter_cursor(&self) -> usize {
        self.filter_cursor
    }

    /// Check if a backend search is in progress
    pub fn is_searching(&self) -> bool {
        self.searching
    }

    /// Set the searching state
    pub fn set_searching(&mut self, searching: bool) {
        self.searching = searching;
    }

    /// Apply search results from the backend
    pub fn apply_search_results(&mut self, results: SchemaTree) {
        self.searching = false;
        // Save original schema before replacing with search results
        if self.pre_search_schema.is_none() {
            self.pre_search_schema = self.schema.take();
        }
        self.schema = Some(results);
        // Expand everything to show all search results
        if let Some(ref tree) = self.schema {
            for schema in &tree.schemas {
                self.expanded.insert(schema.name.clone());
                if !schema.tables.is_empty() {
                    self.expanded.insert(format!("{}.Tables", schema.name));
                }
                if !schema.views.is_empty() {
                    self.expanded.insert(format!("{}.Views", schema.name));
                }
                if !schema.functions.is_empty() {
                    self.expanded.insert(format!("{}.Functions", schema.name));
                }
                if !schema.indexes.is_empty() {
                    self.expanded.insert(format!("{}.Indexes", schema.name));
                }
                // Expand tables to show matching columns
                for table in &schema.tables {
                    self.expanded
                        .insert(format!("{}.Tables.{}", schema.name, table.name));
                }
                for view in &schema.views {
                    self.expanded
                        .insert(format!("{}.Views.{}", schema.name, view.name));
                }
            }
        }
        // Recompute filter matches and rebuild
        self.compute_filter_matches();
        self.rebuild_items();
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Activate filter mode
    pub fn activate_filter(&mut self) {
        // Save current expanded state if we're starting a fresh filter
        if self.pre_filter_expanded.is_none() {
            self.pre_filter_expanded = Some(self.expanded.clone());
        }
        self.filter_active = true;
        self.filter_cursor = self.filter_text.len();
    }

    /// Deactivate filter mode and clear filter
    pub fn deactivate_filter(&mut self) {
        self.filter_active = false;
        self.filter_text.clear();
        self.filter_cursor = 0;
        self.filter_match_paths.clear();
        self.searching = false;
        // Restore original schema if we did a backend search
        if let Some(original_schema) = self.pre_search_schema.take() {
            self.schema = Some(original_schema);
        }
        // Restore pre-filter expanded state
        if let Some(pre_expanded) = self.pre_filter_expanded.take() {
            self.expanded = pre_expanded;
        }
        self.rebuild_items();
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Insert a character at the cursor position in filter text
    pub fn filter_insert_char(&mut self, c: char) {
        self.filter_text.insert(self.filter_cursor, c);
        self.filter_cursor += c.len_utf8();
        self.apply_filter();
    }

    /// Delete the character before the cursor (backspace)
    pub fn filter_backspace(&mut self) {
        if self.filter_cursor > 0 {
            // Find the previous character boundary
            let prev_boundary = self.filter_text[..self.filter_cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.filter_text.remove(prev_boundary);
            self.filter_cursor = prev_boundary;
            self.apply_filter();
        }
    }

    /// Delete the character at the cursor (delete key)
    pub fn filter_delete(&mut self) {
        if self.filter_cursor < self.filter_text.len() {
            self.filter_text.remove(self.filter_cursor);
            self.apply_filter();
        }
    }

    /// Move filter cursor left
    pub fn filter_cursor_left(&mut self) {
        if self.filter_cursor > 0 {
            self.filter_cursor = self.filter_text[..self.filter_cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    /// Move filter cursor right
    pub fn filter_cursor_right(&mut self) {
        if self.filter_cursor < self.filter_text.len() {
            self.filter_cursor += self.filter_text[self.filter_cursor..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
        }
    }

    /// Apply the current filter text to rebuild visible items
    fn apply_filter(&mut self) {
        // Compute matches from schema data (not just visible items)
        self.compute_filter_matches();
        self.rebuild_items();
        // Reset selection to first matching item
        if !self.items.is_empty() {
            self.selected = 0;
            self.scroll_offset = 0;
        }
    }

    /// Scan schema data to find all paths that match the filter text.
    /// Also auto-expands parent paths so matches become visible.
    fn compute_filter_matches(&mut self) {
        self.filter_match_paths.clear();

        let filter_lower = self.filter_text.to_lowercase();
        if filter_lower.is_empty() {
            return;
        }

        let schema_tree = match &self.schema {
            Some(s) => s,
            None => return,
        };

        // Collect paths that match and paths that need to be expanded
        let mut paths_to_expand: HashSet<String> = HashSet::new();

        for schema in &schema_tree.schemas {
            let schema_path = schema.name.clone();

            // Check schema name
            if schema.name.to_lowercase().contains(&filter_lower) {
                self.filter_match_paths.insert(schema_path.clone());
            }

            // Check tables
            let tables_cat_path = format!("{}.Tables", schema.name);
            for table in &schema.tables {
                let table_path = format!("{}.{}", tables_cat_path, table.name);

                if table.name.to_lowercase().contains(&filter_lower) {
                    self.filter_match_paths.insert(table_path.clone());
                    // Expand parents to show this match
                    paths_to_expand.insert(schema_path.clone());
                    paths_to_expand.insert(tables_cat_path.clone());
                }

                // Check columns
                for col in &table.columns {
                    let col_path = format!("{}.{}", table_path, col.name);
                    if col.name.to_lowercase().contains(&filter_lower) {
                        self.filter_match_paths.insert(col_path);
                        // Expand parents to show this match
                        paths_to_expand.insert(schema_path.clone());
                        paths_to_expand.insert(tables_cat_path.clone());
                        paths_to_expand.insert(table_path.clone());
                    }
                }
            }

            // Check views
            let views_cat_path = format!("{}.Views", schema.name);
            for view in &schema.views {
                let view_path = format!("{}.{}", views_cat_path, view.name);

                if view.name.to_lowercase().contains(&filter_lower) {
                    self.filter_match_paths.insert(view_path.clone());
                    paths_to_expand.insert(schema_path.clone());
                    paths_to_expand.insert(views_cat_path.clone());
                }

                // Check view columns
                for col in &view.columns {
                    let col_path = format!("{}.{}", view_path, col.name);
                    if col.name.to_lowercase().contains(&filter_lower) {
                        self.filter_match_paths.insert(col_path);
                        paths_to_expand.insert(schema_path.clone());
                        paths_to_expand.insert(views_cat_path.clone());
                        paths_to_expand.insert(view_path.clone());
                    }
                }
            }

            // Check functions
            let funcs_cat_path = format!("{}.Functions", schema.name);
            for func in &schema.functions {
                let func_path = format!("{}.{}", funcs_cat_path, func.name);
                if func.name.to_lowercase().contains(&filter_lower) {
                    self.filter_match_paths.insert(func_path);
                    paths_to_expand.insert(schema_path.clone());
                    paths_to_expand.insert(funcs_cat_path.clone());
                }
            }

            // Check indexes
            let idx_cat_path = format!("{}.Indexes", schema.name);
            for idx in &schema.indexes {
                let idx_path = format!("{}.{}", idx_cat_path, idx.name);
                if idx.name.to_lowercase().contains(&filter_lower) {
                    self.filter_match_paths.insert(idx_path);
                    paths_to_expand.insert(schema_path.clone());
                    paths_to_expand.insert(idx_cat_path.clone());
                }
            }
        }

        // Auto-expand paths to show matches
        for path in paths_to_expand {
            self.expanded.insert(path);
        }
    }

    /// Check if currently selected item is a LoadMore pseudo-item
    pub fn is_load_more_selected(&self) -> bool {
        self.items
            .get(self.selected)
            .map(|item| item.kind == NodeKind::LoadMore)
            .unwrap_or(false)
    }

    /// Get the schema and category for the selected LoadMore item
    /// Returns (schema_name, category) e.g., ("public", "Tables")
    pub fn load_more_info(&self) -> Option<(String, String)> {
        let item = self.items.get(self.selected)?;
        if item.kind != NodeKind::LoadMore {
            return None;
        }
        // Path format: "schema.Category.__load_more__"
        let parts: Vec<&str> = item.path.splitn(3, '.').collect();
        if parts.len() >= 2 {
            Some((parts[0].to_string(), parts[1].to_string()))
        } else {
            None
        }
    }

    /// Get the current loaded count for a category in a schema
    pub fn loaded_count(&self, schema_name: &str, category: &str) -> usize {
        if let Some(schema) = self.schema.as_ref()
            && let Some(s) = schema.schemas.iter().find(|s| s.name == schema_name)
        {
            return match category {
                "Tables" => s.tables.len(),
                "Views" => s.views.len(),
                "Functions" => s.functions.len(),
                "Indexes" => s.indexes.len(),
                _ => 0,
            };
        }
        0
    }

    /// Extend a category with more loaded items
    pub fn extend_tables(&mut self, schema_name: &str, items: Vec<crate::db::schema::Table>) {
        if let Some(schema) = self.schema.as_mut()
            && let Some(s) = schema
                .schemas
                .items
                .iter_mut()
                .find(|s| s.name == schema_name)
        {
            s.tables.extend(items);
        }
        self.rebuild_items();
    }

    /// Extend views with more loaded items
    pub fn extend_views(&mut self, schema_name: &str, items: Vec<crate::db::schema::Table>) {
        if let Some(schema) = self.schema.as_mut()
            && let Some(s) = schema
                .schemas
                .items
                .iter_mut()
                .find(|s| s.name == schema_name)
        {
            s.views.extend(items);
        }
        self.rebuild_items();
    }

    /// Extend functions with more loaded items
    pub fn extend_functions(&mut self, schema_name: &str, items: Vec<crate::db::schema::Function>) {
        if let Some(schema) = self.schema.as_mut()
            && let Some(s) = schema
                .schemas
                .items
                .iter_mut()
                .find(|s| s.name == schema_name)
        {
            s.functions.extend(items);
        }
        self.rebuild_items();
    }

    /// Extend indexes with more loaded items
    pub fn extend_indexes(&mut self, schema_name: &str, items: Vec<crate::db::schema::Index>) {
        if let Some(schema) = self.schema.as_mut()
            && let Some(s) = schema
                .schemas
                .items
                .iter_mut()
                .find(|s| s.name == schema_name)
        {
            s.indexes.extend(items);
        }
        self.rebuild_items();
    }

    /// Get the category limit from settings
    pub fn category_limit(&self) -> usize {
        self.category_limit
    }
}

/// Push column items into the flat item list
fn push_columns(
    items: &mut Vec<TreeItem>,
    columns: &[crate::db::schema::Column],
    parent_path: &str,
    depth: usize,
    filter_match_paths: &HashSet<String>,
) {
    for col in columns {
        let col_path = format!("{}.{}", parent_path, col.name);
        let col_label = format_column_label(col);
        items.push(TreeItem {
            label: col_label,
            kind: NodeKind::Column,
            depth,
            path: col_path.clone(),
            expandable: false,
            matches_filter: filter_match_paths.contains(&col_path),
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
        // Reserve space for filter bar if active
        let (tree_area, filter_area) = if self.filter_active {
            let filter_height = 1;
            let tree_height = area.height.saturating_sub(filter_height);
            (
                Rect::new(area.x, area.y, area.width, tree_height),
                Some(Rect::new(
                    area.x,
                    area.y + tree_height,
                    area.width,
                    filter_height,
                )),
            )
        } else {
            (area, None)
        };

        // Render filter bar if active
        if let Some(filter_area) = filter_area {
            let status_hint = if self.searching {
                Span::styled(" Searching...", Style::default().fg(Color::Yellow))
            } else if !self.filter_text.is_empty() {
                Span::styled(" (Enter=search)", Style::default().fg(Color::DarkGray))
            } else {
                Span::raw("")
            };

            let filter_line = Line::from(vec![
                Span::styled("/", theme.tree_filter_bar),
                Span::styled(&self.filter_text, theme.tree_filter_text),
                if focused && !self.searching {
                    Span::styled("█", theme.tree_filter_text)
                } else {
                    Span::raw("")
                },
                status_hint,
            ]);
            // Fill background
            let bg_fill = " ".repeat(filter_area.width as usize);
            frame.render_widget(
                Paragraph::new(bg_fill).style(theme.tree_filter_bar),
                filter_area,
            );
            frame.render_widget(Paragraph::new(filter_line), filter_area);
        }

        if self.items.is_empty() {
            let msg = if self.schema.is_some() {
                if !self.filter_text.is_empty() {
                    "No matches"
                } else {
                    "No schemas found"
                }
            } else {
                "Not connected"
            };
            let p = Paragraph::new(msg).style(theme.tree_empty);
            frame.render_widget(p, tree_area);
            return;
        }

        let visible_height = tree_area.height as usize;
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
            let y = tree_area.y + vis_row as u16;

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
            let max_chars = tree_area.width as usize;
            let display_chars = display.chars().count();
            let truncated = if display_chars > max_chars {
                let truncated: String = display.chars().take(max_chars.saturating_sub(3)).collect();
                format!("{}...", truncated)
            } else {
                // Pad with spaces to fill width
                let padding = max_chars.saturating_sub(display_chars);
                format!("{}{}", display, " ".repeat(padding))
            };

            let style = if is_selected {
                theme.tree_selected
            } else if item.matches_filter {
                // Highlight items that match the filter
                theme.tree_filter_match
            } else {
                match item.kind {
                    NodeKind::Schema => theme.tree_schema,
                    NodeKind::Category => theme.tree_category,
                    NodeKind::Table => theme.tree_table,
                    NodeKind::View => theme.tree_view,
                    NodeKind::Column => theme.tree_column,
                    NodeKind::Function => theme.tree_function,
                    NodeKind::Index => theme.tree_index,
                    NodeKind::LoadMore => theme.tree_load_more,
                }
            };

            frame.render_widget(
                Paragraph::new(truncated).style(style),
                Rect::new(tree_area.x, y, tree_area.width, 1),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::{Column, ForeignKey, Function, Index, PaginatedVec, Schema, Table};
    use crate::db::types::DataType;

    fn sample_schema() -> SchemaTree {
        SchemaTree {
            schemas: PaginatedVec::from_vec(vec![Schema {
                name: "public".to_string(),
                tables: PaginatedVec::from_vec(vec![
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
                ]),
                views: PaginatedVec::from_vec(vec![Table {
                    name: "active_users".to_string(),
                    columns: vec![Column {
                        name: "id".to_string(),
                        data_type: DataType::Integer,
                        is_primary_key: false,
                        foreign_key: None,
                    }],
                }]),
                indexes: PaginatedVec::from_vec(vec![Index {
                    name: "users_pkey".to_string(),
                    columns: vec!["id".to_string()],
                    is_unique: true,
                    is_primary: true,
                    table_name: "users".to_string(),
                }]),
                functions: PaginatedVec::from_vec(vec![Function {
                    name: "get_user".to_string(),
                    args: "integer".to_string(),
                    return_type: "users".to_string(),
                }]),
            }]),
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
        // Categories now include counts, e.g., "Tables (2)"
        assert!(labels.iter().any(|l| l.starts_with("Tables (")));
        assert!(labels.iter().any(|l| l.starts_with("Views (")));
        assert!(labels.iter().any(|l| l.starts_with("Functions (")));
        assert!(labels.iter().any(|l| l.starts_with("Indexes (")));
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
        let views_idx = tree
            .items
            .iter()
            .position(|i| i.label.starts_with("Views ("))
            .unwrap();
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
            .position(|i| i.label.starts_with("Functions ("))
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
            .position(|i| i.label.starts_with("Indexes ("))
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
        // Expand Views category first (now includes count in label)
        let views_idx = tree
            .items
            .iter()
            .position(|i| i.label.starts_with("Views ("))
            .unwrap();
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
        // Category labels now include count, e.g., "Tables (2)"
        let tables_idx = tree
            .items
            .iter()
            .position(|i| i.label.starts_with("Tables ("))
            .unwrap();
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
            schemas: PaginatedVec::from_vec(vec![Schema {
                name: "empty".to_string(),
                tables: PaginatedVec::from_vec(vec![Table {
                    name: "t".to_string(),
                    columns: vec![],
                }]),
                views: PaginatedVec::default(),
                indexes: PaginatedVec::default(),
                functions: PaginatedVec::default(),
            }]),
        };
        let mut tree = TreeBrowser::new();
        tree.set_schema(schema);
        let labels: Vec<&str> = tree.items.iter().map(|i| i.label.as_str()).collect();
        // Categories now include counts, e.g., "Tables (1)"
        assert!(labels.iter().any(|l| l.starts_with("Tables (")));
        assert!(!labels.iter().any(|l| l.starts_with("Views (")));
        assert!(!labels.iter().any(|l| l.starts_with("Functions (")));
        assert!(!labels.iter().any(|l| l.starts_with("Indexes (")));
    }

    #[test]
    fn test_preview_query_uses_configured_limit() {
        let mut tree = TreeBrowser::with_preview_rows(50);
        tree.set_schema(sample_schema());
        let users_idx = tree.items.iter().position(|i| i.label == "users").unwrap();
        tree.selected = users_idx;
        assert_eq!(
            tree.preview_query(),
            Some("SELECT * FROM \"public\".\"users\" LIMIT 50".to_string())
        );
    }

    #[test]
    fn test_selected_qualified_name_for_table() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        let users_idx = tree.items.iter().position(|i| i.label == "users").unwrap();
        tree.selected = users_idx;
        assert_eq!(
            tree.selected_qualified_name(),
            Some("\"public\".\"users\"".to_string())
        );
    }

    #[test]
    fn test_selected_qualified_name_for_column() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        // Find users table and expand it
        let users_idx = tree.items.iter().position(|i| i.label == "users").unwrap();
        tree.selected = users_idx;
        tree.expand_current();
        // Find the id column
        let col_idx = tree
            .items
            .iter()
            .position(|i| i.label.starts_with("* id"))
            .unwrap();
        tree.selected = col_idx;
        assert_eq!(tree.selected_qualified_name(), Some("\"id\"".to_string()));
    }

    #[test]
    fn test_selected_qualified_name_for_schema() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        tree.selected = 0; // First item is schema
        assert_eq!(
            tree.selected_qualified_name(),
            Some("\"public\"".to_string())
        );
    }

    #[test]
    fn test_selected_qualified_name_none_for_category() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        // Expand schema to see categories
        tree.expand_current();
        // Find Tables category
        let cat_idx = tree
            .items
            .iter()
            .position(|i| i.label.starts_with("Tables ("))
            .unwrap();
        tree.selected = cat_idx;
        assert_eq!(tree.selected_qualified_name(), None);
    }

    #[test]
    fn test_selected_table_info_for_table() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        let users_idx = tree.items.iter().position(|i| i.label == "users").unwrap();
        tree.selected = users_idx;
        assert_eq!(
            tree.selected_table_info(),
            Some(("public".to_string(), "users".to_string()))
        );
    }

    #[test]
    fn test_selected_table_info_none_for_column() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        let users_idx = tree.items.iter().position(|i| i.label == "users").unwrap();
        tree.selected = users_idx;
        tree.expand_current();
        let col_idx = tree
            .items
            .iter()
            .position(|i| i.label.starts_with("* id"))
            .unwrap();
        tree.selected = col_idx;
        assert_eq!(tree.selected_table_info(), None);
    }

    // ── Filter tests ────────────────────────────────────────────────

    #[test]
    fn test_filter_mode_activation() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        assert!(!tree.is_filter_active());
        tree.activate_filter();
        assert!(tree.is_filter_active());
        tree.deactivate_filter();
        assert!(!tree.is_filter_active());
    }

    #[test]
    fn test_filter_text_input() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        tree.activate_filter();
        tree.filter_insert_char('u');
        tree.filter_insert_char('s');
        tree.filter_insert_char('e');
        assert_eq!(tree.filter_text(), "use");
        tree.filter_backspace();
        assert_eq!(tree.filter_text(), "us");
    }

    #[test]
    fn test_filter_shows_matches_and_expands_ancestors() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());

        tree.activate_filter();
        tree.filter_insert_char('u');
        tree.filter_insert_char('s');
        tree.filter_insert_char('e');
        tree.filter_insert_char('r');

        // Should contain all items matching "user"
        assert!(tree.items.iter().any(|i| i.label == "users")); // table
        assert!(tree.items.iter().any(|i| i.label.contains("user_id"))); // column
        assert!(tree.items.iter().any(|i| i.label == "active_users")); // view
        assert!(tree.items.iter().any(|i| i.label.starts_with("users_pkey"))); // index
        assert!(tree.items.iter().any(|i| i.label.starts_with("get_user"))); // function

        // Matching items should have matches_filter = true
        let users_item = tree.items.iter().find(|i| i.label == "users").unwrap();
        assert!(users_item.matches_filter);

        // Ancestor items (like schema, categories) should have matches_filter = false
        let public_item = tree.items.iter().find(|i| i.label == "public").unwrap();
        assert!(!public_item.matches_filter);
    }

    #[test]
    fn test_filter_clears_on_deactivate() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        tree.activate_filter();
        tree.filter_insert_char('x');
        tree.filter_insert_char('y');
        tree.filter_insert_char('z');

        tree.deactivate_filter();
        assert_eq!(tree.filter_text(), "");
        assert!(!tree.is_filter_active());
    }

    #[test]
    fn test_filter_cursor_movement() {
        let mut tree = TreeBrowser::new();
        tree.activate_filter();
        tree.filter_insert_char('a');
        tree.filter_insert_char('b');
        tree.filter_insert_char('c');
        assert_eq!(tree.filter_cursor(), 3);

        tree.filter_cursor_left();
        assert_eq!(tree.filter_cursor(), 2);

        tree.filter_cursor_right();
        assert_eq!(tree.filter_cursor(), 3);
    }

    // ── Backend search tests ────────────────────────────────────────

    #[test]
    fn test_searching_state() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        assert!(!tree.is_searching());

        tree.set_searching(true);
        assert!(tree.is_searching());

        tree.set_searching(false);
        assert!(!tree.is_searching());
    }

    #[test]
    fn test_apply_search_results_replaces_schema() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        tree.activate_filter();
        tree.filter_insert_char('x');

        // Simulate backend search results with different data
        let search_results = SchemaTree {
            schemas: PaginatedVec::from_vec(vec![Schema {
                name: "search_schema".to_string(),
                tables: PaginatedVec::from_vec(vec![Table {
                    name: "search_table".to_string(),
                    columns: vec![],
                }]),
                views: PaginatedVec::default(),
                indexes: PaginatedVec::default(),
                functions: PaginatedVec::default(),
            }]),
        };

        tree.apply_search_results(search_results);

        // Should show search results
        assert!(tree.items.iter().any(|i| i.label == "search_schema"));
        assert!(tree.items.iter().any(|i| i.label == "search_table"));
        // Original schema should not be visible
        assert!(!tree.items.iter().any(|i| i.label == "public"));
    }

    #[test]
    fn test_deactivate_filter_restores_original_schema_after_search() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        let original_item_count = tree.items.len();

        tree.activate_filter();
        tree.filter_insert_char('x');

        // Simulate backend search returning different schema
        let search_results = SchemaTree {
            schemas: PaginatedVec::from_vec(vec![Schema {
                name: "other".to_string(),
                tables: PaginatedVec::from_vec(vec![Table {
                    name: "other_table".to_string(),
                    columns: vec![],
                }]),
                views: PaginatedVec::default(),
                indexes: PaginatedVec::default(),
                functions: PaginatedVec::default(),
            }]),
        };
        tree.apply_search_results(search_results);

        // Now deactivate - should restore original schema
        tree.deactivate_filter();

        // Original "public" schema should be back
        assert!(tree.items.iter().any(|i| i.label == "public"));
        // Search result schema should not be visible
        assert!(!tree.items.iter().any(|i| i.label == "other"));
        // Item count should be similar to original (may differ due to expansion state)
        assert!(tree.items.len() >= original_item_count / 2);
    }

    #[test]
    fn test_search_results_auto_expand_all() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        tree.activate_filter();

        // Simulate search results with nested structure
        let search_results = SchemaTree {
            schemas: PaginatedVec::from_vec(vec![Schema {
                name: "test_schema".to_string(),
                tables: PaginatedVec::from_vec(vec![Table {
                    name: "test_table".to_string(),
                    columns: vec![Column {
                        name: "test_col".to_string(),
                        data_type: DataType::Text,
                        is_primary_key: false,
                        foreign_key: None,
                    }],
                }]),
                views: PaginatedVec::default(),
                indexes: PaginatedVec::default(),
                functions: PaginatedVec::from_vec(vec![Function {
                    name: "test_func".to_string(),
                    args: "".to_string(),
                    return_type: "void".to_string(),
                }]),
            }]),
        };

        tree.apply_search_results(search_results);

        // All items should be visible (auto-expanded)
        assert!(tree.items.iter().any(|i| i.label == "test_schema"));
        assert!(tree.items.iter().any(|i| i.label.starts_with("Tables (")));
        assert!(tree.items.iter().any(|i| i.label == "test_table"));
        assert!(tree.items.iter().any(|i| i.label.contains("test_col")));
        assert!(
            tree.items
                .iter()
                .any(|i| i.label.starts_with("Functions ("))
        );
        assert!(tree.items.iter().any(|i| i.label.starts_with("test_func")));
    }

    #[test]
    fn test_searching_cleared_on_deactivate() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        tree.activate_filter();
        tree.set_searching(true);
        assert!(tree.is_searching());

        tree.deactivate_filter();
        assert!(!tree.is_searching());
    }

    #[test]
    fn test_category_limit_stored() {
        let tree = TreeBrowser::with_settings(100, 250);
        assert_eq!(tree.category_limit(), 250);
    }

    #[test]
    fn test_loaded_count_returns_correct_counts() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        // sample_schema has 2 tables, 1 view, 1 function, 1 index
        assert_eq!(tree.loaded_count("public", "Tables"), 2);
        assert_eq!(tree.loaded_count("public", "Views"), 1);
        assert_eq!(tree.loaded_count("public", "Functions"), 1);
        assert_eq!(tree.loaded_count("public", "Indexes"), 1);
        assert_eq!(tree.loaded_count("nonexistent", "Tables"), 0);
    }

    #[test]
    fn test_load_more_info_returns_schema_and_category() {
        let mut tree = TreeBrowser::new();

        // Create a truncated schema that will show "Load more" item
        let mut schema = sample_schema();
        schema.schemas.items[0].tables = PaginatedVec::new(
            schema.schemas.items[0].tables.items.clone(),
            100, // total_count > items.len()
        );

        tree.set_schema(schema);

        // Expand public schema
        tree.expand_current(); // Schema
        tree.move_down();
        tree.expand_current(); // Tables category

        // Find and select the LoadMore item
        let load_more_idx = tree.items.iter().position(|i| i.kind == NodeKind::LoadMore);
        if let Some(idx) = load_more_idx {
            tree.selected = idx;
            assert!(tree.is_load_more_selected());
            let info = tree.load_more_info();
            assert!(info.is_some());
            let (schema, category) = info.unwrap();
            assert_eq!(schema, "public");
            assert_eq!(category, "Tables");
        }
    }

    #[test]
    fn test_extend_tables_adds_items() {
        let mut tree = TreeBrowser::new();
        tree.set_schema(sample_schema());
        let initial_count = tree.loaded_count("public", "Tables");

        let new_table = Table {
            name: "new_table".to_string(),
            columns: vec![],
        };
        tree.extend_tables("public", vec![new_table]);

        assert_eq!(tree.loaded_count("public", "Tables"), initial_count + 1);
    }

    #[test]
    fn test_truncated_category_shows_load_more() {
        let mut tree = TreeBrowser::new();

        // Create schema where tables are truncated
        let mut schema = sample_schema();
        schema.schemas.items[0].tables = PaginatedVec::new(
            vec![Table {
                name: "table1".to_string(),
                columns: vec![],
            }],
            10, // total is 10, but only 1 loaded
        );

        tree.set_schema(schema);
        tree.expand_current(); // Schema
        tree.move_down();
        tree.expand_current(); // Tables category

        // Should have a LoadMore item
        let has_load_more = tree.items.iter().any(|i| i.kind == NodeKind::LoadMore);
        assert!(
            has_load_more,
            "Truncated category should show Load more item"
        );

        // Category label should show counts
        let tables_category = tree
            .items
            .iter()
            .find(|i| i.kind == NodeKind::Category && i.path.contains("Tables"));
        assert!(tables_category.is_some());
        assert!(
            tables_category.unwrap().label.contains("1 of 10"),
            "Category should show 'X of Y' when truncated"
        );
    }
}
