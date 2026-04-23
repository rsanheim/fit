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
    Pending,
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
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn pending(repo: String) -> Self {
        Self {
            repo,
            output: "pending".to_string(),
            state: RowState::Pending,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn running(repo: String) -> Self {
        Self {
            repo,
            output: "running".to_string(),
            state: RowState::Running,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn finished(repo: String, output: String) -> Self {
        Self {
            repo,
            output,
            state: RowState::Finished,
        }
    }

    pub fn mark_running(&mut self) {
        self.output = "running".to_string();
        self.state = RowState::Running;
    }

    pub fn mark_finished(&mut self, output: String) {
        self.output = output;
        self.state = RowState::Finished;
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
            .position(|row| row.state != RowState::Finished)
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
    pub pending: usize,
    pub elapsed_ms: u128,
}

impl FooterState {
    pub fn render_message(&self) -> String {
        format!(
            "({}-{} of {}) | {} complete | {} running | {} pending | {:.1}s elapsed",
            self.visible_start,
            self.visible_end,
            self.total_rows,
            self.complete,
            self.running,
            self.pending,
            self.elapsed_ms as f64 / 1000.0,
        )
    }
}

pub trait Printer {
    fn start(&mut self, rows: &[RepoRow]) -> io::Result<()>;
    fn update_row(
        &mut self,
        rows: &[RepoRow],
        row_index: usize,
        elapsed_ms: u128,
    ) -> io::Result<Vec<usize>>;
    fn complete(&mut self, rows: &[RepoRow], elapsed_ms: u128) -> io::Result<Vec<usize>>;
}

pub struct PlainPrinter<W: Write> {
    writer: W,
    repo_width: usize,
    next_to_print: usize,
}

impl<W: Write> PlainPrinter<W> {
    pub fn new(writer: W, repo_width: usize) -> Self {
        Self {
            writer,
            repo_width,
            next_to_print: 0,
        }
    }

    fn flush_finished_rows(&mut self, rows: &[RepoRow]) -> io::Result<Vec<usize>> {
        let mut printed = Vec::new();
        while self.next_to_print < rows.len()
            && rows[self.next_to_print].state == RowState::Finished
        {
            let row = &rows[self.next_to_print];
            writeln!(
                self.writer,
                "{} {}",
                format_repo_name(&row.repo, self.repo_width),
                row.output
            )?;
            printed.push(self.next_to_print);
            self.next_to_print += 1;
        }
        Ok(printed)
    }
}

impl<W: Write> Printer for PlainPrinter<W> {
    fn start(&mut self, _rows: &[RepoRow]) -> io::Result<()> {
        Ok(())
    }

    fn update_row(
        &mut self,
        rows: &[RepoRow],
        row_index: usize,
        _elapsed_ms: u128,
    ) -> io::Result<Vec<usize>> {
        if rows[row_index].state != RowState::Finished {
            return Ok(Vec::new());
        }
        self.flush_finished_rows(rows)
    }

    fn complete(&mut self, rows: &[RepoRow], _elapsed_ms: u128) -> io::Result<Vec<usize>> {
        self.flush_finished_rows(rows)
    }
}

pub struct TtyTablePrinter<W: Write> {
    writer: W,
    terminal_rows: usize,
    terminal_columns: usize,
    repo_width: usize,
    rendered_line_count: usize,
}

impl<W: Write> TtyTablePrinter<W> {
    pub fn new(
        writer: W,
        terminal_rows: usize,
        terminal_columns: usize,
        repo_width: usize,
    ) -> Self {
        Self {
            writer,
            terminal_rows,
            terminal_columns,
            repo_width,
            rendered_line_count: 0,
        }
    }

    fn visible_height(&self) -> usize {
        self.terminal_rows.saturating_sub(2).max(1)
    }

    fn render_row(&self, row: &RepoRow) -> String {
        format!(
            "{:<width$}  {}",
            display_repo_name(&row.repo, self.repo_width),
            row.output,
            width = self.repo_width
        )
    }

    fn render_summary_row(&self, footer: &FooterState) -> String {
        format!(
            "{:<width$}  {}",
            "SUMMARY",
            footer.render_message(),
            width = self.repo_width
        )
    }

    fn render_frame(&mut self, rows: &[RepoRow], elapsed_ms: u128) -> io::Result<Vec<usize>> {
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
        let running = rows.iter().filter(|r| r.state == RowState::Running).count();
        let pending = rows.iter().filter(|r| r.state == RowState::Pending).count();
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
            pending,
            elapsed_ms,
        };
        let summary_row = self.render_summary_row(&footer);
        let separator = "-".repeat(self.terminal_columns.max(summary_row.len()).max(4));
        queue!(self.writer, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
        writeln!(self.writer, "{}", separator)?;
        queue!(self.writer, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
        writeln!(self.writer, "{}", summary_row)?;
        self.writer.flush()?;

        self.rendered_line_count = viewport.end.saturating_sub(viewport.start) + 2;
        Ok(rows
            .iter()
            .enumerate()
            .filter_map(|(idx, row)| (row.state == RowState::Finished).then_some(idx))
            .collect())
    }
}

impl<W: Write> Printer for TtyTablePrinter<W> {
    fn start(&mut self, rows: &[RepoRow]) -> io::Result<()> {
        self.render_frame(rows, 0).map(|_| ())
    }

    fn update_row(
        &mut self,
        rows: &[RepoRow],
        row_index: usize,
        elapsed_ms: u128,
    ) -> io::Result<Vec<usize>> {
        self.render_frame(rows, elapsed_ms)
            .map(|_| match rows[row_index].state {
                RowState::Finished => vec![row_index],
                RowState::Pending | RowState::Running => Vec::new(),
            })
    }

    fn complete(&mut self, rows: &[RepoRow], elapsed_ms: u128) -> io::Result<Vec<usize>> {
        self.render_frame(rows, elapsed_ms).map(|_| Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Clone, Default)]
    struct SharedBuffer(Rc<RefCell<Vec<u8>>>);

    impl SharedBuffer {
        fn rendered(&self) -> String {
            String::from_utf8(self.0.borrow().clone()).expect("utf8")
        }
    }

    impl Write for SharedBuffer {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

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
            printer.update_row(&rows, 0, 0).expect("plain finish");
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
            printer.update_row(&rows, 0, 0).expect("plain finish");
        }

        let rendered = String::from_utf8(output).expect("utf8");
        assert_eq!(rendered, "[this-is-a-very-long--...] clean\n");
    }

    #[test]
    fn plain_printer_buffers_out_of_order_finished_rows_until_contiguous() {
        let output = SharedBuffer::default();
        let mut rows = vec![
            RepoRow::running("activities".to_string()),
            RepoRow::running("agentic-dev".to_string()),
        ];

        {
            let mut printer = PlainPrinter::new(output.clone(), 12);
            printer.start(&rows).expect("plain start");

            rows[1] = RepoRow::finished("agentic-dev".to_string(), "clean".to_string());
            printer
                .update_row(&rows, 1, 100)
                .expect("plain first finish");
            assert_eq!(output.rendered(), "");

            rows[0] = RepoRow::finished("activities".to_string(), "clean".to_string());
            printer
                .update_row(&rows, 0, 200)
                .expect("plain second finish");
        }

        let rendered = output.rendered();
        assert_eq!(rendered, "[activities  ] clean\n[agentic-dev ] clean\n");
    }

    #[test]
    fn viewport_follows_first_unfinished_repo() {
        let rows = vec![
            RepoRow::finished("activities".to_string(), "clean".to_string()),
            RepoRow::pending("agentic-dev".to_string()),
            RepoRow::running("amion-api".to_string()),
            RepoRow::running("api-gateway".to_string()),
            RepoRow::running("billing".to_string()),
        ];

        let viewport = Viewport::for_rows(&rows, 3);

        assert_eq!(viewport.start, 1);
        assert_eq!(viewport.end, 4);
    }

    #[test]
    fn footer_includes_slice_counts_elapsed_and_pending() {
        let footer = FooterState {
            visible_start: 24,
            visible_end: 47,
            total_rows: 98,
            complete: 41,
            running: 8,
            pending: 49,
            elapsed_ms: 2100,
        };

        assert_eq!(
            footer.render_message(),
            "(24-47 of 98) | 41 complete | 8 running | 49 pending | 2.1s elapsed"
        );
    }

    #[test]
    fn tty_table_printer_renders_pending_rows_without_headers() {
        let rows = vec![
            RepoRow::pending("activities".to_string()),
            RepoRow::pending("agentic-dev".to_string()),
        ];
        let mut output = Vec::new();

        {
            let mut printer = TtyTablePrinter::new(&mut output, 6, 80, 14);
            printer.start(&rows).expect("tty start");
        }

        let rendered = String::from_utf8(output).expect("utf8");
        assert!(rendered.contains("activities"));
        assert!(rendered.contains("pending"));
        assert!(!rendered.contains("REPO"));
        assert!(!rendered.contains("OUTPUT"));
    }

    #[test]
    fn tty_table_printer_updates_finished_rows_without_waiting_for_earlier_rows() {
        let rows = vec![
            RepoRow::pending("activities".to_string()),
            RepoRow::finished("agentic-dev".to_string(), "clean".to_string()),
        ];
        let mut output = Vec::new();

        {
            let mut printer = TtyTablePrinter::new(&mut output, 6, 80, 14);
            printer.start(&rows).expect("tty start");
            printer
                .update_row(&rows, 1, 200)
                .expect("tty out of order finish");
        }

        let rendered = String::from_utf8(output).expect("utf8");
        assert!(rendered.contains("activities"));
        assert!(rendered.contains("pending"));
        assert!(rendered.contains("agentic-dev"));
        assert!(rendered.contains("clean"));
    }

    #[test]
    fn tty_table_printer_renders_summary_row_with_separator() {
        let rows = vec![RepoRow::finished(
            "activities".to_string(),
            "clean".to_string(),
        )];
        let mut output = Vec::new();

        {
            let mut printer = TtyTablePrinter::new(&mut output, 6, 80, 14);
            printer.start(&rows).expect("tty start");
            printer.complete(&rows, 1200).expect("tty complete");
        }

        let rendered = String::from_utf8(output).expect("utf8");
        assert!(rendered.contains("SUMMARY"));
        assert!(rendered.contains("elapsed"));
        assert!(rendered.contains("----"));
    }

    #[test]
    fn tty_table_printer_separator_scales_past_tiny_terminal_width() {
        let rows = vec![RepoRow::finished(
            "activities".to_string(),
            "clean".to_string(),
        )];
        let mut output = Vec::new();

        {
            let mut printer = TtyTablePrinter::new(&mut output, 6, 1, 14);
            printer.complete(&rows, 1200).expect("tty complete");
        }

        let rendered = String::from_utf8(output).expect("utf8");
        assert!(rendered.contains("----"));
    }

    #[test]
    fn tty_table_printer_keeps_completed_rows_in_place() {
        let mut rows = vec![
            RepoRow::running("activities".to_string()),
            RepoRow::running("agentic-dev".to_string()),
        ];
        let mut output = Vec::new();

        {
            let mut printer = TtyTablePrinter::new(&mut output, 6, 80, 14);
            printer.start(&rows).expect("tty start");
            rows[0].mark_finished("clean".to_string());
            printer.update_row(&rows, 0, 0).expect("tty finish");
        }

        let rendered = String::from_utf8(output).expect("utf8");
        assert!(rendered.contains("activities"));
        assert!(rendered.contains("clean"));
        assert!(rendered.contains("agentic-dev"));
    }
}
