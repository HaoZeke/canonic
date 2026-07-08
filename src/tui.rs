//! Interactive terminal UI for browsing and gating the canned-response corpus.
//!
//! Launch with `canonic tui`. Pure state helpers are unit-tested with
//! [`ratatui::backend::TestBackend`]; the live loop uses crossterm.

use crate::check::{check_responses, format_check_report, CheckReport};
use crate::convert::{convert_path_to_jira, tool_available as pandoc_available};
use crate::corpus::{load_response, walk_responses, CannedResponse};
use crate::doctor::{collect_statuses, format_doctor};
use crate::index::{default_index_dir, reindex, search};
use crate::lint::{format_report, lint_paths, LintEngine};
use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use std::io::{self, Stdout};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Teal-ish accent matching the Shibuya / canonic brand.
const ACCENT: Color = Color::Rgb(13, 148, 136); // #0D9488
const MUTED: Color = Color::DarkGray;
const WARN: Color = Color::Yellow;
const OK: Color = Color::Green;
const BAD: Color = Color::Red;

/// Which pane / modal is focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    List,
    Filter,
    Help,
}

/// One row in the browser list (filtered view over the full corpus).
#[derive(Debug, Clone)]
pub struct ListEntry {
    pub id: String,
    pub title: String,
    pub path: PathBuf,
    pub check_ok: bool,
    pub finding_count: usize,
}

/// Application state for the corpus browser (testable without a real terminal).
#[derive(Debug)]
pub struct App {
    pub corpus: PathBuf,
    pub responses: Vec<CannedResponse>,
    pub entries: Vec<ListEntry>,
    pub list_state: ListState,
    pub filter: String,
    pub focus: Focus,
    pub status: String,
    pub preview: String,
    pub side_panel: String,
    pub should_quit: bool,
    pub scroll: u16,
}

impl App {
    /// Load corpus from disk and build initial list + selection.
    pub fn load(corpus: PathBuf) -> Result<Self> {
        let responses = if corpus.exists() {
            walk_responses(&corpus)?
        } else {
            Vec::new()
        };
        let mut app = Self {
            corpus,
            responses,
            entries: Vec::new(),
            list_state: ListState::default(),
            filter: String::new(),
            focus: Focus::List,
            status: String::new(),
            preview: String::new(),
            side_panel: String::new(),
            should_quit: false,
            scroll: 0,
        };
        app.rebuild_entries();
        app.refresh_side_panel();
        app.status = format!(
            "{} response(s) in {} · ↑↓ move · / filter · c convert · r reindex · ? help · q quit",
            app.entries.len(),
            app.corpus.display()
        );
        Ok(app)
    }

    /// Apply filter string against id/title/content and refresh selection.
    pub fn rebuild_entries(&mut self) {
        let q = self.filter.to_lowercase();
        let prev_id = self
            .list_state
            .selected()
            .and_then(|i| self.entries.get(i).map(|e| e.id.clone()));

        self.entries = self
            .responses
            .iter()
            .filter(|r| {
                if q.is_empty() {
                    return true;
                }
                r.id.to_lowercase().contains(&q)
                    || r.title.to_lowercase().contains(&q)
                    || r.content.to_lowercase().contains(&q)
                    || r.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .map(|r| {
                let report = check_responses(std::slice::from_ref(r));
                ListEntry {
                    id: r.id.clone(),
                    title: r.title.clone(),
                    path: r.path.clone(),
                    check_ok: report.ok(),
                    finding_count: report.findings.len(),
                }
            })
            .collect();

        // Restore selection if possible.
        let idx = prev_id
            .and_then(|id| self.entries.iter().position(|e| e.id == id))
            .or_else(|| if self.entries.is_empty() { None } else { Some(0) });
        self.list_state.select(idx);
        self.update_preview();
    }

    pub fn selected_entry(&self) -> Option<&ListEntry> {
        self.list_state.selected().and_then(|i| self.entries.get(i))
    }

    pub fn selected_response(&self) -> Option<&CannedResponse> {
        let id = self.selected_entry()?.id.as_str();
        self.responses.iter().find(|r| r.id == id)
    }

    pub fn select_next(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => (i + 1).min(self.entries.len() - 1),
            None => 0,
        };
        self.list_state.select(Some(i));
        self.scroll = 0;
        self.update_preview();
    }

    pub fn select_prev(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(0) | None => 0,
            Some(i) => i - 1,
        };
        self.list_state.select(Some(i));
        self.scroll = 0;
        self.update_preview();
    }

    pub fn update_preview(&mut self) {
        match self.selected_response() {
            Some(r) => {
                let report = check_responses(std::slice::from_ref(r));
                let mut preview = r.body.clone();
                if preview.len() > 12_000 {
                    preview.truncate(12_000);
                    preview.push_str("\n… (truncated)");
                }
                let gate = if report.ok() {
                    "check: OK".to_string()
                } else {
                    format!(
                        "check: {} finding(s)\n{}",
                        report.findings.len(),
                        format_check_report(&report)
                    )
                };
                self.preview = format!(
                    "id: {}\ntitle: {}\npath: {}\nsop: {}\ntags: {:?}\n\n── quality ──\n{}\n\n── markdown ──\n{}",
                    r.id,
                    r.title,
                    r.path.display(),
                    r.sop.as_deref().unwrap_or("-"),
                    r.tags,
                    gate,
                    preview
                );
            }
            None => {
                self.preview = if self.responses.is_empty() {
                    format!(
                        "No responses in {}.\n\nScaffold one:\n  canonic new \"Topic title\"\n\nOr import drafts:\n  canonic import-jira \"project = HSP AND labels = canned-response\"\n  canonic promote corpus/imports/…md",
                        self.corpus.display()
                    )
                } else {
                    "No rows match the current filter.".into()
                };
            }
        }
    }

    pub fn refresh_side_panel(&mut self) {
        let doctor = format_doctor(&collect_statuses());
        let clean = self.entries.iter().filter(|e| e.check_ok).count();
        let dirty = self.entries.len().saturating_sub(clean);
        self.side_panel = format!(
            "corpus: {}\nentries: {} (filter {:?})\nclean: {} · findings: {}\n\n── doctor ──\n{}",
            self.corpus.display(),
            self.entries.len(),
            self.filter,
            clean,
            dirty,
            doctor.trim()
        );
    }

    /// Run quality check over the full (unfiltered) corpus and show the report.
    pub fn run_check_all(&mut self) -> Result<()> {
        let report: CheckReport = check_responses(&self.responses);
        self.status = report.summary_line();
        self.preview = format_check_report(&report);
        self.rebuild_entries();
        Ok(())
    }

    /// Lint the selected path (or whole corpus) with harper-core.
    pub fn run_lint_selected(&mut self) -> Result<()> {
        let paths: Vec<PathBuf> = if let Some(e) = self.selected_entry() {
            vec![e.path.clone()]
        } else {
            self.responses.iter().map(|r| r.path.clone()).collect()
        };
        if paths.is_empty() {
            self.status = "lint: nothing to lint".into();
            return Ok(());
        }
        let report = lint_paths(&paths, LintEngine::Harper)?;
        self.status = report.summary_line();
        self.preview = format_report(&report);
        Ok(())
    }

    /// Convert the selected response with pandoc jira writer into the preview pane.
    pub fn run_convert_selected(&mut self) -> Result<()> {
        let Some(entry) = self.selected_entry().cloned() else {
            self.status = "convert: no selection".into();
            return Ok(());
        };
        if !pandoc_available() {
            self.status = "convert: pandoc missing (install pandoc)".into();
            self.preview = "pandoc is not on PATH. Install pandoc to convert markdown → jira wiki.".into();
            return Ok(());
        }
        let jira = convert_path_to_jira(&entry.path)
            .with_context(|| format!("convert {}", entry.path.display()))?;
        self.status = format!(
            "convert: {} → {} bytes jira wiki (not posted)",
            entry.id,
            jira.len()
        );
        self.preview = format!(
            "── jira wiki (pandoc) · {} ──\n\n{}",
            entry.id, jira
        );
        Ok(())
    }

    /// Rebuild the Tantivy index from the published corpus.
    pub fn run_reindex(&mut self) -> Result<()> {
        let index = default_index_dir();
        let n = reindex(&self.corpus, &index)?;
        self.status = format!("reindexed {n} document(s) → {}", index.display());
        Ok(())
    }

    /// Search the local index with the current filter (or a fixed query).
    pub fn run_search(&mut self, query: &str) -> Result<()> {
        let index = default_index_dir();
        if !index.exists() {
            let n = reindex(&self.corpus, &index)?;
            self.status = format!("built index ({n} docs); searching…");
        }
        let hits = search(&index, query, 15)?;
        if hits.is_empty() {
            self.preview = format!("(no hits for {query:?})");
            self.status = format!("search: 0 hits for {query:?}");
        } else {
            let mut out = format!("── search: {query:?} ({} hit(s)) ──\n\n", hits.len());
            for (i, h) in hits.iter().enumerate() {
                out.push_str(&format!(
                    "{}. {}  score={:.3}\n   {}\n   {}\n\n",
                    i + 1,
                    h.id,
                    h.score,
                    h.title,
                    h.snippet
                ));
            }
            self.preview = out;
            self.status = format!("search: {} hit(s) for {query:?}", hits.len());
        }
        Ok(())
    }

    /// Reload markdown files from disk (after external edits).
    pub fn reload_corpus(&mut self) -> Result<()> {
        self.responses = if self.corpus.exists() {
            walk_responses(&self.corpus)?
        } else {
            Vec::new()
        };
        self.rebuild_entries();
        self.refresh_side_panel();
        self.status = format!("reloaded {} response(s)", self.responses.len());
        Ok(())
    }

    /// Handle a key press. Returns Ok after mutating state.
    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        if self.focus == Focus::Help {
            if matches!(
                code,
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::Enter
            ) {
                self.focus = Focus::List;
            }
            return Ok(());
        }

        if self.focus == Focus::Filter {
            match code {
                KeyCode::Esc => {
                    self.filter.clear();
                    self.focus = Focus::List;
                    self.rebuild_entries();
                    self.status = "filter cleared".into();
                }
                KeyCode::Enter => {
                    self.focus = Focus::List;
                    self.rebuild_entries();
                    self.status = format!("filter: {:?}", self.filter);
                }
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.rebuild_entries();
                }
                KeyCode::Char(c) if !modifiers.contains(KeyModifiers::CONTROL) => {
                    self.filter.push(c);
                    self.rebuild_entries();
                }
                _ => {}
            }
            return Ok(());
        }

        // List focus
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Down | KeyCode::Char('j') => self.select_next(),
            KeyCode::Up | KeyCode::Char('k') => self.select_prev(),
            KeyCode::Char('/') => {
                self.focus = Focus::Filter;
                self.status = "filter: type to match id/title/body · Enter apply · Esc clear".into();
            }
            KeyCode::Char('?') => {
                self.focus = Focus::Help;
            }
            KeyCode::Char('g') => {
                if !self.entries.is_empty() {
                    self.list_state.select(Some(0));
                    self.scroll = 0;
                    self.update_preview();
                }
            }
            KeyCode::Char('G') => {
                if !self.entries.is_empty() {
                    self.list_state.select(Some(self.entries.len() - 1));
                    self.scroll = 0;
                    self.update_preview();
                }
            }
            KeyCode::PageDown | KeyCode::Char('J') => {
                self.scroll = self.scroll.saturating_add(8);
            }
            KeyCode::PageUp | KeyCode::Char('K') => {
                self.scroll = self.scroll.saturating_sub(8);
            }
            KeyCode::Char('R') => {
                self.reload_corpus()?;
            }
            KeyCode::Char('r') => {
                if let Err(e) = self.run_reindex() {
                    self.status = format!("reindex failed: {e:#}");
                }
            }
            KeyCode::Char('c') => {
                if let Err(e) = self.run_convert_selected() {
                    self.status = format!("convert failed: {e:#}");
                }
            }
            KeyCode::Char('C') => {
                if let Err(e) = self.run_check_all() {
                    self.status = format!("check failed: {e:#}");
                }
            }
            KeyCode::Char('l') => {
                if let Err(e) = self.run_lint_selected() {
                    self.status = format!("lint failed: {e:#}");
                }
            }
            KeyCode::Char('s') => {
                let q = if self.filter.is_empty() {
                    self.selected_entry()
                        .map(|e| e.title.clone())
                        .unwrap_or_else(|| "resp".into())
                } else {
                    self.filter.clone()
                };
                if let Err(e) = self.run_search(&q) {
                    self.status = format!("search failed: {e:#}");
                }
            }
            KeyCode::Char('d') => {
                self.refresh_side_panel();
                self.preview = self.side_panel.clone();
                self.status = "doctor snapshot in preview".into();
            }
            KeyCode::Enter => {
                self.update_preview();
                self.status = self
                    .selected_entry()
                    .map(|e| format!("selected {}", e.id))
                    .unwrap_or_else(|| "no selection".into());
            }
            _ => {}
        }
        Ok(())
    }
}

fn help_text() -> String {
    r#"canonic TUI — corpus browser

Navigation
  j / ↓          next response
  k / ↑          previous response
  g / G          first / last
  PgDn/PgUp      scroll preview
  /              filter by id, title, tags, body
  Enter          refresh preview
  ?              this help
  q / Esc        quit

Actions (operate on selection or corpus)
  C              run quality check on whole corpus
  c              convert selection → jira wiki (pandoc; preview only)
  l              lint selection with harper-core
  r              rebuild Tantivy index
  s              search index (uses filter text or selected title)
  R              reload corpus from disk
  d              show doctor / tooling snapshot

Constraints
  • Markdown under corpus/responses/ is source of truth
  • TUI never POSTs to Jira (use canonic jira-comment explicitly)
  • Promote imports with: canonic promote PATH.md
"#
    .to_string()
}

fn draw(frame: &mut Frame, app: &mut App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(frame.area());

    // Header
    let title = Line::from(vec![
        Span::styled(
            " canonic ",
            Style::default()
                .fg(Color::Black)
                .bg(ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled("corpus browser", Style::default().fg(ACCENT)),
        Span::raw("  "),
        Span::styled(
            format!(
                "{} shown · {} total",
                app.entries.len(),
                app.responses.len()
            ),
            Style::default().fg(MUTED),
        ),
    ]);
    let header = Paragraph::new(title).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(ACCENT))
            .title(" Jira canned responses "),
    );
    frame.render_widget(header, root[0]);

    // Body: list | preview
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(34), Constraint::Percentage(66)])
        .split(root[1]);

    let items: Vec<ListItem> = app
        .entries
        .iter()
        .map(|e| {
            let mark = if e.check_ok { "✓" } else { "!" };
            let style = if e.check_ok {
                Style::default().fg(OK)
            } else {
                Style::default().fg(BAD)
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {mark} "), style),
                Span::styled(e.id.clone(), Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(format!("  {}", e.title), Style::default().fg(MUTED)),
            ]))
        })
        .collect();

    let list_title = if app.focus == Focus::Filter {
        format!(" filter: {}_ ", app.filter)
    } else if app.filter.is_empty() {
        " responses ".into()
    } else {
        format!(" responses · /{} ", app.filter)
    };
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(list_title)
                .border_style(if app.focus == Focus::Filter {
                    Style::default().fg(WARN)
                } else {
                    Style::default().fg(ACCENT)
                }),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(19, 78, 74))
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("› ");
    frame.render_stateful_widget(list, body[0], &mut app.list_state);

    let preview = Paragraph::new(app.preview.as_str())
        .wrap(Wrap { trim: false })
        .scroll((app.scroll, 0))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" preview ")
                .border_style(Style::default().fg(ACCENT)),
        );
    frame.render_widget(preview, body[1]);

    // Status
    let status = Paragraph::new(app.status.as_str())
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" status · C check · c convert · l lint · r reindex · s search · / filter · ? help · q quit ")
                .border_style(Style::default().fg(MUTED)),
        );
    frame.render_widget(status, root[2]);

    if app.focus == Focus::Help {
        let area = centered_rect(72, 80, frame.area());
        frame.render_widget(Clear, area);
        let help = Paragraph::new(help_text())
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" help · Esc/q/? to close ")
                    .border_style(Style::default().fg(ACCENT)),
            );
        frame.render_widget(help, area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Run the interactive TUI until the user quits.
pub fn run_tui(corpus: PathBuf) -> Result<()> {
    let mut app = App::load(corpus)?;
    let mut terminal = setup_terminal()?;
    let result = run_loop(&mut terminal, &mut app);
    restore_terminal(&mut terminal)?;
    result
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode().context("enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend).context("create terminal")
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode().context("disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .context("leave alternate screen")?;
    terminal.show_cursor().context("show cursor")?;
    Ok(())
}

fn run_loop<B>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()>
where
    B: Backend,
    B::Error: Send + Sync + 'static,
{
    loop {
        terminal
            .draw(|f| draw(f, app))
            .map_err(|e| anyhow::anyhow!("draw: {e}"))?;
        if app.should_quit {
            break;
        }
        if event::poll(Duration::from_millis(200)).context("poll events")? {
            if let Event::Key(key) = event::read().context("read event")? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key.code, key.modifiers)?;
                }
            }
        }
    }
    Ok(())
}

/// Render one frame to a test backend (for structural UI tests).
pub fn render_test_frame(app: &mut App, width: u16, height: u16) -> Result<String> {
    use ratatui::backend::TestBackend;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).context("test terminal")?;
    terminal.draw(|f| draw(f, app))?;
    let buffer = terminal.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..height {
        for x in 0..width {
            let cell = &buffer[(x, y)];
            out.push_str(cell.symbol());
        }
        out.push('\n');
    }
    Ok(out)
}

/// Open path as a single-file corpus view (load parent dir, select that id).
pub fn load_focusing(corpus: PathBuf, focus_path: Option<&Path>) -> Result<App> {
    let mut app = App::load(corpus)?;
    if let Some(path) = focus_path {
        if path.is_file() {
            let doc = load_response(path)?;
            if let Some(i) = app.entries.iter().position(|e| e.id == doc.id) {
                app.list_state.select(Some(i));
                app.update_preview();
            }
        }
    }
    Ok(app)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write_ok(dir: &Path, id: &str, title: &str, body: &str) {
        fs::write(
            dir.join(format!("{id}.md")),
            format!(
                "---\nid: {id}\ntitle: {title}\nprefix: resp\nsop: none\n---\n\n# {title}\n\n{body}\n\nRegards,\nSupport Team\n"
            ),
        )
        .unwrap();
    }

    #[test]
    fn load_lists_check_clean_entries() {
        let dir = tempdir().unwrap();
        write_ok(
            dir.path(),
            "resp-alpha",
            "Alpha",
            "Alpha body about project space.",
        );
        write_ok(dir.path(), "resp-beta", "Beta", "Beta body about modules.");
        let app = App::load(dir.path().to_path_buf()).unwrap();
        assert_eq!(app.entries.len(), 2);
        assert!(app.entries.iter().all(|e| e.check_ok));
        assert!(app.list_state.selected().is_some());
        assert!(app.preview.contains("resp-") || app.preview.contains("Alpha") || app.preview.contains("Beta"));
    }

    #[test]
    fn filter_narrows_list() {
        let dir = tempdir().unwrap();
        write_ok(dir.path(), "resp-storage", "Storage topic", "disk quota");
        write_ok(dir.path(), "resp-queue", "Queue topic", "partition limits");
        let mut app = App::load(dir.path().to_path_buf()).unwrap();
        app.filter = "queue".into();
        app.rebuild_entries();
        assert_eq!(app.entries.len(), 1);
        assert_eq!(app.entries[0].id, "resp-queue");
    }

    #[test]
    fn handle_key_quit_and_nav() {
        let dir = tempdir().unwrap();
        write_ok(dir.path(), "resp-a", "A", "one");
        write_ok(dir.path(), "resp-b", "B", "two");
        let mut app = App::load(dir.path().to_path_buf()).unwrap();
        app.handle_key(KeyCode::Down, KeyModifiers::NONE).unwrap();
        assert_eq!(app.list_state.selected(), Some(1));
        app.handle_key(KeyCode::Up, KeyModifiers::NONE).unwrap();
        assert_eq!(app.list_state.selected(), Some(0));
        app.handle_key(KeyCode::Char('q'), KeyModifiers::NONE)
            .unwrap();
        assert!(app.should_quit);
    }

    #[test]
    fn test_backend_renders_branding() {
        let dir = tempdir().unwrap();
        write_ok(
            dir.path(),
            "resp-project-space",
            "Project space",
            "Not a backup.",
        );
        let mut app = App::load(dir.path().to_path_buf()).unwrap();
        let frame = render_test_frame(&mut app, 100, 30).unwrap();
        assert!(
            frame.contains("canonic") || frame.contains("corpus"),
            "frame missing brand: {frame}"
        );
        assert!(
            frame.contains("resp-project-space") || frame.contains("Project"),
            "frame missing entry: {frame}"
        );
    }

    #[test]
    fn empty_corpus_has_guidance() {
        let dir = tempdir().unwrap();
        let app = App::load(dir.path().to_path_buf()).unwrap();
        assert!(app.entries.is_empty());
        assert!(
            app.preview.contains("canonic new") || app.preview.contains("No responses"),
            "{}",
            app.preview
        );
    }
}
