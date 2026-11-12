//! A pane's screen with no window to draw it: the terminal emulator the
//! desktop's terminals use (alacritty's), fed the pane's output, read as text.
//!
//! What it is for: an agent reading another pane (`pane_read`), the trust
//! question looked for on screen, and the answers a program asks of its
//! terminal (the cursor's position, the device's attributes), written back
//! to the pane as a window's terminal would: a program that asks and is
//! never answered can wait for good.

use std::sync::{Arc, Mutex, PoisonError};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{self, Term, TermMode};
use alacritty_terminal::vte::ansi::Processor;

/// The rows kept above the screen: what an agent's longest read (1,000
/// lines, `control/panes.rs`) takes when its lines wrap onto two rows each.
/// Nobody scrolls a screen with no window, and each row kept is paid for in
/// every pane: 120 columns of cells are 2.9 kB, 5.8 MiB for these rows
/// (benches/scale.rs, `memory`).
pub const HISTORY: usize = 2_000;

/// What the emulator says back to the program.
#[derive(Clone, Default)]
struct Answers(Arc<Mutex<Vec<String>>>);

impl EventListener for Answers {
    fn send_event(&self, event: Event) {
        if let Event::PtyWrite(text) = event {
            self.0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(text);
        }
    }
}

struct Size {
    columns: usize,
    rows: usize,
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.rows
    }
    fn screen_lines(&self) -> usize {
        self.rows
    }
    fn columns(&self) -> usize {
        self.columns
    }
}

pub struct Screen {
    term: Term<Answers>,
    parser: Processor,
    answers: Answers,
    columns: usize,
    rows: usize,
}

impl std::fmt::Debug for Screen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Screen")
            .field("columns", &self.columns)
            .field("rows", &self.rows)
            .finish()
    }
}

impl Screen {
    pub fn new(columns: u16, rows: u16) -> Screen {
        Screen::with_history(columns, rows, HISTORY)
    }

    /// A screen keeping `history` rows above what shows.
    pub fn with_history(columns: u16, rows: u16, history: usize) -> Screen {
        let columns = (columns as usize).max(term::MIN_COLUMNS);
        let rows = (rows as usize).max(term::MIN_SCREEN_LINES);
        let answers = Answers::default();
        let config = term::Config {
            scrolling_history: history,
            ..term::Config::default()
        };
        Screen {
            term: Term::new(config, &Size { columns, rows }, answers.clone()),
            parser: Processor::new(),
            answers,
            columns,
            rows,
        }
    }

    /// The program's output; answers what the program asked of its
    /// terminal, to be written back to it.
    pub fn feed(&mut self, data: &[u8]) -> Vec<String> {
        self.parser.advance(&mut self.term, data);
        std::mem::take(
            &mut *self
                .answers
                .0
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        )
    }

    /// The pane was resized: the screen takes its new size, as a window's
    /// terminal does, so that what a full-screen program draws for that
    /// size lands where it drew it.
    pub fn resize(&mut self, columns: u16, rows: u16) {
        let columns = (columns as usize).max(term::MIN_COLUMNS);
        let rows = (rows as usize).max(term::MIN_SCREEN_LINES);
        if (columns, rows) == (self.columns, self.rows) {
            return;
        }
        self.term.resize(Size { columns, rows });
        self.columns = columns;
        self.rows = rows;
    }

    /// The program asked for pastes to be bracketed.
    pub fn bracketed_paste(&self) -> bool {
        self.term.mode().contains(TermMode::BRACKETED_PASTE)
    }

    /// The program asked for the cursor keys' application sequences.
    pub fn application_cursor(&self) -> bool {
        self.term.mode().contains(TermMode::APP_CURSOR)
    }

    /// The rows of the screen as the program drew them, without the history
    /// behind them.
    pub fn rows(&self) -> Vec<String> {
        (0..self.rows as i32)
            .map(|row| self.row_text(Line(row), true))
            .collect()
    }

    /// The last `n` lines of text, history included, oldest first; a line
    /// the emulator wrapped is read as the one line it was printed as, and
    /// the blank rows under the last text are not lines (the desktop's
    /// `Terminal::read_lines`).
    pub fn lines(&self, n: usize) -> Vec<String> {
        if n == 0 {
            return Vec::new();
        }
        let grid = self.term.grid();
        let top = self.term.topmost_line();
        let mut last = self.term.bottommost_line();
        while last > top && self.row_is_blank(last) {
            last = Line(last.0 - 1);
        }
        if self.row_is_blank(last) {
            return Vec::new();
        }
        let wraps_into_next = |line: Line| {
            line > top
                && grid[Line(line.0 - 1)][Column(self.columns - 1)]
                    .flags
                    .contains(Flags::WRAPLINE)
        };
        let mut lines = Vec::new();
        let mut end = last;
        loop {
            let mut first = end;
            while wraps_into_next(first) {
                first = Line(first.0 - 1);
            }
            let mut text = String::new();
            for line in first.0..=end.0 {
                text.push_str(&self.row_text(Line(line), line == end.0));
            }
            lines.push(text);
            if lines.len() == n || first == top {
                break;
            }
            end = Line(first.0 - 1);
        }
        lines.reverse();
        lines
    }

    fn row_text(&self, line: Line, trim: bool) -> String {
        let grid = self.term.grid();
        let mut text = String::new();
        for column in 0..self.columns {
            let cell = &grid[line][Column(column)];
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            text.push(if cell.c == '\0' { ' ' } else { cell.c });
            for extra in cell.zerowidth().into_iter().flatten() {
                text.push(*extra);
            }
        }
        if trim {
            text.truncate(text.trim_end().len());
        }
        text
    }

    fn row_is_blank(&self, line: Line) -> bool {
        let grid = self.term.grid();
        (0..self.columns).all(|column| {
            let cell = &grid[line][Column(column)];
            matches!(cell.c, ' ' | '\0') && cell.zerowidth().is_none_or(|z| z.is_empty())
        })
    }
}

/// Every pane's screen, by pane: what a host's terminals keep beside the
/// panes' shells. Each screen has a lock of its own: a pane that prints
/// holds up nobody else's, and what reads a screen waits for that pane
/// alone.
#[derive(Clone, Default)]
pub struct Screens(Arc<std::sync::RwLock<std::collections::HashMap<String, Fed>>>);

/// One pane's screen, as its reader thread feeds it: handed out by
/// [`Screens::open`], so that a chunk printed looks nothing up.
#[derive(Clone)]
pub struct Fed(Arc<Mutex<Screen>>);

impl Fed {
    fn screen(&self) -> std::sync::MutexGuard<'_, Screen> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The pane printed `data`: what its program asked of its terminal, to
    /// be written back to it.
    pub fn feed(&self, data: &[u8]) -> Vec<String> {
        self.screen().feed(data)
    }
}

impl Screens {
    fn get(&self, pane: &str) -> Option<Fed> {
        self.0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(pane)
            .cloned()
    }

    /// A new screen for `pane`, in place of any it had.
    pub fn open(&self, pane: &str, columns: u16, rows: u16) -> Fed {
        let fed = Fed(Arc::new(Mutex::new(Screen::new(columns, rows))));
        self.0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(pane.to_string(), fed.clone());
        fed
    }

    /// `pane`'s screen at its new size; nothing for a pane with no screen.
    pub fn resize(&self, pane: &str, columns: u16, rows: u16) {
        if let Some(fed) = self.get(pane) {
            fed.screen().resize(columns, rows);
        }
    }

    pub fn close(&self, pane: &str) {
        self.0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(pane);
    }

    pub fn clear(&self) {
        self.0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }

    /// What `read` reads of `pane`'s screen; `None` when it has none.
    pub fn read<T>(&self, pane: &str, read: impl FnOnce(&Screen) -> T) -> Option<T> {
        let fed = self.get(pane)?;
        let screen = fed.screen();
        Some(read(&screen))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_was_printed_without_its_escapes() {
        let mut screen = Screen::new(40, 10);
        screen.feed(b"\x1b[1;32mhello\x1b[0m world\r\nsecond line\r\n");
        assert_eq!(screen.lines(5), ["hello world", "second line"]);
        assert_eq!(screen.rows()[0], "hello world");
    }

    #[test]
    fn a_long_line_wrapped_on_screen_reads_as_one() {
        let mut screen = Screen::new(10, 5);
        screen.feed(b"abcdefghijklmnop\r\n");
        assert_eq!(screen.lines(1), ["abcdefghijklmnop"]);
        assert_eq!(&screen.rows()[..2], ["abcdefghij", "klmnop"]);
    }

    #[test]
    fn answers_the_cursor_position_a_program_asks_for() {
        let mut screen = Screen::new(20, 5);
        let answers = screen.feed(b"ab\x1b[6n");
        assert_eq!(answers, ["\x1b[1;3R"]);
        assert!(screen.feed(b"c").is_empty());
    }

    #[test]
    fn each_pane_has_a_screen_of_its_own_until_it_is_closed() {
        let screens = Screens::default();
        let leo = screens.open("leo", 40, 10);
        let ada = screens.open("ada", 40, 10);
        leo.feed(b"from leo\r\n");
        ada.feed(b"from ada\r\n");
        assert_eq!(
            screens.read("leo", |s| s.lines(5)),
            Some(vec!["from leo".to_string()])
        );
        assert_eq!(
            screens.read("ada", |s| s.lines(5)),
            Some(vec!["from ada".to_string()])
        );
        // A pane restarted starts from an empty screen.
        let again = screens.open("leo", 40, 10);
        again.feed(b"new\r\n");
        assert_eq!(
            screens.read("leo", |s| s.lines(5)),
            Some(vec!["new".to_string()])
        );
        // While one pane's screen is held (it is being fed), another's is
        // read without waiting.
        let held = again.0.lock().unwrap();
        assert_eq!(screens.read("ada", |s| s.lines(5).len()), Some(1));
        drop(held);
        screens.close("ada");
        assert_eq!(screens.read("ada", |s| s.lines(5)), None);
    }

    #[test]
    fn the_longest_read_is_served_long_after_it_scrolled_away() {
        let mut screen = Screen::new(120, 40);
        for line in 0..(HISTORY * 3) {
            screen.feed(format!("line {line}\r\n").as_bytes());
        }
        let read = screen.lines(1_000);
        assert_eq!(read.len(), 1_000);
        assert_eq!(read[999], format!("line {}", HISTORY * 3 - 1));
        assert_eq!(read[0], format!("line {}", HISTORY * 3 - 1_000));
        // What is kept is bounded: the rows above the screen, and the screen.
        assert!(screen.lines(usize::MAX).len() <= HISTORY + 40);
    }

    #[test]
    fn a_resized_screen_wraps_at_its_new_width_and_keeps_what_it_showed() {
        let mut screen = Screen::new(10, 5);
        screen.feed(b"kept\r\n");
        screen.resize(20, 8);
        screen.feed(b"abcdefghijklmnop\r\n");
        assert_eq!(screen.rows().len(), 8);
        assert_eq!(&screen.rows()[..2], ["kept", "abcdefghijklmnop"]);
        // Through the panes' screens, by pane; a pane with none is nothing.
        let screens = Screens::default();
        screens.open("leo", 10, 5);
        screens.resize("leo", 30, 12);
        screens.resize("nobody", 30, 12);
        assert_eq!(screens.read("leo", |s| s.rows().len()), Some(12));
    }

    #[test]
    fn a_cleared_screen_reads_empty() {
        let mut screen = Screen::new(20, 5);
        screen.feed(b"gone\x1b[2J\x1b[H");
        assert!(screen.rows().iter().all(String::is_empty));
    }
}
