use std::io::{self, Write};

pub(crate) fn format_repo_name(name: &str, width: usize) -> String {
    let display_name = if name.len() > width {
        if width <= 4 {
            name.chars().take(width).collect()
        } else {
            format!("{}-...", &name[..width - 4])
        }
    } else {
        name.to_string()
    };
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
}
