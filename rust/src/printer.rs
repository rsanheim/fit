use crossterm::cursor::{MoveToColumn, MoveUp};
use crossterm::queue;
use crossterm::terminal::{Clear, ClearType};
use std::io::{self, Write};
use std::time::Instant;

fn display_repo_name(name: &str, width: usize) -> String {
    if name.len() > width {
        if width <= 4 {
            name.chars().take(width).collect()
        } else {
            format!("{}-...", &name[..width - 4])
        }
    } else {
        name.to_string()
    }
}

pub(crate) fn format_repo_name(name: &str, width: usize) -> String {
    let display_name = display_repo_name(name, width);
    format!("[{:<width$}]", display_name, width = width)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowState {
    Running,
    Finished,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoRow {
    pub index: usize,
    pub repo: String,
    pub output: String,
    pub state: RowState,
}

impl RepoRow {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(index: usize, repo: String, output: String) -> Self {
        Self {
            index,
            repo,
            output,
            state: RowState::Finished,
        }
    }

    pub fn running(index: usize, repo: String) -> Self {
        Self {
            index,
            repo,
            output: "running".to_string(),
            state: RowState::Running,
        }
    }

    pub fn finished(index: usize, repo: String, output: String) -> Self {
        Self {
            index,
            repo,
            output,
            state: RowState::Finished,
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub struct Viewport {
    pub start: usize,
    pub end: usize,
}

#[cfg_attr(not(test), allow(dead_code))]
impl Viewport {
    pub fn for_rows(rows: &[RepoRow], height: usize) -> Self {
        let height = height.max(1);
        let anchor = rows
            .iter()
            .position(|row| row.state == RowState::Running)
            .unwrap_or(rows.len().saturating_sub(height));
        let start = anchor.min(rows.len().saturating_sub(height));
        let end = (start + height).min(rows.len());
        Self { start, end }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub struct FooterState {
    pub visible_start: usize,
    pub visible_end: usize,
    pub total_rows: usize,
    pub complete: usize,
    pub running: usize,
    pub elapsed_ms: u128,
}

#[cfg_attr(not(test), allow(dead_code))]
impl FooterState {
    pub fn render(&self) -> String {
        format!(
            "showing {}-{} of {} | {} complete | {} running | {:.1}s",
            self.visible_start,
            self.visible_end,
            self.total_rows,
            self.complete,
            self.running,
            self.elapsed_ms as f64 / 1000.0,
        )
    }
}

pub trait Printer {
    fn start(&mut self, rows: &[RepoRow]) -> io::Result<()>;
    fn finish_row(&mut self, row_index: usize, row: &RepoRow) -> io::Result<()>;
    fn complete(&mut self, rows: &[RepoRow], elapsed_ms: u128) -> io::Result<()>;
}

pub struct PlainPrinter<W: Write> {
    writer: W,
    repo_width: usize,
}

impl<W: Write> PlainPrinter<W> {
    pub fn new(writer: W, repo_width: usize) -> Self {
        Self { writer, repo_width }
    }
}

impl<W: Write> Printer for PlainPrinter<W> {
    fn start(&mut self, _rows: &[RepoRow]) -> io::Result<()> {
        Ok(())
    }

    fn finish_row(&mut self, _row_index: usize, row: &RepoRow) -> io::Result<()> {
        writeln!(self.writer, "{} {}", format_repo_name(&row.repo, self.repo_width), row.output)
    }

    fn complete(&mut self, _rows: &[RepoRow], _elapsed_ms: u128) -> io::Result<()> {
        Ok(())
    }
}

pub struct TtyTablePrinter<W: Write> {
    writer: W,
    terminal_rows: usize,
    repo_width: usize,
    rows: Vec<RepoRow>,
    rendered_line_count: usize,
    started_at: Option<Instant>,
    elapsed_override_ms: Option<u128>,
}

impl<W: Write> TtyTablePrinter<W> {
    pub fn new(writer: W, terminal_rows: usize, repo_width: usize) -> Self {
        Self {
            writer,
            terminal_rows,
            repo_width,
            rows: Vec::new(),
            rendered_line_count: 0,
            started_at: None,
            elapsed_override_ms: None,
        }
    }

    fn visible_height(&self) -> usize {
        self.terminal_rows.saturating_sub(1).max(1)
    }

    fn viewport(&self) -> Viewport {
        Viewport::for_rows(&self.rows, self.visible_height())
    }

    fn elapsed_ms(&self) -> u128 {
        self.elapsed_override_ms.unwrap_or_else(|| {
            self.started_at
                .map(|started_at| started_at.elapsed().as_millis())
                .unwrap_or(0)
        })
    }

    fn footer(&self) -> FooterState {
        let viewport = self.viewport();
        let complete = self
            .rows
            .iter()
            .filter(|row| row.state == RowState::Finished)
            .count();
        let running = self.rows.len().saturating_sub(complete);

        FooterState {
            visible_start: if self.rows.is_empty() {
                0
            } else {
                viewport.start + 1
            },
            visible_end: viewport.end,
            total_rows: self.rows.len(),
            complete,
            running,
            elapsed_ms: self.elapsed_ms(),
        }
    }

    fn render_row(&self, row: &RepoRow) -> String {
        format!(
            "{:<width$}  {}",
            display_repo_name(&row.repo, self.repo_width),
            row.output,
            width = self.repo_width
        )
    }

    fn render_frame(&mut self) -> io::Result<()> {
        if self.rendered_line_count > 0 {
            queue!(
                self.writer,
                MoveToColumn(0),
                MoveUp(self.rendered_line_count as u16)
            )?;
        }

        let viewport = self.viewport();
        for row in &self.rows[viewport.start..viewport.end] {
            queue!(self.writer, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
            writeln!(self.writer, "{}", self.render_row(row))?;
        }

        queue!(self.writer, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
        writeln!(self.writer, "{}", self.footer().render())?;
        self.writer.flush()?;

        self.rendered_line_count = viewport.end.saturating_sub(viewport.start) + 1;
        Ok(())
    }
}

impl<W: Write> Printer for TtyTablePrinter<W> {
    fn start(&mut self, rows: &[RepoRow]) -> io::Result<()> {
        self.rows = rows.to_vec();
        self.started_at = Some(Instant::now());
        self.elapsed_override_ms = None;
        self.render_frame()
    }

    fn finish_row(&mut self, row_index: usize, row: &RepoRow) -> io::Result<()> {
        if row_index < self.rows.len() {
            self.rows[row_index] = row.clone();
        }
        self.elapsed_override_ms = None;
        self.render_frame()
    }

    fn complete(&mut self, rows: &[RepoRow], elapsed_ms: u128) -> io::Result<()> {
        self.rows = rows.to_vec();
        self.elapsed_override_ms = Some(elapsed_ms);
        self.render_frame()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_printer_formats_repo_and_output_without_ansi() {
        let mut output = Vec::new();
        let rows = vec![RepoRow::new(0, "agentic-dev".to_string(), "running".to_string())];

        {
            let mut printer = PlainPrinter::new(&mut output, 12);
            printer.start(&rows).expect("plain printer start");
            printer.finish_row(0, &rows[0]).expect("plain printer finish");
        }

        let rendered = String::from_utf8(output).expect("utf8");
        assert_eq!(rendered, "[agentic-dev ] running\n");
        assert!(!rendered.contains('\u{1b}'));
    }

    #[test]
    fn plain_printer_truncates_long_repo_names() {
        let mut output = Vec::new();
        let rows = vec![RepoRow::new(
            0,
            "this-is-a-very-long-repository-name".to_string(),
            "clean".to_string(),
        )];

        {
            let mut printer = PlainPrinter::new(&mut output, 24);
            printer.start(&rows).expect("plain printer start");
            printer.finish_row(0, &rows[0]).expect("plain printer finish");
        }

        let rendered = String::from_utf8(output).expect("utf8");
        assert_eq!(rendered, "[this-is-a-very-long--...] clean\n");
    }

    #[test]
    fn viewport_follows_first_unfinished_repo() {
        let rows = vec![
            RepoRow::finished(0, "activities".to_string(), "clean".to_string()),
            RepoRow::finished(1, "agentic-dev".to_string(), "clean".to_string()),
            RepoRow::running(2, "amion-api".to_string()),
            RepoRow::running(3, "api-gateway".to_string()),
            RepoRow::running(4, "billing".to_string()),
        ];

        let viewport = Viewport::for_rows(&rows, 3);

        assert_eq!(viewport.start, 2);
        assert_eq!(viewport.end, 5);
    }

    #[test]
    fn footer_includes_slice_counts_and_elapsed_time() {
        let footer = FooterState {
            visible_start: 24,
            visible_end: 47,
            total_rows: 98,
            complete: 41,
            running: 8,
            elapsed_ms: 2100,
        };

        assert_eq!(
            footer.render(),
            "showing 24-47 of 98 | 41 complete | 8 running | 2.1s"
        );
    }

    #[test]
    fn tty_table_printer_renders_running_rows_without_headers() {
        let rows = vec![
            RepoRow::running(0, "activities".to_string()),
            RepoRow::running(1, "agentic-dev".to_string()),
        ];
        let mut output = Vec::new();

        {
            let mut printer = TtyTablePrinter::new(&mut output, 6, 14);
            printer.start(&rows).expect("tty start");
        }

        let rendered = String::from_utf8(output).expect("utf8");
        assert!(rendered.contains("activities"));
        assert!(rendered.contains("running"));
        assert!(!rendered.contains("REPO"));
        assert!(!rendered.contains("OUTPUT"));
    }

    #[test]
    fn tty_table_printer_keeps_completed_rows_in_place() {
        let mut rows = vec![
            RepoRow::running(0, "activities".to_string()),
            RepoRow::running(1, "agentic-dev".to_string()),
        ];
        let mut output = Vec::new();

        {
            let mut printer = TtyTablePrinter::new(&mut output, 6, 14);
            printer.start(&rows).expect("tty start");
            rows[0] = RepoRow::finished(0, "activities".to_string(), "clean".to_string());
            printer.finish_row(0, &rows[0]).expect("tty finish");
        }

        let rendered = String::from_utf8(output).expect("utf8");
        assert!(rendered.contains("activities"));
        assert!(rendered.contains("clean"));
        assert!(rendered.contains("agentic-dev"));
    }
}
