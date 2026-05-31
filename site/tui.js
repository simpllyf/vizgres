/* ============================================================
   vizgres — live TUI engine
   Faithful HTML recreation of the terminal client, driven by the
   exact palettes from src/ui/theme.rs. Reused across the site:
   hero (animated), EXPLAIN demo, themes showcase, screen gallery.
   ============================================================ */
(function () {
  "use strict";

  /* ---- palettes lifted from theme.rs ---- */
  const THEMES = {
    dark: {
      bg: "#0D1117", panel: "#0D1117",
      borderF: "#41C7D4", borderU: "#454B54", titleF: "#41C7D4", titleU: "#6E7681",
      schema: "#4D90D9", category: "#D8A93B", table: "#46B450", view: "#BC8CFF",
      column: "#9098A1", fn: "#41C7D4", index: "#5A636E",
      kw: "#4D90D9", str: "#46B450", num: "#41C7D4", text: "#E6EDF3", comment: "#5A636E",
      lineNo: "#5A636E", tilde: "#454B54", cursor: "#E6EDF3", cursorFg: "#0D1117",
      hdr: "#D8A93B", rowEven: "#E6EDF3", rowOdd: "#9098A1", selBg: "#D8A93B", selFg: "#0D1117",
      nul: "#5A636E", footer: "#5A636E", tabActiveBg: "#41C7D4", tabActiveFg: "#0D1117", tabIn: "#5A636E",
      cmd: "#BC8CFF", ok: "#46B450", info: "#4D90D9", warn: "#D8A93B", err: "#E0533D",
      dot: "#46B450", dim: "#5A636E",
      tGreen: "#46B450", tYellow: "#D8A93B", tRed: "#E0533D",
    },
    light: {
      bg: "#FFFFFF", panel: "#FFFFFF",
      borderF: "#0A56C2", borderU: "#A7AEB6", titleF: "#0A56C2", titleU: "#8A929B",
      schema: "#0000B4", category: "#8C5000", table: "#008200", view: "#960096",
      column: "#505050", fn: "#007896", index: "#8A929B",
      kw: "#0000B4", str: "#008200", num: "#007896", text: "#1E1E1E", comment: "#8A929B",
      lineNo: "#A7AEB6", tilde: "#C8CED5", cursor: "#1E1E1E", cursorFg: "#FFFFFF",
      hdr: "#0000B4", rowEven: "#1E1E1E", rowOdd: "#3C3C3C", selBg: "#0A56C2", selFg: "#FFFFFF",
      nul: "#8A929B", footer: "#8A929B", tabActiveBg: "#0A56C2", tabActiveFg: "#FFFFFF", tabIn: "#8A929B",
      cmd: "#960096", ok: "#008200", info: "#0A56C2", warn: "#C87800", err: "#B40000",
      dot: "#008200", dim: "#8A929B",
      tGreen: "#008200", tYellow: "#C87800", tRed: "#B40000",
    },
    midnight: {
      bg: "#141430", panel: "#141430",
      borderF: "#B4A0FF", borderU: "#46506E", titleF: "#B4A0FF", titleU: "#46506E",
      schema: "#7896FF", category: "#FFBE8C", table: "#82E6B4", view: "#FFA0C8",
      column: "#646E8C", fn: "#B4A0FF", index: "#46506E",
      kw: "#7896FF", str: "#82E6B4", num: "#FFBE8C", text: "#D2D7E6", comment: "#46506E",
      lineNo: "#46506E", tilde: "#46506E", cursor: "#D2D7E6", cursorFg: "#141430",
      hdr: "#B4A0FF", rowEven: "#D2D7E6", rowOdd: "#646E8C", selBg: "#B4A0FF", selFg: "#141428",
      nul: "#46506E", footer: "#46506E", tabActiveBg: "#B4A0FF", tabActiveFg: "#141428", tabIn: "#46506E",
      cmd: "#FFA0C8", ok: "#82E6B4", info: "#7896FF", warn: "#FFBE8C", err: "#FF6464",
      dot: "#82E6B4", dim: "#46506E",
      tGreen: "#82E6B4", tYellow: "#FFBE8C", tRed: "#FF6464",
    },
    ember: {
      bg: "#1E1914", panel: "#1E1914",
      borderF: "#FFB432", borderU: "#5A5046", titleF: "#FFB432", titleU: "#5A5046",
      schema: "#E68232", category: "#FFB432", table: "#8CBE78", view: "#C896B4",
      column: "#8C7864", fn: "#B4A078", index: "#5A5046",
      kw: "#E68232", str: "#8CBE78", num: "#FFB432", text: "#DCC8AA", comment: "#5A5046",
      lineNo: "#5A5046", tilde: "#5A5046", cursor: "#DCC8AA", cursorFg: "#1E1914",
      hdr: "#FFB432", rowEven: "#DCC8AA", rowOdd: "#8C7864", selBg: "#FFB432", selFg: "#1E1914",
      nul: "#5A5046", footer: "#5A5046", tabActiveBg: "#FFB432", tabActiveFg: "#1E1914", tabIn: "#5A5046",
      cmd: "#E68232", ok: "#8CBE78", info: "#E68232", warn: "#FFB432", err: "#DC503C",
      dot: "#8CBE78", dim: "#5A5046",
      tGreen: "#8CBE78", tYellow: "#FFB432", tRed: "#DC503C",
    },
  };

  /* ---- tiny DOM helpers ---- */
  function el(tag, cls, txt) {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (txt != null) e.textContent = txt;
    return e;
  }
  function span(txt, color, opts) {
    const s = el("span", null, txt);
    if (color) s.style.color = color;
    if (opts && opts.bold) s.style.fontWeight = "700";
    if (opts && opts.italic) s.style.fontStyle = "italic";
    if (opts && opts.bg) { s.style.background = opts.bg; }
    if (opts && opts.underline) s.style.textDecoration = "underline";
    return s;
  }
  function line(parent) { const l = el("div", "tline"); parent.appendChild(l); return l; }

  /* ---- panel shell with ratatui-style title chip ---- */
  function panel(title, focused, p, extraClass) {
    const wrap = el("div", "tpanel" + (focused ? " focused" : "") + (extraClass ? " " + extraClass : ""));
    wrap.style.borderColor = focused ? p.borderF : p.borderU;
    const t = el("div", "tpanel-title");
    if (focused) {
      t.appendChild(span("▸ ", p.titleF, { bold: true }));
      t.appendChild(span(title, p.titleF, { bold: true }));
    } else {
      t.appendChild(span(title, p.titleU));
    }
    t.style.background = p.bg;
    wrap.appendChild(t);
    const body = el("div", "tpanel-body");
    wrap.appendChild(body);
    return { wrap, body };
  }

  /* ---- schema tree ---- */
  const TREE = [
    { d: 0, arrow: "▼", label: "public", color: "schema", bold: true },
    { d: 1, arrow: "▼", label: "Tables (3)", color: "category", bold: true },
    { d: 2, arrow: "▶", label: "orders", color: "table" },
    { d: 2, arrow: "▶", label: "products", color: "table" },
    { d: 2, arrow: "▶", label: "users", color: "table", sel: true },
    { d: 1, arrow: "▶", label: "Views (2)", color: "category", bold: true },
    { d: 1, arrow: "▶", label: "Functions (4)", color: "category", bold: true },
    { d: 1, arrow: "▶", label: "Indexes (7)", color: "category", bold: true },
    { d: 0, arrow: "▶", label: "test_schema", color: "schema", bold: true },
  ];
  function buildTree(body, p, opts) {
    opts = opts || {};
    TREE.forEach((n) => {
      const l = line(body);
      l.appendChild(span("  ".repeat(n.d), null));
      if (n.sel && opts.selected) {
        const chip = span(" " + n.arrow + " " + n.label + " ", p.selFg, { bold: true, bg: p.borderF });
        chip.style.background = p.borderF;
        l.appendChild(chip);
      } else {
        l.appendChild(span(n.arrow + " ", p.index));
        l.appendChild(span(n.label, p[n.color], { bold: n.bold }));
      }
    });
  }

  /* ---- SQL editor with highlight ---- */
  function sqlSpans(p) {
    // SELECT * FROM users;
    return [
      span("SELECT", p.kw, { bold: true }), span(" * ", p.text),
      span("FROM", p.kw, { bold: true }), span(" users", p.text), span(";", p.text),
    ];
  }
  function buildEditor(body, p, opts) {
    opts = opts || {};
    const rows = opts.rows || 13;
    const code = line(body);
    code.classList.add("tcode");
    code.appendChild(span("1", p.lineNo));
    code.appendChild(span("  ", null));
    const codeSpan = el("span", "tcode-text");
    code.appendChild(codeSpan);
    if (opts.typed === false) {
      // empty — just a cursor block
      const cur = el("span", "tcursor");
      cur.style.background = p.cursor; cur.style.color = p.cursorFg; cur.textContent = " ";
      codeSpan.appendChild(cur);
    } else {
      sqlSpans(p).forEach((s) => codeSpan.appendChild(s));
    }
    for (let i = 1; i < rows; i++) {
      const t = line(body);
      t.appendChild(span("~", p.tilde));
    }
    return { codeSpan, p };
  }

  /* ---- results table ---- */
  const COLS = [
    { name: "id", type: "", w: 4 },
    { name: "name", type: "varchar", w: 15 },
    { name: "email", type: "varchar", w: 21 },
    { name: "active", type: "bool", w: 13 },
    { name: "metadata", type: "jsonb", w: 24 },
    { name: "created_at", type: "timestamptz", w: 25 },
    { name: "updated_at", type: "timestamp", w: 22 },
  ];
  const RESULT_ROWS = [
    ["1", "Alice Smith", "alice@example.com", "true", '{"permissions":["read","write","delet…', "2026-02-28 02:31:44 UTC", "2026-02-28 02:31:44"],
    ["2", "Bob Jones", "bob@example.com", "true", '{"permissions":["read"],"role":"user"}', "2026-02-28 02:31:44 UTC", "2026-02-28 02:31:44"],
    ["3", "Charlie Brown", "charlie@example.com", "false", '{"role":"user","suspended":true}', "2026-02-28 02:31:44 UTC", "2026-02-28 02:31:44"],
    ["4", "Diana Prince", "diana@example.com", "true", "NULL", "2026-02-28 02:31:44 UTC", "2026-02-28 02:31:44"],
  ];
  function buildResults(body, p, opts) {
    opts = opts || {};
    const grid = el("div", "tgrid");
    grid.style.gridTemplateColumns = COLS.map((c) => c.w + "ch").join(" ");
    body.appendChild(grid);

    // header
    COLS.forEach((c, i) => {
      const cell = el("div", "tcell thdr");
      cell.appendChild(span(c.name, p.hdr, { bold: true, underline: i === 0 }));
      if (c.type) cell.appendChild(span(": " + c.type, p.hdr, { bold: true }));
      grid.appendChild(cell);
    });

    const nRows = opts.visibleRows != null ? opts.visibleRows : RESULT_ROWS.length;
    RESULT_ROWS.slice(0, nRows).forEach((row, ri) => {
      row.forEach((val, ci) => {
        const cell = el("div", "tcell");
        const isNull = val === "NULL";
        const selected = opts.selected && ri === 0 && ci === 0;
        if (selected) {
          const s = span(val, p.selFg, { bold: true });
          cell.style.background = p.selBg;
          cell.appendChild(s);
        } else {
          cell.appendChild(span(val, isNull ? p.nul : (ri % 2 === 0 ? p.rowEven : p.rowOdd), { italic: isNull }));
        }
        grid.appendChild(cell);
      });
    });
    return grid;
  }
  function resultsFooter(body, p, text) {
    const f = line(body);
    f.classList.add("tfooter");
    f.appendChild(span(text, p.footer));
  }

  /* ---- EXPLAIN tree ---- */
  const PLAN = [
    { d: 0, kids: true, label: "Hash Join", t: "0.046ms", tc: "tRed", rows: "5 rows", cost: "cost 2" },
    { d: 1, kids: false, label: "Seq Scan on orders", t: "0.008ms", tc: "tGreen", rows: "10 rows", cost: "cost 1" },
    { d: 1, kids: true, label: "Hash", t: "0.015ms", tc: "tYellow", rows: "5 rows", cost: "cost 1" },
    { d: 2, kids: false, label: "Seq Scan on users", t: "0.004ms", tc: "tGreen", rows: "5 rows", cost: "cost 1" },
  ];
  function buildExplain(body, p, opts) {
    opts = opts || {};
    PLAN.forEach((n, idx) => {
      const l = line(body);
      const selected = opts.selected != null ? opts.selected === idx : idx === 0;
      const prefix = "  ".repeat(n.d) + (n.kids ? "├─ " : "── ");
      if (selected) {
        // Selected node: same focus-colored chip as the schema tree, so it
        // reads correctly in every theme (selFg on the panel's focus color).
        const txt = prefix + n.label + "  " + n.t + "  " + n.rows + "  " + n.cost;
        const chip = span(txt, p.selFg, { bold: true });
        chip.style.background = p.borderF;
        l.appendChild(chip);
      } else {
        l.appendChild(span(prefix, p.index));
        l.appendChild(span(n.label, p.text, { bold: true }));
        l.appendChild(span("  " + n.t, p[n.tc]));
        l.appendChild(span("  " + n.rows, p.column));
        l.appendChild(span("  " + n.cost, p.index));
      }
    });
    // footer
    const f = line(body);
    f.classList.add("tfooter");
    f.appendChild(span(" Total: 1.0ms ", p.footer));
    f.appendChild(span("Plan: 150µs ", p.footer));
    f.appendChild(span("Exec: 75µs ", p.footer));
    f.appendChild(span("│ 4 nodes │ t=toggle raw ", p.footer));
    f.appendChild(span("[TREE]", p.borderF, { bold: true }));
  }

  /* ---- tab bar ---- */
  function buildTabs(parent, p) {
    const tabs = el("div", "ttabs");
    const a = span(" Tab 1 ", p.tabActiveFg, { bold: true });
    a.style.background = p.tabActiveBg;
    tabs.appendChild(a);
    tabs.appendChild(span(" │ ", p.tabIn));
    tabs.appendChild(span(" Tab 2 ", p.tabIn));
    parent.appendChild(tabs);
  }

  /* ---- status bar ---- */
  function buildStatus(parent, p, opts) {
    opts = opts || {};
    const bar = el("div", "tstatus");
    const left = el("div", "tstatus-left");
    if (opts.left) {
      left.appendChild(span(opts.left, opts.leftColor || p.info));
    } else {
      left.appendChild(span("F1", p.dim)); left.appendChild(span("=help | ", p.dim));
      left.appendChild(span("Ctrl+P", p.dim)); left.appendChild(span("=commands | ", p.dim));
      left.appendChild(span("Ctrl+Enter", p.dim)); left.appendChild(span("=run | ", p.dim));
      left.appendChild(span("Ctrl+Q", p.dim)); left.appendChild(span("=quit", p.dim));
    }
    const right = el("div", "tstatus-right");
    if (opts.readonly) { const ro = span(" RO ", "#fff"); ro.style.background = p.info; right.appendChild(ro); right.appendChild(span(" ", null)); }
    right.appendChild(span("● ", p.dot));
    right.appendChild(span("[" + (opts.conn || "my-local") + "]", p.dim));
    bar.appendChild(left); bar.appendChild(right);
    parent.appendChild(bar);
  }

  /* ---- command bar (overlay style at bottom) ---- */
  function buildCommandBar(parent, p) {
    const bar = el("div", "tstatus tcmdbar");
    const left = el("div", "tstatus-left");
    left.appendChild(span("❯ ", p.cmd, { bold: true }));
    left.appendChild(span("/connect ", p.text));
    const cur = el("span", "tcursor"); cur.style.background = p.cursor; cur.style.color = p.cursorFg; cur.textContent = " ";
    left.appendChild(cur);
    bar.appendChild(left);
    parent.appendChild(bar);
    return bar;
  }

  /* ============ assemble a full TUI ============ */
  function buildTUI(opts) {
    opts = opts || {};
    const p = THEMES[opts.theme || "dark"];
    const focus = opts.focus || "results";

    const root = el("div", "tui");
    root.style.background = p.bg;
    root.dataset.theme = opts.theme || "dark";

    if (opts.window) {
      root.classList.add("has-chrome");
      const chrome = el("div", "tui-chrome");
      const dots = el("div", "tui-dots");
      ["#FF5F57", "#FEBC2E", "#28C840"].forEach((c) => { const d = el("span", "tui-dot"); d.style.background = c; dots.appendChild(d); });
      chrome.appendChild(dots);
      chrome.appendChild(el("div", "tui-chrome-title", opts.title || "vizgres — my-local"));
      chrome.appendChild(el("div", "tui-dots")); // spacer
      chrome.style.background = p.panel;
      chrome.style.borderColor = p.borderU;
      root.appendChild(chrome);
    }

    const screen = el("div", "tui-screen");
    root.appendChild(screen);

    const grid = el("div", "tui-layout");
    screen.appendChild(grid);

    // left — schema
    const tree = panel(" Schema ", focus === "tree", p, "col-tree");
    buildTree(tree.body, p, { selected: focus === "tree" });
    grid.appendChild(tree.wrap);

    // right column
    const rightCol = el("div", "tui-right");
    grid.appendChild(rightCol);
    buildTabs(rightCol, p);

    const ed = panel(" Query ", focus === "editor", p, "col-editor");
    const edRefs = buildEditor(ed.body, p, { rows: opts.editorRows || 11, typed: opts.typed });
    rightCol.appendChild(ed.wrap);

    let resRefs = null;
    if (opts.mode === "explain") {
      const ex = panel(" Explain ", focus === "results", p, "col-results");
      buildExplain(ex.body, p, {});
      rightCol.appendChild(ex.wrap);
    } else {
      const res = panel(" Results ", focus === "results", p, "col-results");
      resRefs = buildResults(res.body, p, { selected: focus === "results", visibleRows: opts.visibleRows });
      resultsFooter(res.body, p, opts.footer || "Row 1/4 │ Col 1/7 │ 1.6ms");
      rightCol.appendChild(res.wrap);
      resRefs = { panelBody: res.body, grid: resRefs };
    }

    if (opts.command) buildCommandBar(screen, p);
    else buildStatus(screen, p, { left: opts.statusLeft, leftColor: opts.statusLeftColor, conn: opts.conn, readonly: opts.readonly });

    return { root, p, editor: edRefs, results: resRefs, screen };
  }

  /* ============ hero animation ============
     Types a query, runs it, streams in results, then loops.
     Self-gating: only animates while on-screen and the tab is foregrounded,
     and respects prefers-reduced-motion (renders the settled state instead). */
  function animateHero(mount) {
    const theme = mount.dataset.theme || "dark";
    const p = THEMES[theme];

    // Each call supersedes any earlier run on this mount (e.g. theme toggle).
    const gen = (mount._gen = (mount._gen || 0) + 1);
    const alive = () => mount._gen === gen;

    // Tear down a previous run's observers/timers so they don't accumulate.
    if (mount._teardown) mount._teardown();
    mount._teardown = null;

    function freshTUI(state) {
      const built = buildTUI(Object.assign({ theme, window: true, focus: "editor" }, state));
      mount.replaceChildren(built.root);
      return built;
    }

    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      freshTUI({ typed: true, focus: "results" });
      return;
    }

    let onScreen = true;     // corrected by the IntersectionObserver below
    let parked = null;       // a step deferred while off-screen, resumed on return
    let timer = null;
    const active = () => alive() && onScreen && !document.hidden;

    function step(fn, ms) {
      clearTimeout(timer);
      timer = setTimeout(function () {
        timer = null;
        if (!alive()) return;
        if (active()) fn();
        else parked = fn;
      }, ms);
    }
    function resume() {
      if (!active() || !parked) return;
      const fn = parked; parked = null; fn();
    }

    const CHARS = "SELECT * FROM users;";

    function run() {
      const built = freshTUI({ typed: false, visibleRows: 0, footer: "— editor — type a query, Ctrl+Enter to run", statusLeft: null });
      const codeSpan = built.editor.codeSpan;
      codeSpan.replaceChildren();
      let i = 0;
      (function type() {
        if (!alive()) return;
        codeSpan.replaceChildren();
        renderTyped(codeSpan, CHARS.slice(0, i), p);
        const cur = el("span", "tcursor blink");
        cur.style.background = p.cursor; cur.style.color = p.cursorFg; cur.textContent = " ";
        codeSpan.appendChild(cur);
        i++;
        if (i <= CHARS.length) step(type, 55 + Math.random() * 50);
        else step(execute, 520);
      })();
    }

    function execute() {
      freshTUI({ typed: true, visibleRows: 0, focus: "results",
        statusLeft: "Executing… (0.1s) — Esc to cancel", statusLeftColor: p.info,
        footer: "running…" });
      let n = 0;
      const total = 4;
      function stream() {
        if (!alive()) return;
        n++;
        freshTUI({ typed: true, visibleRows: n, focus: "results",
          statusLeft: "Streaming… " + n + " rows (0." + n + "s) — Esc to cancel", statusLeftColor: p.info,
          footer: "Row " + n + "/4 │ Col 1/7 │ 1.6ms" });
        if (n < total) step(stream, 220);
        else step(settle, 260);
      }
      step(stream, 420);
    }

    function settle() {
      freshTUI({ typed: true, visibleRows: 4, focus: "results",
        statusLeft: "4 rows in 1.6 ms", statusLeftColor: p.ok,
        footer: "Row 1/4 │ Col 1/7 │ 1.6ms" });
      step(run, 4200); // loop
    }

    const io = new IntersectionObserver(function (entries) {
      onScreen = entries[0].isIntersecting;
      if (onScreen) resume();
    }, { threshold: 0.2 });
    io.observe(mount);
    const onVis = function () { resume(); };
    document.addEventListener("visibilitychange", onVis);

    mount._teardown = function () {
      clearTimeout(timer);
      io.disconnect();
      document.removeEventListener("visibilitychange", onVis);
    };

    if (active()) run();
    else parked = run;
  }

  function renderTyped(container, sql, p) {
    // tokenize lightly: SELECT, FROM keywords blue-bold; rest text
    const parts = sql.split(/(\bSELECT\b|\bFROM\b)/);
    parts.forEach((part) => {
      if (part === "SELECT" || part === "FROM") container.appendChild(span(part, p.kw, { bold: true }));
      else if (part) container.appendChild(span(part, p.text));
    });
  }

  /* ---- expose ---- */
  window.VizgresTUI = { THEMES, buildTUI, animateHero };
})();
