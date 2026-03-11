//! EXPLAIN ANALYZE tree viewer
//!
//! Parses JSON-format EXPLAIN output into a navigable tree with color-coded
//! timing information. Supports toggling between visual tree and raw text view.

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use std::cell::Cell;
use std::time::Duration;

use crate::ui::Component;
use crate::ui::theme::Theme;

/// A single node in the query plan tree
#[derive(Debug, Clone)]
struct PlanNode {
    node_type: String,
    relation: Option<String>,
    startup_cost: f64,
    total_cost: f64,
    plan_rows: u64,
    actual_rows: Option<u64>,
    actual_time: Option<f64>,
    actual_loops: Option<u64>,
    children: Vec<PlanNode>,
}

/// Flattened row for rendering — one per visible line
#[derive(Debug, Clone)]
struct ExplainRow {
    depth: usize,
    node_type: String,
    relation: Option<String>,
    actual_rows: Option<u64>,
    actual_time: Option<f64>,
    total_cost: f64,
    has_children: bool,
}

/// View mode for the explain viewer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Tree,
    RawText,
}

/// EXPLAIN ANALYZE tree viewer component
#[derive(Debug)]
pub struct ExplainViewer {
    rows: Vec<ExplainRow>,
    raw_text: String,
    planning_time: Option<f64>,
    execution_time_ms: Option<f64>,
    query_duration: Duration,
    selected: usize,
    scroll_offset: usize,
    view_mode: ViewMode,
    /// Maximum actual_time across all nodes, for color scaling
    max_time: f64,
    /// Cached line count for raw text view (avoids O(n) recount on every keystroke)
    raw_text_line_count: usize,
    page_height: Cell<usize>,
}

impl ExplainViewer {
    /// Parse JSON EXPLAIN output into an ExplainViewer.
    /// Returns None if the JSON doesn't look like EXPLAIN output.
    pub fn from_json(json_str: &str, query_duration: Duration) -> Option<Self> {
        let parsed: serde_json::Value = serde_json::from_str(json_str).ok()?;
        let arr = parsed.as_array()?;
        let root = arr.first()?.as_object()?;
        let plan = root.get("Plan")?;

        let planning_time = root.get("Planning Time").and_then(|v| v.as_f64());
        let execution_time_ms = root.get("Execution Time").and_then(|v| v.as_f64());

        let root_node = Self::parse_node(plan)?;

        let mut rows = Vec::new();
        let mut max_time: f64 = 0.0;
        Self::flatten(&root_node, 0, &mut rows, &mut max_time);

        // Build raw text representation
        let raw_text = Self::build_raw_text(&root_node, planning_time, execution_time_ms);
        let raw_text_line_count = raw_text.lines().count();

        Some(Self {
            rows,
            raw_text,
            planning_time,
            execution_time_ms,
            query_duration,
            selected: 0,
            scroll_offset: 0,
            view_mode: ViewMode::Tree,
            max_time,
            raw_text_line_count,
            page_height: Cell::new(20),
        })
    }

    fn parse_node(value: &serde_json::Value) -> Option<PlanNode> {
        let obj = value.as_object()?;
        let node_type = obj.get("Node Type")?.as_str()?.to_string();
        let relation = obj
            .get("Relation Name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let startup_cost = obj
            .get("Startup Cost")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let total_cost = obj
            .get("Total Cost")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let plan_rows = obj.get("Plan Rows").and_then(|v| v.as_u64()).unwrap_or(0);
        let actual_rows = obj.get("Actual Rows").and_then(|v| v.as_u64());
        let actual_time = obj.get("Actual Total Time").and_then(|v| v.as_f64());
        let actual_loops = obj.get("Actual Loops").and_then(|v| v.as_u64());

        let children = obj
            .get("Plans")
            .and_then(|v| v.as_array())
            .map(|plans| plans.iter().filter_map(Self::parse_node).collect())
            .unwrap_or_default();

        Some(PlanNode {
            node_type,
            relation,
            startup_cost,
            total_cost,
            plan_rows,
            actual_rows,
            actual_time,
            actual_loops,
            children,
        })
    }

    fn flatten(node: &PlanNode, depth: usize, rows: &mut Vec<ExplainRow>, max_time: &mut f64) {
        if let Some(t) = node.actual_time
            && t > *max_time
        {
            *max_time = t;
        }
        rows.push(ExplainRow {
            depth,
            node_type: node.node_type.clone(),
            relation: node.relation.clone(),
            actual_rows: node.actual_rows,
            actual_time: node.actual_time,
            total_cost: node.total_cost,
            has_children: !node.children.is_empty(),
        });
        for child in &node.children {
            Self::flatten(child, depth + 1, rows, max_time);
        }
    }

    fn build_raw_text(
        node: &PlanNode,
        planning_time: Option<f64>,
        execution_time: Option<f64>,
    ) -> String {
        let mut lines = Vec::new();
        Self::raw_text_node(node, 0, &mut lines);
        if let Some(pt) = planning_time {
            lines.push(format!("Planning Time: {:.3} ms", pt));
        }
        if let Some(et) = execution_time {
            lines.push(format!("Execution Time: {:.3} ms", et));
        }
        lines.join("\n")
    }

    fn raw_text_node(node: &PlanNode, depth: usize, lines: &mut Vec<String>) {
        let indent = if depth == 0 {
            String::new()
        } else {
            format!("{}->  ", "      ".repeat(depth - 1))
        };
        let relation = node
            .relation
            .as_deref()
            .map(|r| format!(" on {}", r))
            .unwrap_or_default();
        let actual = if let (Some(time), Some(rows), Some(loops)) =
            (node.actual_time, node.actual_rows, node.actual_loops)
        {
            format!(
                " (actual time={:.3}..{:.3} rows={} loops={})",
                node.startup_cost, time, rows, loops
            )
        } else {
            String::new()
        };
        lines.push(format!(
            "{}{}{} (cost={:.2}..{:.2} rows={}){}",
            indent,
            node.node_type,
            relation,
            node.startup_cost,
            node.total_cost,
            node.plan_rows,
            actual
        ));
        for child in &node.children {
            Self::raw_text_node(child, depth + 1, lines);
        }
    }

    // Navigation

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        let max = match self.view_mode {
            ViewMode::Tree => self.rows.len().saturating_sub(1),
            ViewMode::RawText => self.raw_text_line_count.saturating_sub(1),
        };
        if self.selected < max {
            self.selected += 1;
        }
    }

    pub fn page_up(&mut self) {
        let page = self.page_height.get().max(1);
        self.selected = self.selected.saturating_sub(page);
    }

    pub fn page_down(&mut self) {
        let page = self.page_height.get().max(1);
        let max = match self.view_mode {
            ViewMode::Tree => self.rows.len().saturating_sub(1),
            ViewMode::RawText => self.raw_text_line_count.saturating_sub(1),
        };
        self.selected = (self.selected + page).min(max);
    }

    pub fn go_to_top(&mut self) {
        self.selected = 0;
    }

    pub fn go_to_bottom(&mut self) {
        let max = match self.view_mode {
            ViewMode::Tree => self.rows.len().saturating_sub(1),
            ViewMode::RawText => self.raw_text_line_count.saturating_sub(1),
        };
        self.selected = max;
    }

    pub fn toggle_view_mode(&mut self) {
        self.view_mode = match self.view_mode {
            ViewMode::Tree => ViewMode::RawText,
            ViewMode::RawText => ViewMode::Tree,
        };
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Color for a node based on its actual time relative to max
    fn time_color(&self, time: f64) -> Color {
        if self.max_time <= 0.0 {
            return Color::Green;
        }
        let ratio = time / self.max_time;
        if ratio > 0.6 {
            Color::Red
        } else if ratio > 0.2 {
            Color::Yellow
        } else {
            Color::Green
        }
    }

    fn format_time(ms: f64) -> String {
        if ms >= 1000.0 {
            format!("{:.2}s", ms / 1000.0)
        } else if ms >= 1.0 {
            format!("{:.1}ms", ms)
        } else {
            format!("{:.0}µs", ms * 1000.0)
        }
    }

    fn format_rows(count: u64) -> String {
        if count >= 1_000_000 {
            format!("{:.1}M", count as f64 / 1_000_000.0)
        } else if count >= 1_000 {
            format!("{:.1}K", count as f64 / 1_000.0)
        } else {
            count.to_string()
        }
    }
}

impl Component for ExplainViewer {
    fn render(&self, frame: &mut Frame, area: Rect, focused: bool, theme: &Theme) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        // Reserve last line for footer
        let content_height = area.height.saturating_sub(1) as usize;
        self.page_height.set(content_height);

        // Ensure selected is visible
        let scroll_offset = if self.selected < self.scroll_offset {
            self.selected
        } else if self.selected >= self.scroll_offset + content_height {
            self.selected - content_height + 1
        } else {
            self.scroll_offset
        };

        match self.view_mode {
            ViewMode::Tree => {
                self.render_tree(frame, area, content_height, scroll_offset, focused, theme)
            }
            ViewMode::RawText => {
                self.render_raw(frame, area, content_height, scroll_offset, focused, theme)
            }
        }

        // Footer
        let footer_y = area.y + area.height.saturating_sub(1);
        let footer_area = Rect::new(area.x, footer_y, area.width, 1);

        let mut footer_spans = vec![Span::styled(
            format!(
                " Total: {} ",
                Self::format_time(self.query_duration.as_secs_f64() * 1000.0)
            ),
            Style::default().fg(Color::DarkGray),
        )];
        if let Some(pt) = self.planning_time {
            footer_spans.push(Span::styled(
                format!("Plan: {} ", Self::format_time(pt)),
                Style::default().fg(Color::DarkGray),
            ));
        }
        if let Some(et) = self.execution_time_ms {
            footer_spans.push(Span::styled(
                format!("Exec: {} ", Self::format_time(et)),
                Style::default().fg(Color::DarkGray),
            ));
        }
        footer_spans.push(Span::styled(
            format!("│ {} nodes │ t=toggle raw ", self.rows.len()),
            Style::default().fg(Color::DarkGray),
        ));
        let mode_label = match self.view_mode {
            ViewMode::Tree => "TREE",
            ViewMode::RawText => "TEXT",
        };
        footer_spans.push(Span::styled(
            format!("[{}]", mode_label),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

        frame.render_widget(Paragraph::new(Line::from(footer_spans)), footer_area);
    }
}

impl ExplainViewer {
    fn render_tree(
        &self,
        frame: &mut Frame,
        area: Rect,
        content_height: usize,
        scroll_offset: usize,
        focused: bool,
        theme: &Theme,
    ) {
        let _ = theme; // tree view uses its own color scheme

        for vis_row in 0..content_height {
            let item_idx = scroll_offset + vis_row;
            let y = area.y + vis_row as u16;
            if item_idx >= self.rows.len() {
                break;
            }

            let row = &self.rows[item_idx];
            let is_selected = focused && item_idx == self.selected;

            // Build tree prefix
            let indent = "  ".repeat(row.depth);
            let indicator = if row.has_children {
                "├─ "
            } else {
                "── "
            };

            // Node type + relation
            let label = match &row.relation {
                Some(rel) => format!("{} on {}", row.node_type, rel),
                None => row.node_type.clone(),
            };

            // Metrics
            let time_str = row.actual_time.map(Self::format_time).unwrap_or_default();
            let rows_str = row
                .actual_rows
                .map(|r| format!("{} rows", Self::format_rows(r)))
                .unwrap_or_default();
            let cost_str = format!("cost {:.0}", row.total_cost);

            // Build spans
            let mut spans = Vec::new();

            if is_selected {
                // Selected row: uniform highlight
                let full = format!(
                    "{}{}{}  {}  {}  {}",
                    indent, indicator, label, time_str, rows_str, cost_str
                );
                let padded = format!("{:<width$}", full, width = area.width as usize);
                spans.push(Span::styled(
                    padded,
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                // Tree prefix
                spans.push(Span::styled(
                    format!("{}{}", indent, indicator),
                    Style::default().fg(Color::DarkGray),
                ));
                // Node type
                spans.push(Span::styled(
                    label,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ));
                // Time (color-coded)
                if let Some(time) = row.actual_time {
                    spans.push(Span::styled(
                        format!("  {}", time_str),
                        Style::default().fg(self.time_color(time)),
                    ));
                }
                // Rows
                if !rows_str.is_empty() {
                    spans.push(Span::styled(
                        format!("  {}", rows_str),
                        Style::default().fg(Color::Gray),
                    ));
                }
                // Cost
                spans.push(Span::styled(
                    format!("  {}", cost_str),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            let line_area = Rect::new(area.x, y, area.width, 1);
            frame.render_widget(Paragraph::new(Line::from(spans)), line_area);
        }
    }

    fn render_raw(
        &self,
        frame: &mut Frame,
        area: Rect,
        content_height: usize,
        scroll_offset: usize,
        focused: bool,
        _theme: &Theme,
    ) {
        let lines: Vec<&str> = self.raw_text.lines().collect();
        for vis_row in 0..content_height {
            let item_idx = scroll_offset + vis_row;
            let y = area.y + vis_row as u16;
            if item_idx >= lines.len() {
                break;
            }

            let is_selected = focused && item_idx == self.selected;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let text = lines[item_idx];
            let display = format!("{:<width$}", text, width = area.width as usize);
            let line_area = Rect::new(area.x, y, area.width, 1);
            frame.render_widget(Paragraph::new(display).style(style), line_area);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_explain_json() -> &'static str {
        r#"[
          {
            "Plan": {
              "Node Type": "Hash Join",
              "Startup Cost": 1.07,
              "Total Cost": 2.28,
              "Plan Rows": 5,
              "Actual Rows": 5,
              "Actual Total Time": 0.046,
              "Actual Loops": 1,
              "Plans": [
                {
                  "Node Type": "Seq Scan",
                  "Relation Name": "orders",
                  "Startup Cost": 0.0,
                  "Total Cost": 1.10,
                  "Plan Rows": 10,
                  "Actual Rows": 10,
                  "Actual Total Time": 0.008,
                  "Actual Loops": 1,
                  "Plans": []
                },
                {
                  "Node Type": "Hash",
                  "Startup Cost": 1.05,
                  "Total Cost": 1.05,
                  "Plan Rows": 5,
                  "Actual Rows": 5,
                  "Actual Total Time": 0.015,
                  "Actual Loops": 1,
                  "Plans": [
                    {
                      "Node Type": "Seq Scan",
                      "Relation Name": "users",
                      "Startup Cost": 0.0,
                      "Total Cost": 1.05,
                      "Plan Rows": 5,
                      "Actual Rows": 5,
                      "Actual Total Time": 0.004,
                      "Actual Loops": 1,
                      "Plans": []
                    }
                  ]
                }
              ]
            },
            "Planning Time": 0.150,
            "Execution Time": 0.075
          }
        ]"#
    }

    #[test]
    fn test_parse_explain_json() {
        let viewer =
            ExplainViewer::from_json(sample_explain_json(), Duration::from_millis(1)).unwrap();
        assert_eq!(viewer.rows.len(), 4);
        assert_eq!(viewer.rows[0].node_type, "Hash Join");
        assert_eq!(viewer.rows[1].node_type, "Seq Scan");
        assert_eq!(viewer.rows[1].relation.as_deref(), Some("orders"));
        assert_eq!(viewer.rows[2].node_type, "Hash");
        assert_eq!(viewer.rows[3].node_type, "Seq Scan");
        assert_eq!(viewer.rows[3].relation.as_deref(), Some("users"));
    }

    #[test]
    fn test_node_depths() {
        let viewer =
            ExplainViewer::from_json(sample_explain_json(), Duration::from_millis(1)).unwrap();
        assert_eq!(viewer.rows[0].depth, 0); // Hash Join
        assert_eq!(viewer.rows[1].depth, 1); // Seq Scan on orders
        assert_eq!(viewer.rows[2].depth, 1); // Hash
        assert_eq!(viewer.rows[3].depth, 2); // Seq Scan on users
    }

    #[test]
    fn test_planning_and_execution_time() {
        let viewer =
            ExplainViewer::from_json(sample_explain_json(), Duration::from_millis(1)).unwrap();
        assert!((viewer.planning_time.unwrap() - 0.15).abs() < 0.001);
        assert!((viewer.execution_time_ms.unwrap() - 0.075).abs() < 0.001);
    }

    #[test]
    fn test_max_time_tracking() {
        let viewer =
            ExplainViewer::from_json(sample_explain_json(), Duration::from_millis(1)).unwrap();
        assert!(
            (viewer.max_time - 0.046).abs() < 0.001,
            "max_time should be root node time"
        );
    }

    #[test]
    fn test_time_color_scaling() {
        let viewer =
            ExplainViewer::from_json(sample_explain_json(), Duration::from_millis(1)).unwrap();
        // Root node (0.046ms) is the max → should be red
        assert_eq!(viewer.time_color(0.046), Color::Red);
        // Leaf node (0.004ms) is ~8.7% of max → should be green
        assert_eq!(viewer.time_color(0.004), Color::Green);
    }

    #[test]
    fn test_navigation() {
        let mut viewer =
            ExplainViewer::from_json(sample_explain_json(), Duration::from_millis(1)).unwrap();
        assert_eq!(viewer.selected, 0);
        viewer.move_down();
        assert_eq!(viewer.selected, 1);
        viewer.move_down();
        assert_eq!(viewer.selected, 2);
        viewer.move_up();
        assert_eq!(viewer.selected, 1);
        viewer.go_to_bottom();
        assert_eq!(viewer.selected, 3);
        viewer.go_to_top();
        assert_eq!(viewer.selected, 0);
    }

    #[test]
    fn test_navigation_bounds() {
        let mut viewer =
            ExplainViewer::from_json(sample_explain_json(), Duration::from_millis(1)).unwrap();
        viewer.move_up(); // Already at 0
        assert_eq!(viewer.selected, 0);
        viewer.go_to_bottom();
        viewer.move_down(); // Already at max
        assert_eq!(viewer.selected, 3);
    }

    #[test]
    fn test_toggle_view_mode() {
        let mut viewer =
            ExplainViewer::from_json(sample_explain_json(), Duration::from_millis(1)).unwrap();
        assert_eq!(viewer.view_mode, ViewMode::Tree);
        viewer.selected = 2;
        viewer.toggle_view_mode();
        assert_eq!(viewer.view_mode, ViewMode::RawText);
        assert_eq!(viewer.selected, 0, "toggle resets selection");
        viewer.toggle_view_mode();
        assert_eq!(viewer.view_mode, ViewMode::Tree);
    }

    #[test]
    fn test_raw_text_contains_nodes() {
        let viewer =
            ExplainViewer::from_json(sample_explain_json(), Duration::from_millis(1)).unwrap();
        assert!(viewer.raw_text.contains("Hash Join"));
        assert!(viewer.raw_text.contains("Seq Scan on orders"));
        assert!(viewer.raw_text.contains("Seq Scan on users"));
        assert!(viewer.raw_text.contains("Planning Time:"));
        assert!(viewer.raw_text.contains("Execution Time:"));
    }

    #[test]
    fn test_invalid_json_returns_none() {
        assert!(ExplainViewer::from_json("not json", Duration::from_millis(1)).is_none());
        assert!(ExplainViewer::from_json("{}", Duration::from_millis(1)).is_none());
        assert!(ExplainViewer::from_json("[]", Duration::from_millis(1)).is_none());
        assert!(ExplainViewer::from_json("[{}]", Duration::from_millis(1)).is_none());
    }

    #[test]
    fn test_format_time() {
        assert_eq!(ExplainViewer::format_time(0.001), "1µs");
        assert_eq!(ExplainViewer::format_time(0.5), "500µs");
        assert_eq!(ExplainViewer::format_time(1.0), "1.0ms");
        assert_eq!(ExplainViewer::format_time(45.6), "45.6ms");
        assert_eq!(ExplainViewer::format_time(1500.0), "1.50s");
    }

    #[test]
    fn test_format_rows() {
        assert_eq!(ExplainViewer::format_rows(0), "0");
        assert_eq!(ExplainViewer::format_rows(999), "999");
        assert_eq!(ExplainViewer::format_rows(1500), "1.5K");
        assert_eq!(ExplainViewer::format_rows(2_500_000), "2.5M");
    }
}
