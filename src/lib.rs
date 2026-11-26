use std::collections::VecDeque;
use std::io::{self, BufWriter, Stdout, Write};
use std::time::Duration;

pub mod config;

use config::{Bold, Config};

use clap::ValueEnum;
use crossterm::cursor;
use crossterm::event::{self, Event};
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{execute, queue};
use rand::RngExt;
use rand::rngs::SmallRng;

/// Half-width katakana, from 'ｦ' to 'ﾝ': like ASCII, they are one cell wide.
const KATAKANA: std::ops::RangeInclusive<char> = '\u{FF66}'..='\u{FF9D}';

fn rand_char(rng: &mut SmallRng, config: &Config) -> char {
    if config.katakana {
        // Katakana mixed with a few digits, as in the film
        if rng.random_range(0..6) == 0 {
            rng.random_range('0'..='9')
        } else {
            rng.random_range(KATAKANA)
        }
    } else {
        // Printable ASCII characters, from '!' to '}'
        rng.random_range('!'..='}')
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum MatrixColor {
    Green,
    Red,
    Blue,
    White,
    Yellow,
    Cyan,
    Magenta,
    Black,
}

impl MatrixColor {
    /// The next colour when cycling through them (black is skipped: it is
    /// invisible on most terminals).
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            MatrixColor::Green => MatrixColor::Red,
            MatrixColor::Red => MatrixColor::Blue,
            MatrixColor::Blue => MatrixColor::White,
            MatrixColor::White => MatrixColor::Yellow,
            MatrixColor::Yellow => MatrixColor::Cyan,
            MatrixColor::Cyan => MatrixColor::Magenta,
            MatrixColor::Magenta | MatrixColor::Black => MatrixColor::Green,
        }
    }

    /// The colour of a block in fade mode. `depth` is its distance from the
    /// tail of its stream, starting at 1: the tail is dark, and the stream
    /// gets brighter towards its head.
    fn faded(self, depth: usize) -> Color {
        let (r, g, b) = match self {
            MatrixColor::Black => (0, 0, 0),
            MatrixColor::Green => (0, 255, 65),
            MatrixColor::White => (255, 255, 255),
            MatrixColor::Red => (255, 50, 50),
            MatrixColor::Cyan => (0, 230, 255),
            MatrixColor::Magenta => (255, 70, 255),
            MatrixColor::Blue => (70, 120, 255),
            MatrixColor::Yellow => (255, 225, 50),
        };
        let percent =
            FADE_MIN_PERCENT + (100 - FADE_MIN_PERCENT) * depth.min(FADE_LENGTH) / FADE_LENGTH;
        let scale = |channel: usize| (channel * percent / 100) as u8;
        Color::Rgb {
            r: scale(r),
            g: scale(g),
            b: scale(b),
        }
    }

    fn as_crossterm(self) -> Color {
        match self {
            MatrixColor::Black => Color::Black,
            MatrixColor::Green => Color::Green,
            MatrixColor::White => Color::White,
            MatrixColor::Red => Color::Red,
            MatrixColor::Cyan => Color::Cyan,
            MatrixColor::Magenta => Color::Magenta,
            MatrixColor::Blue => Color::Blue,
            MatrixColor::Yellow => Color::Yellow,
        }
    }
}

/// Number of blocks, from the tail of a stream, over which it fades in
const FADE_LENGTH: usize = 8;

/// Brightness of the tail of a stream in fade mode
const FADE_MIN_PERCENT: usize = 15;

/// In asynchronous mode, the slowest columns move once every `MAX_PERIOD` frames
const MAX_PERIOD: usize = 3;

/// The colours a stream can take in rainbow mode
const RAINBOW_COLOURS: [MatrixColor; 6] = [
    MatrixColor::Green,
    MatrixColor::Blue,
    MatrixColor::White,
    MatrixColor::Yellow,
    MatrixColor::Cyan,
    MatrixColor::Magenta,
];

#[derive(Clone)]
pub struct Block {
    val: char,
    white: bool,
    color: MatrixColor,
}

impl Block {
    fn is_space(&self) -> bool {
        self.val == ' '
    }
}

impl Default for Block {
    fn default() -> Self {
        Block {
            val: ' ',
            white: false,
            color: MatrixColor::Red,
        }
    }
}

pub struct Column {
    length: usize,        // The length of the stream
    spaces: usize,        // The spaces between streams
    period: usize,        // In asynchronous mode, the column moves once every `period` frames
    col: VecDeque<Block>, // The actual column
}

impl Column {
    /// Return a column keyed by a random number generator
    fn new(lines: usize, rng: &mut SmallRng) -> Self {
        Column {
            length: rng.random_range(3..lines),
            spaces: rng.random_range(1..=lines),
            period: rng.random_range(1..=MAX_PERIOD),
            col: (0..lines).map(|_| Block::default()).collect(),
        }
    }
    fn head_is_empty(&self) -> bool {
        self.col[1].val == ' '
    }
    fn new_rand_char(&mut self, config: &Config, rng: &mut SmallRng) {
        self.col[0].val = rand_char(rng, config);
        self.col[0].color = self.col[1].color;
    }
    fn new_rand_head(&mut self, config: &Config, rng: &mut SmallRng) {
        self.col[0].val = rand_char(rng, config);
        self.col[0].color = if config.rainbow {
            RAINBOW_COLOURS[rng.random_range(0..RAINBOW_COLOURS.len())]
        } else {
            config.colour
        };
        // 50/50 chance the head is white
        self.col[0].white = rng.random();
    }
    /// Decide what enters the column at its top
    fn feed(&mut self, config: &Config, rng: &mut SmallRng) {
        let lines = self.col.len();

        if self.head_is_empty() && self.spaces != 0 {
            // Decrement the spaces until the next stream starts
            self.spaces -= 1;
        } else if self.head_is_empty() && self.spaces == 0 {
            // Start a new stream
            self.new_rand_head(config, rng);

            // Decrement length of stream
            self.length -= 1;

            // Reset number of spaces until next stream
            self.spaces = rng.random_range(1..=lines);
        } else if self.length != 0 {
            // Continue producing stream
            self.new_rand_char(config, rng);
            self.length -= 1;
        } else {
            // Display spaces until next stream
            self.col[0].val = ' ';
            self.length = rng.random_range(3..lines);
        }
    }
    fn move_down(&mut self, config: &Config, rng: &mut SmallRng) {
        let mut in_stream = false;

        let mut last_was_white = false; // Keep track of white heads
        let mut running_color = MatrixColor::Cyan;

        for block in &mut self.col {
            if !in_stream {
                if !block.is_space() {
                    block.val = ' ';
                    in_stream = true; // We're now in a stream
                    running_color = block.color;
                }
            } else if block.is_space() {
                // New rand char for head of stream
                block.val = rand_char(rng, config);
                block.white = last_was_white;
                in_stream = false;
            }
            // Swapped to "pass on" whiteness and prepare the variable for the next iteration
            std::mem::swap(&mut last_was_white, &mut block.white);
            block.color = running_color;
        }
    }
    fn old_style_move_down(&mut self) {
        self.col.pop_back();
        self.col.push_back(Block::default()); // Put a Blank space at the head.
        self.col.rotate_right(1);
    }
}

impl std::ops::Index<usize> for Column {
    type Output = Block;
    fn index(&self, i: usize) -> &Self::Output {
        &self.col[i]
    }
}

pub struct Matrix {
    m: Vec<Column>,
    rng: SmallRng,
    frame: usize, // Number of frames since the start
}

impl std::ops::Index<usize> for Matrix {
    type Output = Column;
    fn index(&self, i: usize) -> &Self::Output {
        &self.m[i]
    }
}

impl Matrix {
    /// Create a new matrix with the dimensions of the screen
    pub fn from_terminal_size() -> Self {
        let (lines, cols) = get_term_size();
        Self::with_rng(lines, cols, rand::make_rng())
    }

    /// Create a new matrix with the given number of lines and columns
    fn with_rng(lines: usize, cols: usize, mut rng: SmallRng) -> Self {
        Matrix {
            m: (0..cols).map(|_| Column::new(lines, &mut rng)).collect(),
            rng,
            frame: 0,
        }
    }

    fn num_columns(&self) -> usize {
        self.m.len()
    }

    fn num_lines(&self) -> usize {
        self[0].col.len()
    }

    /// Change the size of the matrix to the one of the screen
    pub fn resize_to_terminal(&mut self) {
        let (lines, cols) = get_term_size();
        self.resize(lines, cols);
    }

    /// Change the size of the matrix, keeping the streams that still fit
    fn resize(&mut self, lines: usize, cols: usize) {
        for col in &mut self.m {
            col.col.resize(lines, Block::default());
        }
        self.m.truncate(cols);
        while self.m.len() < cols {
            self.m.push(Column::new(lines, &mut self.rng));
        }
    }

    /// Make the next iteration of matrix
    pub fn arrange(&mut self, config: &Config) {
        self.frame = self.frame.wrapping_add(1);
        let frame = self.frame;
        let rng = &mut self.rng;

        for col in &mut self.m {
            // In asynchronous mode, the slower columns skip frames.
            if config.async_scroll && !frame.is_multiple_of(col.period) {
                continue;
            }
            col.feed(config, rng);
            if config.oldstyle {
                col.old_style_move_down();
            } else {
                col.move_down(config, rng);
            }
        }
    }

    /// Draw the matrix on the screen
    pub fn draw(&self, terminal: &mut Terminal, config: &Config) -> io::Result<()> {
        self.render(&mut terminal.stdout, config)?;
        terminal.stdout.flush()
    }

    /// Write one frame, as terminal escape sequences and characters
    fn render<W: Write>(&self, out: &mut W, config: &Config) -> io::Result<()> {
        // The whole frame is buffered and shown at once, to avoid tearing.
        queue!(out, terminal::BeginSynchronizedUpdate)?;
        // For each column, the distance of the current block from the tail of its stream.
        let mut depth = vec![0; self.num_columns()];
        // Saving the last colour allows us to change colour only when the colour changes.
        let mut last_colour = None;

        for j in 0..self.num_lines() {
            for (i, depth) in depth.iter_mut().enumerate() {
                *depth = if self[i][j].is_space() { 0 } else { *depth + 1 };
            }
            // The first line is where the streams are born: it is never shown.
            if j == 0 {
                continue;
            }

            // Each line is written in one go: the cursor only moves at its start.
            queue!(out, cursor::MoveTo(0, j as u16 - 1))?;
            let mut last_bold = false;
            queue!(out, SetAttribute(Attribute::NormalIntensity))?;

            for i in 0..self.num_columns() {
                let block = &self[i][j];

                // Columns are separated by a space (not after the last one, to avoid wrapping).
                if i > 0 {
                    queue!(out, Print(' '))?;
                }
                // Spaces have no visible colour.
                if !block.is_space() {
                    // Pick the colour we need
                    let colour = if block.white {
                        Color::White
                    } else if config.fade {
                        block.color.faded(depth[i])
                    } else {
                        block.color.as_crossterm()
                    };
                    if last_colour != Some(colour) {
                        // Set the colour in the terminal.
                        queue!(out, SetForegroundColor(colour))?;
                        last_colour = Some(colour);
                    }
                }
                // -b makes the white heads bold, -B makes everything bold.
                let bold = match config.bold {
                    Bold::Off => false,
                    Bold::Heads => block.white,
                    Bold::All => true,
                };
                if last_bold != bold {
                    let attribute = if bold {
                        Attribute::Bold
                    } else {
                        Attribute::NormalIntensity
                    };
                    queue!(out, SetAttribute(attribute))?;
                    last_bold = bold;
                }
                // Draw the character.
                queue!(out, Print(block.val))?;
            }
        }
        queue!(out, terminal::EndSynchronizedUpdate)
    }
}

/// Size of the output buffer: large enough for a typical frame, so that it
/// reaches the terminal in a single write.
const OUTPUT_BUFFER_SIZE: usize = 64 * 1024;

/// Terminal state object
pub struct Terminal {
    stdout: BufWriter<Stdout>,
    active: bool,
}

impl Terminal {
    /// Set up the screen and set important variables
    pub fn new() -> io::Result<Self> {
        terminal::enable_raw_mode()?;

        let mut stdout = BufWriter::with_capacity(OUTPUT_BUFFER_SIZE, io::stdout());
        // Line wrapping is turned off so that a terminal narrower than the
        // matrix cuts the lines instead of scrambling the screen.
        if let Err(error) = execute!(
            stdout,
            terminal::EnterAlternateScreen,
            terminal::DisableLineWrap,
            cursor::Hide,
            Clear(ClearType::All)
        ) {
            let _ = terminal::disable_raw_mode();
            return Err(error);
        }

        Ok(Terminal {
            stdout,
            active: true,
        })
    }

    /// Return the next terminal event, if one is ready
    pub fn get_event(&self, timeout: Duration) -> io::Result<Option<Event>> {
        if event::poll(timeout)? {
            return Ok(Some(event::read()?));
        }
        Ok(None)
    }

    /// Clean up terminal stuff when we're ready to exit
    pub fn finish(mut self) -> io::Result<()> {
        self.restore()
    }

    /// Clear the terminal when the window changes size
    pub fn resize_window(&mut self) -> io::Result<()> {
        execute!(self.stdout, Clear(ClearType::All), cursor::MoveTo(0, 0))?;
        self.stdout.flush()
    }

    fn restore(&mut self) -> io::Result<()> {
        if self.active {
            let terminal_result = execute!(
                self.stdout,
                ResetColor,
                cursor::Show,
                terminal::EnableLineWrap,
                terminal::LeaveAlternateScreen
            );
            let raw_mode_result = terminal::disable_raw_mode();
            self.active = false;
            terminal_result?;
            raw_mode_result?;
        }
        Ok(())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

fn get_term_size() -> (usize, usize) {
    match terminal::size() {
        Ok((width, height)) => {
            // Minimum size for terminal
            let width = usize::from(width.max(10));
            let height = usize::from(height.max(10));
            // Rounding up makes odd-columned screens print on the rightmost edge
            (height + 1, width.div_ceil(2))
        }
        Err(_) => (10, 10),
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    use super::*;

    const LINES: usize = 20;
    const COLUMNS: usize = 30;

    /// A matrix that always plays the same animation
    fn matrix() -> Matrix {
        Matrix::with_rng(LINES, COLUMNS, SmallRng::seed_from_u64(1999))
    }

    /// Remove the escape sequences, keeping what is shown on screen
    fn visible_text(output: &str) -> String {
        let mut text = String::new();
        let mut chars = output.chars();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                // An escape sequence ends with a letter.
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            } else {
                text.push(c);
            }
        }
        text
    }

    fn render(matrix: &Matrix, config: &Config) -> String {
        let mut output = Vec::new();
        matrix.render(&mut output, config).unwrap();
        String::from_utf8(output).unwrap()
    }

    #[test]
    fn rand_char_is_printable() {
        let mut rng = SmallRng::seed_from_u64(1999);
        let config = Config::default();
        let chars: std::collections::HashSet<_> =
            (0..10_000).map(|_| rand_char(&mut rng, &config)).collect();
        assert!(chars.iter().all(|c| ('!'..='}').contains(c)));
        assert!(chars.contains(&'!'));
        assert!(chars.contains(&'}'));
    }

    #[test]
    fn katakana_chars_are_one_cell_wide() {
        let mut rng = SmallRng::seed_from_u64(1999);
        let config = Config {
            katakana: true,
            ..Config::default()
        };
        let chars: std::collections::HashSet<_> =
            (0..10_000).map(|_| rand_char(&mut rng, &config)).collect();
        // Half-width katakana and digits only: wide characters would break the columns.
        assert!(
            chars
                .iter()
                .all(|c| c.is_ascii_digit() || KATAKANA.contains(c))
        );
        assert!(chars.contains(&'ｦ'));
        assert!(chars.contains(&'ﾝ'));
        assert!(chars.contains(&'0'));
        assert!(chars.contains(&'9'));
    }

    #[test]
    fn fade_gets_brighter_towards_the_head() {
        let brightness = |depth| match MatrixColor::Green.faded(depth) {
            Color::Rgb { r, g, b } => u32::from(r) + u32::from(g) + u32::from(b),
            colour => panic!("not an RGB colour: {colour:?}"),
        };
        for depth in 1..FADE_LENGTH {
            assert!(brightness(depth) < brightness(depth + 1));
        }
        assert!(brightness(1) > 0);
        assert_eq!(brightness(FADE_LENGTH), brightness(FADE_LENGTH + 50));
        assert_eq!(
            MatrixColor::Green.faded(FADE_LENGTH),
            Color::Rgb {
                r: 0,
                g: 255,
                b: 65
            }
        );
    }

    #[test]
    fn render_fade_uses_rgb_colours() {
        const RGB: &str = "\x1b[38;2;";
        let mut config = Config::default();
        let mut matrix = matrix();
        for _ in 0..100 {
            matrix.arrange(&config);
        }
        let plain = render(&matrix, &config);
        assert!(!plain.contains(RGB));

        config.fade = true;
        let faded = render(&matrix, &config);
        assert!(faded.contains(RGB));
        // The dark tail of a stream and its full brightness both appear.
        assert!(faded.contains("\x1b[38;2;0;63;16m"));
        assert!(faded.contains("\x1b[38;2;0;255;65m"));
        assert_eq!(visible_text(&plain), visible_text(&faded));
    }

    fn check_arrange(config: &Config) {
        let mut matrix = matrix();
        for _ in 0..500 {
            matrix.arrange(config);
            assert_eq!(matrix.num_columns(), COLUMNS);
            assert_eq!(matrix.num_lines(), LINES);
            for col in &matrix.m {
                for block in &col.col {
                    let valid = if config.katakana {
                        block.val.is_ascii_digit() || KATAKANA.contains(&block.val)
                    } else {
                        ('!'..='}').contains(&block.val)
                    };
                    assert!(block.val == ' ' || valid);
                }
            }
        }
    }

    #[test]
    fn arrange_keeps_dimensions_and_chars() {
        check_arrange(&Config::default());
    }

    #[test]
    fn arrange_oldstyle_keeps_dimensions_and_chars() {
        check_arrange(&Config {
            oldstyle: true,
            ..Config::default()
        });
    }

    #[test]
    fn arrange_katakana_keeps_dimensions_and_chars() {
        check_arrange(&Config {
            katakana: true,
            ..Config::default()
        });
    }

    #[test]
    fn arrange_async_keeps_dimensions_and_chars() {
        check_arrange(&Config {
            async_scroll: true,
            ..Config::default()
        });
    }

    #[test]
    fn async_columns_move_at_their_own_speed() {
        let config = Config {
            async_scroll: true,
            ..Config::default()
        };
        let mut matrix = matrix();
        for period in 1..=MAX_PERIOD {
            assert!(matrix.m.iter().any(|col| col.period == period));
        }
        for _ in 0..100 {
            matrix.arrange(&config);
        }

        let text = |col: &Column| col.col.iter().map(|block| block.val).collect::<String>();
        for frame in 101..=106_usize {
            let before: Vec<_> = matrix.m.iter().map(text).collect();
            matrix.arrange(&config);
            for (col, before) in matrix.m.iter().zip(before) {
                // A column that is not empty always changes when it moves.
                if !frame.is_multiple_of(col.period) {
                    assert_eq!(text(col), before);
                } else if !before.trim().is_empty() {
                    assert_ne!(text(col), before);
                }
            }
        }
    }

    #[test]
    fn resize_keeps_the_streams() {
        let config = Config::default();
        let mut matrix = matrix();
        for _ in 0..100 {
            matrix.arrange(&config);
        }
        let before = visible_text(&render(&matrix, &config));
        let first_line: String = before.chars().take(2 * COLUMNS - 1).collect();
        assert!(first_line.chars().any(|c| c != ' '));

        // Bigger: the old streams are still there, in the top left corner.
        matrix.resize(LINES + 5, COLUMNS + 4);
        assert_eq!(matrix.num_lines(), LINES + 5);
        assert_eq!(matrix.num_columns(), COLUMNS + 4);
        assert!(matrix.m.iter().all(|col| col.col.len() == LINES + 5));
        let bigger = visible_text(&render(&matrix, &config));
        assert!(bigger.starts_with(&first_line));

        // Smaller: what still fits is kept.
        matrix.resize(12, 10);
        assert_eq!(matrix.num_lines(), 12);
        assert_eq!(matrix.num_columns(), 10);
        let smaller = visible_text(&render(&matrix, &config));
        assert!(smaller.starts_with(&first_line[..2 * 10 - 1]));

        // The animation carries on at any size.
        for _ in 0..500 {
            matrix.arrange(&config);
        }
        matrix.resize(LINES, COLUMNS);
        for _ in 0..500 {
            matrix.arrange(&config);
        }
        assert_eq!(matrix.num_lines(), LINES);
    }

    #[test]
    fn same_seed_gives_same_animation() {
        let config = Config::default();
        let (mut first, mut second) = (matrix(), matrix());
        for _ in 0..100 {
            first.arrange(&config);
            second.arrange(&config);
        }
        assert_eq!(render(&first, &config), render(&second, &config));
    }

    #[test]
    fn render_fills_the_screen() {
        let config = Config::default();
        let mut matrix = matrix();
        for _ in 0..100 {
            matrix.arrange(&config);
        }
        let output = render(&matrix, &config);

        assert!(output.starts_with("\x1b[?2026h"));
        assert!(output.ends_with("\x1b[?2026l"));

        // The first line of the matrix is hidden, and columns are separated by a space.
        let text = visible_text(&output);
        assert_eq!(text.chars().count(), (LINES - 1) * (2 * COLUMNS - 1));
        assert!(text.chars().any(|c| c != ' '));
    }

    #[test]
    fn render_bold_modes() {
        const BOLD: &str = "\x1b[1m";
        let mut config = Config::default();
        let mut matrix = matrix();
        for _ in 0..100 {
            matrix.arrange(&config);
        }
        let has_white_head = matrix
            .m
            .iter()
            .any(|col| col.col.iter().skip(1).any(|block| block.white));
        assert!(has_white_head);

        assert!(!render(&matrix, &config).contains(BOLD));
        config.bold = Bold::Heads;
        assert!(render(&matrix, &config).contains(BOLD));
        config.bold = Bold::All;
        assert_eq!(render(&matrix, &config).matches(BOLD).count(), LINES - 1);
    }

    #[test]
    fn colour_cycle_visits_every_visible_colour() {
        let mut seen = vec![MatrixColor::Green];
        let mut colour = MatrixColor::Green.next();
        while colour != MatrixColor::Green {
            assert!(!seen.contains(&colour));
            assert_ne!(colour, MatrixColor::Black);
            seen.push(colour);
            colour = colour.next();
        }
        assert_eq!(seen.len(), 7);
        assert_eq!(MatrixColor::Black.next(), MatrixColor::Green);
    }

    #[test]
    fn rainbow_uses_several_colours() {
        let config = Config {
            rainbow: true,
            ..Config::default()
        };
        let mut matrix = matrix();
        for _ in 0..200 {
            matrix.arrange(&config);
        }
        let colours: std::collections::HashSet<_> = matrix
            .m
            .iter()
            .flat_map(|col| col.col.iter())
            .filter(|block| !block.is_space())
            .map(|block| block.color as u8)
            .collect();
        assert!(colours.len() > 1);
    }
}
