use std::io::Write;
use std::time::Instant;

use crossterm::{cursor, execute, terminal};

/// Repo label formatted for output: `[repo-name  ]`
pub struct RepoRow {
    pub name: String,
    pub name_width: usize,
}

impl RepoRow {
    pub fn label(&self) -> String {
        let display_name = truncate_name(&self.name, self.name_width);
        format!("[{:<width$}]", display_name, width = self.name_width)
    }
}

fn truncate_name(name: &str, width: usize) -> String {
    if name.len() > width {
        if width <= 4 {
            name.chars().take(width).collect()
        } else {
            let prefix_len = width - 4;
            let safe_end = name.floor_char_boundary(prefix_len);
            format!("{}-...", &name[..safe_end])
        }
    } else {
        name.to_string()
    }
}

pub trait Printer {
    fn mark_started(&mut self);
    fn print_result(&mut self, row: &RepoRow, status: &str);
    fn finish(&mut self);
}

/// Non-TTY printer: one plain-text line per completed repo.
pub struct StreamPrinter<W: Write> {
    writer: W,
}

impl<W: Write> StreamPrinter<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

impl<W: Write> Printer for StreamPrinter<W> {
    fn mark_started(&mut self) {}

    fn print_result(&mut self, row: &RepoRow, status: &str) {
        let _ = writeln!(self.writer, "{} {}", row.label(), status);
    }

    fn finish(&mut self) {
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
}

impl<W: Write> Printer for TtyPrinter<W> {
    fn mark_started(&mut self) {
        self.running += 1;
        self.clear_footer();
        self.write_footer();
    }

    fn print_result(&mut self, row: &RepoRow, status: &str) {
        self.completed += 1;
        debug_assert!(
            self.running > 0,
            "print_result called without preceding mark_started"
        );
        self.running = self.running.saturating_sub(1);

        self.clear_footer();

        let _ = writeln!(self.writer, "{} {}", row.label(), status);

        if self.completed < self.total {
            self.write_footer();
        }
    }

    fn finish(&mut self) {
        self.clear_footer();
        let _ = self.writer.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_row(name: &str) -> RepoRow {
        RepoRow {
            name: name.to_string(),
            name_width: 12,
        }
    }

    #[test]
    fn repo_row_label_formats_correctly() {
        let row = make_row("agentic-dev");
        assert_eq!(row.label(), "[agentic-dev ]");
    }

    #[test]
    fn repo_row_label_truncates_long_names() {
        let row = make_row("very-long-repo-name-here");
        assert_eq!(row.label(), "[very-lon-...]");
    }

    #[test]
    fn stream_printer_emits_plain_lines() {
        let mut buf = Vec::new();
        {
            let mut printer = StreamPrinter::new(&mut buf);
            printer.print_result(&make_row("repo-a"), "clean");
            printer.print_result(&make_row("repo-b"), "1 modified");
            printer.finish();
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("[repo-a      ] clean\n"));
        assert!(output.contains("[repo-b      ] 1 modified\n"));
        assert!(
            !output.contains('\x1b'),
            "non-TTY output must not contain ANSI escapes"
        );
    }

    #[test]
    fn stream_printer_mark_started_is_noop() {
        let mut buf = Vec::new();
        {
            let mut printer = StreamPrinter::new(&mut buf);
            printer.mark_started();
            printer.mark_started();
            printer.finish();
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn tty_printer_includes_progress_footer() {
        let mut buf = Vec::new();
        let started = Instant::now();
        {
            let mut printer = TtyPrinter::new(&mut buf, 3, started);
            printer.mark_started();
            printer.mark_started();
            printer.print_result(&make_row("repo-b"), "clean");
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("[repo-b      ] clean"));
        assert!(output.contains("complete"));
        assert!(output.contains("running"));
    }

    #[test]
    fn tty_printer_interleaved_events() {
        let mut buf = Vec::new();
        let started = Instant::now();
        {
            let mut printer = TtyPrinter::new(&mut buf, 3, started);
            printer.mark_started(); // repo a starts
            printer.mark_started(); // repo b starts
            printer.print_result(&make_row("repo-b"), "clean"); // b finishes
            printer.mark_started(); // repo c starts
            printer.print_result(&make_row("repo-a"), "1 modified"); // a finishes
            printer.print_result(&make_row("repo-c"), "clean"); // c finishes
            printer.finish();
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("[repo-b      ] clean"));
        assert!(output.contains("[repo-a      ] 1 modified"));
        assert!(output.contains("[repo-c      ] clean"));
    }

    #[test]
    fn tty_printer_clears_footer_on_finish() {
        let mut buf = Vec::new();
        let started = Instant::now();
        {
            let mut printer = TtyPrinter::new(&mut buf, 1, started);
            printer.mark_started();
            printer.print_result(&make_row("repo-a"), "clean");
            printer.finish();
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("[repo-a      ] clean"));
    }
}
