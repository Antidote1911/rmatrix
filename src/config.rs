use clap::Parser;

use super::MatrixColor;

#[derive(Debug, Parser)]
#[command(
    name = "rmatrix",
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
        default_value = "green",
        value_parser = ["green", "red", "blue", "white", "yellow", "cyan", "magenta", "black"]
    )]
    colour: String,

    #[arg(short = 'r', long = "rainbow")]
    /// Rainbow mode
    rainbow: bool,
}

fn validate_update(n: &str) -> Result<usize, String> {
    if let Ok(n) = n.parse::<usize>()
        && n <= 10
    {
        return Ok(n);
    }
    Err("must be a number between 0 and 10".to_string())
}

/// The global state object
pub struct Config {
    pub bold: u8,
    pub oldstyle: bool,
    pub screensaver: bool,
    pub update: usize,
    pub colour: MatrixColor,
    pub rainbow: bool,
    pub pause: bool,
}

impl Config {
    /// Build the configuration from the command line arguments
    pub fn from_args() -> Self {
        let opt = Opt::parse();

        let colour = match opt.colour.as_ref() {
            "green" => MatrixColor::Green,
            "red" => MatrixColor::Red,
            "blue" => MatrixColor::Blue,
            "white" => MatrixColor::White,
            "yellow" => MatrixColor::Yellow,
            "cyan" => MatrixColor::Cyan,
            "magenta" => MatrixColor::Magenta,
            "black" => MatrixColor::Black,
            _ => unreachable!(),
        };

        Config {
            bold: if opt.all_bold { 2 } else { opt.bold as u8 },
            oldstyle: opt.oldstyle,
            screensaver: opt.screensaver,
            update: opt.update,
            rainbow: opt.rainbow,
            colour,
            pause: false,
        }
    }

    pub fn handle_keypress(&mut self, keypress: char) -> bool {
        if self.screensaver {
            return true;
        }

        match keypress {
            'q' => return true,
            'b' => self.bold = 1,
            'B' => self.bold = 2,
            'n' => self.bold = 0,
            '!' => {
                self.colour = MatrixColor::Red;
                self.rainbow = false;
            }
            '@' => {
                self.colour = MatrixColor::Green;
                self.rainbow = false;
            }
            '#' => {
                self.colour = MatrixColor::Yellow;
                self.rainbow = false;
            }
            '$' => {
                self.colour = MatrixColor::Blue;
                self.rainbow = false;
            }
            '%' => {
                self.colour = MatrixColor::Magenta;
                self.rainbow = false;
            }
            'r' => {
                self.rainbow = true;
            }
            '^' => {
                self.colour = MatrixColor::Cyan;
                self.rainbow = false;
            }
            '&' => {
                self.colour = MatrixColor::White;
                self.rainbow = false;
            }
            'p' | 'P' => self.pause = !self.pause,
            '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' | '0' => {
                self.update = keypress as usize - 48
            }
            _ => {}
        }
        false
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
    fn update_delay_range() {
        assert_eq!(validate_update("0"), Ok(0));
        assert_eq!(validate_update("10"), Ok(10));
        assert!(validate_update("11").is_err());
        assert!(validate_update("-1").is_err());
        assert!(validate_update("abc").is_err());
    }

    #[test]
    fn quit_keys() {
        assert!(config().handle_keypress('q'));
        assert!(!config().handle_keypress('z'));

        let mut screensaver = config();
        screensaver.screensaver = true;
        assert!(screensaver.handle_keypress('z'));
    }

    #[test]
    fn option_keys() {
        let mut c = config();
        c.handle_keypress('B');
        assert_eq!(c.bold, 2);
        c.handle_keypress('n');
        assert_eq!(c.bold, 0);

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
        c.handle_keypress('!');
        assert!(!c.rainbow);
        assert_eq!(c.colour, MatrixColor::Red);
    }
}
