use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::{self, Stdout, Write};
use std::ops::Range;
use std::thread;
use std::time::Duration;

pub mod config;

use config::Config;

use crossterm::cursor;
use crossterm::event::{self, Event};
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{execute, queue};
use rand::RngExt;
use rand::rngs::SmallRng;

thread_local! {
    static RNG: RefCell<SmallRng> = RefCell::new(rand::make_rng());
}

fn random_range(range: Range<usize>) -> usize {
    RNG.with(|rng| (*rng).borrow_mut().random_range(range))
}

fn rand_char() -> char {
    // Printable ASCII characters, from '!' to '}'
    random_range(33..126) as u8 as char
}

fn coin_flip() -> bool {
    RNG.with(|rng| (*rng).borrow_mut().random())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MatrixColor {
    Black,
    Green,
    White,
    Red,
    Cyan,
    Magenta,
    Blue,
    Yellow,
}

impl MatrixColor {
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
    col: VecDeque<Block>, // The actual column
}

impl Column {
    /// Return a column keyed by a random number generator
    fn new(lines: usize) -> Self {
        Column {
            length: random_range(3..lines),
            spaces: random_range(1..lines + 1),
            col: (0..lines).map(|_| Block::default()).collect(),
        }
    }
    fn head_is_empty(&self) -> bool {
        self.col[1].val == ' '
    }
    fn new_rand_char(&mut self) {
        self.col[0].val = rand_char();
        self.col[0].color = self.col[1].color;
    }
    fn new_rand_head(&mut self, config: &Config) {
        self.col[0].val = rand_char();
        self.col[0].color = if config.rainbow {
            match random_range(0..6) {
                0 => MatrixColor::Green,
                1 => MatrixColor::Blue,
                2 => MatrixColor::White,
                3 => MatrixColor::Yellow,
                4 => MatrixColor::Cyan,
                5 => MatrixColor::Magenta,
                _ => unreachable!(),
            }
        } else {
            config.colour
        };
        // 50/50 chance the head is white
        self.col[0].white = coin_flip();
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
        Self::with_size(lines, cols)
    }

    /// Create a new matrix with the given number of lines and columns
    fn with_size(lines: usize, cols: usize) -> Self {
        Matrix {
            m: (0..cols).map(|_| Column::new(lines)).collect(),
        }
    }

    fn num_columns(&self) -> usize {
        self.m.len()
    }

    fn num_lines(&self) -> usize {
        self[0].col.len()
    }

    /// Make the next iteration of matrix
    pub fn arrange(&mut self, config: &Config) {
        let lines = self.num_lines();

        self.m.iter_mut().for_each(|col| {
            if col.head_is_empty() && col.spaces != 0 {
                // Decrement the spaces until the next stream starts
                col.spaces -= 1;
            } else if col.head_is_empty() && col.spaces == 0 {
                // Start a new stream
                col.new_rand_head(config);

                // Decrement length of stream
                col.length -= 1;

                // Reset number of spaces until next stream
                col.spaces = random_range(1..lines + 1);
            } else if col.length != 0 {
                // Continue producing stream
                col.new_rand_char();
                col.length -= 1;
            } else {
                // Display spaces until next stream
                col.col[0].val = ' ';
                col.length = random_range(3..lines);
            }
        });
        if config.oldstyle {
            self.old_style_move_down();
        } else {
            self.move_down();
        }
    }
    fn move_down(&mut self) {
        self.m.iter_mut().for_each(|col| {
            // Reset for each column
            let mut in_stream = false;

            let mut last_was_white = false; // Keep track of white heads
            let mut running_color = MatrixColor::Cyan;

            col.col.iter_mut().for_each(|block| {
                if !in_stream {
                    if !block.is_space() {
                        block.val = ' ';
                        in_stream = true; // We're now in a stream
                        running_color = block.color;
                    }
                } else if block.is_space() {
                    // New rand char for head of stream
                    block.val = rand_char();
                    block.white = last_was_white;
                    in_stream = false;
                }
                // Swapped to "pass on" whiteness and prepare the variable for the next iteration
                std::mem::swap(&mut last_was_white, &mut block.white);
                block.color = running_color;
            })
        })
    }
    fn old_style_move_down(&mut self) {
        // Iterate over all columns and swap spaces
        self.m.iter_mut().for_each(|col| {
            col.col.pop_back();
            col.col.push_back(Block::default()); // Put a Blank space at the head.
            col.col.rotate_right(1)
        });
    }
    /// Draw the matrix on the screen
    pub fn draw(&self, terminal: &mut Terminal, config: &Config) -> io::Result<()> {
        let stdout = &mut terminal.stdout;

        for j in 1..self.num_lines() {
            // Each line is written in one go: the cursor only moves at its start.
            queue!(stdout, cursor::MoveTo(0, j as u16 - 1))?;
            // Saving the last colour allows us to change colour only when the colour changes.
            let mut last_colour = self[0][j].color;
            queue!(stdout, SetForegroundColor(last_colour.as_crossterm()))?;
            let mut last_bold = false;
            queue!(stdout, SetAttribute(Attribute::NormalIntensity))?;

            for i in 0..self.num_columns() {
                // Pick the colour we need
                let mcolour = if self[i][j].white {
                    MatrixColor::White
                } else {
                    self[i][j].color
                };

                // Columns are separated by a space (not after the last one, to avoid wrapping).
                if i > 0 {
                    queue!(stdout, Print(' '))?;
                }
                if last_colour != mcolour {
                    // Set the colour in the terminal.
                    queue!(stdout, SetForegroundColor(mcolour.as_crossterm()))?;
                    last_colour = mcolour;
                }
                // -b makes the white heads bold, -B makes everything bold.
                let bold = config.bold >= 2 || (config.bold == 1 && self[i][j].white);
                if last_bold != bold {
                    let attribute = if bold {
                        Attribute::Bold
                    } else {
                        Attribute::NormalIntensity
                    };
                    queue!(stdout, SetAttribute(attribute))?;
                    last_bold = bold;
                }
                // Draw the character.
                queue!(stdout, Print(self[i][j].val))?;
            }
        }
        stdout.flush()?;
        thread::sleep(Duration::from_millis(config.update as u64 * 10));
        Ok(())
    }
}

/// Terminal state object
pub struct Terminal {
    stdout: Stdout,
    active: bool,
}

impl Terminal {
    /// Set up the screen and set important variables
    pub fn new() -> io::Result<Self> {
        terminal::enable_raw_mode()?;

        let mut stdout = io::stdout();
        if let Err(error) = execute!(
            stdout,
            terminal::EnterAlternateScreen,
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
        Ok((mut width, mut height)) => {
            // Minimum size for terminal
            if width < 10 {
                width = 10
            }
            if height < 10 {
                height = 10
            }
            if width % 2 != 0 {
                // Makes odd-columned screens print on the rightmost edge
                ((height + 1) as usize, (width / 2 + 1) as usize)
            } else {
                ((height + 1) as usize, (width / 2) as usize)
            }
        }
        Err(_) => (10, 10),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config {
            bold: 0,
            oldstyle: false,
            screensaver: false,
            update: 4,
            colour: MatrixColor::Green,
            rainbow: false,
            pause: false,
        }
    }

    #[test]
    fn rand_char_is_printable() {
        for _ in 0..1000 {
            let c = rand_char();
            assert!(('!'..='}').contains(&c));
        }
    }

    fn check_arrange(config: &Config) {
        let mut matrix = Matrix::with_size(20, 30);
        for _ in 0..500 {
            matrix.arrange(config);
            assert_eq!(matrix.num_columns(), 30);
            assert_eq!(matrix.num_lines(), 20);
            for col in &matrix.m {
                for block in &col.col {
                    assert!(block.val == ' ' || ('!'..='}').contains(&block.val));
                }
            }
        }
    }

    #[test]
    fn arrange_keeps_dimensions_and_chars() {
        check_arrange(&config());
    }

    #[test]
    fn arrange_oldstyle_keeps_dimensions_and_chars() {
        let mut config = config();
        config.oldstyle = true;
        check_arrange(&config);
    }

    #[test]
    fn rainbow_uses_several_colours() {
        let mut config = config();
        config.rainbow = true;
        let mut matrix = Matrix::with_size(20, 30);
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
