use crossterm::cursor::{MoveToColumn, MoveUp};
use crossterm::queue;
use crossterm::terminal::{Clear, ClearType};
use std::io::{self, Write};

fn display_repo_name(name: &str, width: usize) -> String {
    if name.len() <= width {
        return name.to_string();
    }
    if width <= 4 {
        return name.chars().take(width).collect();
    }
    let end = name.floor_char_boundary(width - 4);
    format!("{}-...", &name[..end])
}

pub(crate) fn format_repo_name(name: &str, width: usize) -> String {
    format!(
        "[{:<width$}]",
        display_repo_name(name, width),
        width = width
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowState {
    Running,
    Finished,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoRow {
    pub repo: String,
    pub output: String,
    pub state: RowState,
}

impl RepoRow {
    pub fn running(repo: String) -> Self {
        Self {
            repo,
            output: "running".to_string(),
            state: RowState::Running,
        }
    }

    pub fn finished(repo: String, output: String) -> Self {
        Self {
            repo,
            output,
            state: RowState::Finished,
        }
    }
}

pub struct Viewport {
    pub start: usize,
    pub end: usize,
}

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

pub struct FooterState {
    pub visible_start: usize,
    pub visible_end: usize,
    pub total_rows: usize,
    pub complete: usize,
    pub running: usize,
    pub elapsed_ms: u128,
}

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
    fn finish_row(
        &mut self,
        rows: &[RepoRow],
        row_index: usize,
        elapsed_ms: u128,
    ) -> io::Result<()>;
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

    fn finish_row(
        &mut self,
        rows: &[RepoRow],
        row_index: usize,
        _elapsed_ms: u128,
    ) -> io::Result<()> {
        let row = &rows[row_index];
        writeln!(
            self.writer,
            "{} {}",
            format_repo_name(&row.repo, self.repo_width),
            row.output
        )
    }

    fn complete(&mut self, _rows: &[RepoRow], _elapsed_ms: u128) -> io::Result<()> {
        Ok(())
    }
}

pub struct TtyTablePrinter<W: Write> {
    writer: W,
    terminal_rows: usize,
    repo_width: usize,
    rendered_line_count: usize,
}

impl<W: Write> TtyTablePrinter<W> {
    pub fn new(writer: W, terminal_rows: usize, repo_width: usize) -> Self {
        Self {
            writer,
            terminal_rows,
            repo_width,
            rendered_line_count: 0,
        }
    }

    fn visible_height(&self) -> usize {
        self.terminal_rows.saturating_sub(1).max(1)
    }

    fn render_row(&self, row: &RepoRow) -> String {
        format!(
            "{:<width$}  {}",
            display_repo_name(&row.repo, self.repo_width),
            row.output,
            width = self.repo_width
        )
    }

    fn render_frame(&mut self, rows: &[RepoRow], elapsed_ms: u128) -> io::Result<()> {
        if self.rendered_line_count > 0 {
            queue!(
                self.writer,
                MoveToColumn(0),
                MoveUp(self.rendered_line_count as u16)
            )?;
        }

        let viewport = Viewport::for_rows(rows, self.visible_height());
        for row in &rows[viewport.start..viewport.end] {
            queue!(self.writer, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
            writeln!(self.writer, "{}", self.render_row(row))?;
        }

        let complete = rows
            .iter()
            .filter(|r| r.state == RowState::Finished)
            .count();
        let running = rows.len().saturating_sub(complete);
        let footer = FooterState {
            visible_start: if rows.is_empty() {
                0
            } else {
                viewport.start + 1
            },
            visible_end: viewport.end,
            total_rows: rows.len(),
            complete,
            running,
            elapsed_ms,
        };
        queue!(self.writer, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
        writeln!(self.writer, "{}", footer.render())?;
        self.writer.flush()?;

        self.rendered_line_count = viewport.end.saturating_sub(viewport.start) + 1;
        Ok(())
    }
}

impl<W: Write> Printer for TtyTablePrinter<W> {
    fn start(&mut self, rows: &[RepoRow]) -> io::Result<()> {
        self.render_frame(rows, 0)
    }

    fn finish_row(
        &mut self,
        rows: &[RepoRow],
        _row_index: usize,
        elapsed_ms: u128,
    ) -> io::Result<()> {
        self.render_frame(rows, elapsed_ms)
    }

    fn complete(&mut self, rows: &[RepoRow], elapsed_ms: u128) -> io::Result<()> {
        self.render_frame(rows, elapsed_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_repo_name_pads_short() {
        let result = format_repo_name("my-repo", 24);
        assert_eq!(result, "[my-repo                 ]");
        assert_eq!(result.len(), 26);
    }

    #[test]
    fn format_repo_name_exact_length() {
        let result = format_repo_name("exactly-twenty-four-chr", 24);
        assert_eq!(result.len(), 26);
    }

    #[test]
    fn format_repo_name_truncates_long() {
        let result = format_repo_name("this-is-a-very-long-repository-name", 24);
        assert_eq!(result, "[this-is-a-very-long--...]");
        assert_eq!(result.len(), 26);
    }

    #[test]
    fn display_repo_name_handles_multibyte_truncation() {
        // floor_char_boundary ensures we never slice through a UTF-8 code point.
        let name = "héllo-wörld-🦀-crates";
        let _ = display_repo_name(name, 10);
    }

    #[test]
    fn plain_printer_formats_repo_and_output_without_ansi() {
        let mut output = Vec::new();
        let rows = vec![RepoRow::finished(
            "agentic-dev".to_string(),
            "running".to_string(),
        )];

        {
            let mut printer = PlainPrinter::new(&mut output, 12);
            printer.start(&rows).expect("plain start");
            printer.finish_row(&rows, 0, 0).expect("plain finish");
        }

        let rendered = String::from_utf8(output).expect("utf8");
        assert_eq!(rendered, "[agentic-dev ] running\n");
        assert!(!rendered.contains('\u{1b}'));
    }

    #[test]
    fn plain_printer_truncates_long_repo_names() {
        let mut output = Vec::new();
        let rows = vec![RepoRow::finished(
            "this-is-a-very-long-repository-name".to_string(),
            "clean".to_string(),
        )];

        {
            let mut printer = PlainPrinter::new(&mut output, 24);
            printer.start(&rows).expect("plain start");
            printer.finish_row(&rows, 0, 0).expect("plain finish");
        }

        let rendered = String::from_utf8(output).expect("utf8");
        assert_eq!(rendered, "[this-is-a-very-long--...] clean\n");
    }

    #[test]
    fn viewport_follows_first_unfinished_repo() {
        let rows = vec![
            RepoRow::finished("activities".to_string(), "clean".to_string()),
            RepoRow::finished("agentic-dev".to_string(), "clean".to_string()),
            RepoRow::running("amion-api".to_string()),
            RepoRow::running("api-gateway".to_string()),
            RepoRow::running("billing".to_string()),
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
            RepoRow::running("activities".to_string()),
            RepoRow::running("agentic-dev".to_string()),
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
            RepoRow::running("activities".to_string()),
            RepoRow::running("agentic-dev".to_string()),
        ];
        let mut output = Vec::new();

        {
            let mut printer = TtyTablePrinter::new(&mut output, 6, 14);
            printer.start(&rows).expect("tty start");
            rows[0] = RepoRow::finished("activities".to_string(), "clean".to_string());
            printer.finish_row(&rows, 0, 0).expect("tty finish");
        }

        let rendered = String::from_utf8(output).expect("utf8");
        assert!(rendered.contains("activities"));
        assert!(rendered.contains("clean"));
        assert!(rendered.contains("agentic-dev"));
    }
}
