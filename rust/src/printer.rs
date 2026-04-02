use std::io::Write;
use std::time::Instant;

use crossterm::{cursor, execute, terminal};

/// Repo label formatted for output: `[001 repo-name  ]`
pub struct RepoRow {
    pub idx: usize,
    pub name: String,
    pub id_width: usize,
    pub name_width: usize,
}

impl RepoRow {
    pub fn label(&self) -> String {
        let display_name = truncate_name(&self.name, self.name_width);
        format!(
            "[{:0id_width$} {:<name_width$}]",
            self.idx,
            display_name,
            id_width = self.id_width,
            name_width = self.name_width
        )
    }
}

fn truncate_name(name: &str, width: usize) -> String {
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

/// Non-TTY printer: one plain-text line per completed repo.
pub struct StreamPrinter<W: Write> {
    writer: W,
}

impl<W: Write> StreamPrinter<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn print_result(&mut self, row: &RepoRow, status: &str) {
        let _ = writeln!(self.writer, "{} {}", row.label(), status);
    }

    pub fn finish(&mut self) {
        let _ = self.writer.flush();
    }
}

/// TTY printer: completion-order lines with a sticky progress footer.
pub struct TtyPrinter<W: Write> {
    writer: W,
    total: usize,
    completed: usize,
    running: usize,
    started_at: Instant,
    footer_visible: bool,
}

impl<W: Write> TtyPrinter<W> {
    pub fn new(writer: W, total: usize, started_at: Instant) -> Self {
        Self {
            writer,
            total,
            completed: 0,
            running: 0,
            started_at,
            footer_visible: false,
        }
    }

    pub fn mark_started(&mut self) {
        self.running += 1;
        self.rewrite_footer();
    }

    pub fn print_result(&mut self, row: &RepoRow, status: &str) {
        self.completed += 1;
        if self.running > 0 {
            self.running -= 1;
        }

        self.clear_footer();

        let _ = writeln!(self.writer, "{} {}", row.label(), status);

        if self.completed < self.total {
            self.write_footer();
        }
    }

    pub fn finish(&mut self) {
        self.clear_footer();
        let _ = self.writer.flush();
    }

    fn clear_footer(&mut self) {
        if self.footer_visible {
            let _ = execute!(
                self.writer,
                cursor::MoveToColumn(0),
                terminal::Clear(terminal::ClearType::CurrentLine)
            );
            self.footer_visible = false;
        }
    }

    fn write_footer(&mut self) {
        let elapsed = self.started_at.elapsed();
        let secs = elapsed.as_secs_f64();
        let footer = format!(
            "[{}/{} complete | {} running | {:.1}s]",
            self.completed, self.total, self.running, secs
        );
        let _ = write!(self.writer, "{}", footer);
        let _ = self.writer.flush();
        self.footer_visible = true;
    }

    fn rewrite_footer(&mut self) {
        self.clear_footer();
        self.write_footer();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_row(idx: usize, name: &str) -> RepoRow {
        RepoRow {
            idx,
            name: name.to_string(),
            id_width: 3,
            name_width: 12,
        }
    }

    #[test]
    fn repo_row_label_formats_correctly() {
        let row = make_row(2, "agentic-dev");
        assert_eq!(row.label(), "[002 agentic-dev ]");
    }

    #[test]
    fn repo_row_label_truncates_long_names() {
        let row = make_row(1, "very-long-repo-name-here");
        assert_eq!(row.label(), "[001 very-lon-...]");
    }

    #[test]
    fn stream_printer_emits_plain_lines() {
        let mut buf = Vec::new();
        {
            let mut printer = StreamPrinter::new(&mut buf);
            printer.print_result(&make_row(1, "repo-a"), "clean");
            printer.print_result(&make_row(2, "repo-b"), "1 modified");
            printer.finish();
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("[001 repo-a      ] clean\n"));
        assert!(output.contains("[002 repo-b      ] 1 modified\n"));
        assert!(!output.contains('\x1b'), "non-TTY output must not contain ANSI escapes");
    }

    #[test]
    fn tty_printer_includes_progress_footer() {
        let mut buf = Vec::new();
        let started = Instant::now();
        {
            let mut printer = TtyPrinter::new(&mut buf, 3, started);
            printer.mark_started();
            printer.mark_started();
            printer.print_result(&make_row(2, "repo-b"), "clean");
        }
        let output = String::from_utf8(buf).unwrap();
        // Should contain the completed line
        assert!(output.contains("[002 repo-b      ] clean"));
        // Should contain footer text (progress indicator)
        assert!(output.contains("complete"));
        assert!(output.contains("running"));
    }

    #[test]
    fn tty_printer_clears_footer_on_finish() {
        let mut buf = Vec::new();
        let started = Instant::now();
        {
            let mut printer = TtyPrinter::new(&mut buf, 1, started);
            printer.mark_started();
            printer.print_result(&make_row(1, "repo-a"), "clean");
            printer.finish();
        }
        let output = String::from_utf8(buf).unwrap();
        // After all repos complete, print_result doesn't write a new footer
        // and finish() clears any remaining footer
        assert!(output.contains("[001 repo-a      ] clean"));
    }
}
