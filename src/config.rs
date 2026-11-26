use std::io;
use std::time::Duration;

use clap::{CommandFactory, Parser};
use clap_complete::Shell;

use super::MatrixColor;

#[derive(Debug, Parser)]
#[command(
    name = "rmatrix",
    version,
    about = "Shows a scrolling 'Matrix' like screen in your terminal"
)]
struct Opt {
    #[arg(short = 'b')]
    /// Bold characters on
    bold: bool,

    #[arg(short = 'B')]
    /// All bold characters (overrides -b)
    all_bold: bool,

    #[arg(short = 'o', long = "oldstyle")]
    /// Use old-style scrolling
    oldstyle: bool,

    #[arg(short = 's', long = "screensaver")]
    /// "Screensaver" mode, exits on first keystroke
    screensaver: bool,

    #[arg(
        short = 'u',
        long = "update",
        default_value = "4",
        value_parser = validate_update
    )]
    /// Screen update delay (0-10)
    update: usize,

    #[arg(
        short = 'C',
        long = "colour",
        value_enum,
        default_value_t = MatrixColor::Green
    )]
    /// Colour of the characters
    colour: MatrixColor,

    #[arg(short = 'r', long = "rainbow")]
    /// Rainbow mode
    rainbow: bool,

    #[arg(short = 'f', long = "fade")]
    /// Fade the tail of the streams (needs a true colour terminal)
    fade: bool,

    #[arg(short = 'c', long = "katakana")]
    /// Use Japanese characters, as in the film (needs a font with half-width katakana)
    katakana: bool,

    #[arg(short = 'a', long = "async")]
    /// Asynchronous scroll: each column falls at its own speed
    async_scroll: bool,

    #[arg(long = "completions", value_name = "SHELL")]
    /// Print the completion script for a shell and exit
    completions: Option<Shell>,

    #[arg(long = "man")]
    /// Print the manual page and exit
    man: bool,
}

/// Print a completion script or the manual page, if one was asked for.
/// Returns false when nothing was asked.
fn print_documentation(opt: &Opt) -> bool {
    let mut stdout = io::stdout();
    if let Some(shell) = opt.completions {
        clap_complete::generate(shell, &mut Opt::command(), "rmatrix", &mut stdout);
    } else if opt.man {
        // Nothing can be done if the output is closed.
        let _ = clap_mangen::Man::new(Opt::command()).render(&mut stdout);
    } else {
        return false;
    }
    true
}

fn validate_update(n: &str) -> Result<usize, String> {
    if let Ok(n) = n.parse::<usize>()
        && n <= MAX_UPDATE
    {
        return Ok(n);
    }
    Err(format!("must be a number between 0 and {MAX_UPDATE}"))
}

/// Largest screen update delay.
const MAX_UPDATE: usize = 10;

/// Frame delay used for update delay 0, in milliseconds.
const MIN_FRAME_DELAY_MS: u64 = 5;

/// Which characters are drawn in bold
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Bold {
    /// No bold characters
    #[default]
    Off,
    /// Only the white heads of the streams
    Heads,
    /// Every character
    All,
}

/// The global state object
pub struct Config {
    pub bold: Bold,
    pub oldstyle: bool,
    pub screensaver: bool,
    pub update: usize,
    pub colour: MatrixColor,
    pub rainbow: bool,
    pub fade: bool,
    pub katakana: bool,
    pub async_scroll: bool,
    pub pause: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            bold: Bold::Off,
            oldstyle: false,
            screensaver: false,
            update: 4,
            colour: MatrixColor::Green,
            rainbow: false,
            fade: false,
            katakana: false,
            async_scroll: false,
            pause: false,
        }
    }
}

impl Config {
    /// Build the configuration from the command line arguments
    pub fn from_args() -> Self {
        let opt = Opt::parse();
        if print_documentation(&opt) {
            std::process::exit(0);
        }

        Config {
            bold: if opt.all_bold {
                Bold::All
            } else if opt.bold {
                Bold::Heads
            } else {
                Bold::Off
            },
            oldstyle: opt.oldstyle,
            screensaver: opt.screensaver,
            update: opt.update,
            colour: opt.colour,
            rainbow: opt.rainbow,
            fade: opt.fade,
            katakana: opt.katakana,
            async_scroll: opt.async_scroll,
            pause: false,
        }
    }

    /// Time between two frames. Delay 0 still waits a little, so the
    /// animation never spins a CPU core at full speed.
    pub fn frame_delay(&self) -> Duration {
        Duration::from_millis((self.update as u64 * 10).max(MIN_FRAME_DELAY_MS))
    }

    /// Make the letters fall faster, down to delay 0.
    pub fn speed_up(&mut self) {
        self.update = self.update.saturating_sub(1);
    }

    /// Make the letters fall slower, up to the largest delay.
    pub fn slow_down(&mut self) {
        self.update = (self.update + 1).min(MAX_UPDATE);
    }

    /// Use a single colour, which turns rainbow mode off.
    fn set_colour(&mut self, colour: MatrixColor) {
        self.colour = colour;
        self.rainbow = false;
    }

    /// Handle a character key. Returns true if the program should exit.
    pub fn handle_keypress(&mut self, keypress: char) -> bool {
        if self.screensaver {
            return true;
        }

        match keypress {
            'q' => return true,
            'b' => self.bold = Bold::Heads,
            'B' => self.bold = Bold::All,
            'n' => self.bold = Bold::Off,
            '!' => self.set_colour(MatrixColor::Red),
            '@' => self.set_colour(MatrixColor::Green),
            '#' => self.set_colour(MatrixColor::Yellow),
            '$' => self.set_colour(MatrixColor::Blue),
            '%' => self.set_colour(MatrixColor::Magenta),
            '^' => self.set_colour(MatrixColor::Cyan),
            '&' => self.set_colour(MatrixColor::White),
            // Next colour: works on every keyboard layout, unlike the symbols.
            'c' | 'C' => self.set_colour(self.colour.next()),
            'r' => self.rainbow = !self.rainbow,
            'f' => self.fade = !self.fade,
            'k' => self.katakana = !self.katakana,
            'a' => self.async_scroll = !self.async_scroll,
            'p' | 'P' => self.pause = !self.pause,
            '0'..='9' => self.update = keypress as usize - '0' as usize,
            _ => {}
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_is_valid() {
        Opt::command().debug_assert();
    }

    #[test]
    fn colour_option() {
        let opt = Opt::try_parse_from(["rmatrix"]).unwrap();
        assert_eq!(opt.colour, MatrixColor::Green);
        let opt = Opt::try_parse_from(["rmatrix", "-C", "magenta"]).unwrap();
        assert_eq!(opt.colour, MatrixColor::Magenta);
        assert!(Opt::try_parse_from(["rmatrix", "-C", "pink"]).is_err());
    }

    #[test]
    fn update_delay_range() {
        assert_eq!(validate_update("0"), Ok(0));
        assert_eq!(validate_update("10"), Ok(10));
        assert!(validate_update("11").is_err());
        assert!(validate_update("-1").is_err());
        assert!(validate_update("abc").is_err());
    }

    #[test]
    fn frame_delay_follows_update() {
        let mut c = Config::default();
        assert_eq!(c.frame_delay(), Duration::from_millis(40));
        c.update = 10;
        assert_eq!(c.frame_delay(), Duration::from_millis(100));
        c.update = 0;
        assert_eq!(c.frame_delay(), Duration::from_millis(MIN_FRAME_DELAY_MS));
    }

    #[test]
    fn speed_steps_stay_in_range() {
        let mut c = Config::default();
        c.speed_up();
        assert_eq!(c.update, 3);
        for _ in 0..20 {
            c.speed_up();
        }
        assert_eq!(c.update, 0);
        c.slow_down();
        assert_eq!(c.update, 1);
        for _ in 0..20 {
            c.slow_down();
        }
        assert_eq!(c.update, MAX_UPDATE);
    }

    #[test]
    fn quit_keys() {
        assert!(Config::default().handle_keypress('q'));
        assert!(!Config::default().handle_keypress('z'));

        let mut screensaver = Config {
            screensaver: true,
            ..Config::default()
        };
        assert!(screensaver.handle_keypress('z'));
    }

    #[test]
    fn option_keys() {
        let mut c = Config::default();
        c.handle_keypress('B');
        assert_eq!(c.bold, Bold::All);
        c.handle_keypress('b');
        assert_eq!(c.bold, Bold::Heads);
        c.handle_keypress('n');
        assert_eq!(c.bold, Bold::Off);

        c.handle_keypress('p');
        assert!(c.pause);
        c.handle_keypress('P');
        assert!(!c.pause);

        c.handle_keypress('7');
        assert_eq!(c.update, 7);
        c.handle_keypress('0');
        assert_eq!(c.update, 0);

        c.handle_keypress('r');
        assert!(c.rainbow);
        c.handle_keypress('c');
        assert!(!c.rainbow);
        assert_eq!(c.colour, MatrixColor::Red);
        c.handle_keypress('r');
        assert!(c.rainbow);
        c.handle_keypress('!');
        assert!(!c.rainbow);
        assert_eq!(c.colour, MatrixColor::Red);
        c.handle_keypress('r');
        c.handle_keypress('r');
        assert!(!c.rainbow);

        c.handle_keypress('f');
        assert!(c.fade);
        c.handle_keypress('f');
        assert!(!c.fade);

        c.handle_keypress('k');
        assert!(c.katakana);
        c.handle_keypress('k');
        assert!(!c.katakana);

        c.handle_keypress('a');
        assert!(c.async_scroll);
        c.handle_keypress('a');
        assert!(!c.async_scroll);
    }
}
