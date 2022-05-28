use pancurses::*;
use std::ops::RangeInclusive;

use clap::{AppSettings, ArgEnum, Parser};

const AUTHOR: &str = "
Author : Fabrice Corraire <antidote1911@gmail.com>
Github : https://github.com/Antidote1911/
";

#[derive(Parser)]
#[clap(global_setting(AppSettings::DeriveDisplayOrder))]
#[clap(about, author=AUTHOR, version)]
/// The struct for handling command line arguments
struct Opt {
    #[clap(short = 'b', parse(from_occurrences))]
    /// Bold characters on
    bold: isize,

    #[clap(short = 'l', long = "console")]
    /// Linux mode (use matrix console font)
    console: bool,

    #[clap(short = 'o', long = "oldstyle")]
    /// Use old-style scrolling
    oldstyle: bool,

    #[clap(short = 's', long = "screensaver")]
    /// "Screensaver" mode, exits on first keystroke
    screensaver: bool,

    #[clap(short = 'x', long = "xwindow")]
    /// X window mode, use if your xterm is using mtx.pcf
    xwindow: bool,

    #[clap(short = 'u', long = "update", default_value = "4", parse(try_from_str=update_in_range))]
    /// Screen update delay
    update: usize,

    #[clap(
        short = 'C',
        long = "colour",
        arg_enum,
        name = "COLOR",
        default_value = "green"
    )]
    colour: Colors,

    #[clap(short = 'c', long = "characters", arg_enum, default_value = "classic")]
    characters: Characters,

    #[clap(short = 'r', long = "rainbow")]
    /// Rainbow mode
    rainbow: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
pub enum Characters {
    Classic,
    Jap,
    Digits
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
pub enum Colors {
    Green,
    Red,
    Blue,
    White,
    Yellow,
    Cyan,
    Magenta,
    Black
}

const UPDATE_RANGE: RangeInclusive<usize> = 0..=9;
fn update_in_range(s: &str) -> Result<usize, String> {
    let update: usize = s
        .parse()
        .map_err(|_| format!("`{}` isn't a valid speed update", s))?;
    if UPDATE_RANGE.contains(&update) {
        Ok(update)
    } else {
        Err(format!(
            "Speed update not in range {}-{}",
            UPDATE_RANGE.start(),
            UPDATE_RANGE.end()
        ))
    }
}

/// The global state object
pub struct Config {
    pub bold: isize,
    pub characters: Characters,
    pub console: bool,
    pub oldstyle: bool,
    pub screensaver: bool,
    pub xwindow: bool,
    pub update: usize,
    pub colour: i16,
    pub rainbow: bool,
    pub pause: bool,
}

impl Config {
    /// Get the new config object based on command line arguments
    pub fn default() -> Self {
        let opt = Opt::parse();

        let colour = match opt.colour {
            Colors::Green => COLOR_GREEN,
            Colors::Red => COLOR_RED,
            Colors::Blue => COLOR_BLUE,
            Colors::White => COLOR_WHITE,
            Colors::Yellow => COLOR_YELLOW,
            Colors::Cyan => COLOR_CYAN,
            Colors::Magenta => COLOR_MAGENTA,
            Colors::Black => COLOR_BLACK,
        };

        Config {
            bold: opt.bold,
            characters: opt.characters,
            console: opt.console,
            oldstyle: opt.oldstyle,
            screensaver: opt.screensaver,
            xwindow: opt.xwindow,
            update: opt.update,
            rainbow: opt.rainbow,
            colour,
            pause: false,
        }
    }
    /// Update the config based on any keypresses
    pub fn handle_keypress(&mut self, keypress: char) {
        // Exit if in screensaver mode
        if self.screensaver {
            super::finish();
        }

        match keypress {
            '\u{1b}' | 'q' => super::finish(),
            'b' => self.bold = 1,
            'B' => self.bold = 2,
            'n' => self.bold = 0,
            'a' => {
                self.colour = COLOR_RED;
                self.rainbow = false;
            }
            'z' => {
                self.colour = COLOR_GREEN;
                self.rainbow = false;
            }
            'e' => {
                self.colour = COLOR_YELLOW;
                self.rainbow = false;
            }
            'r' => {
                self.colour = COLOR_BLUE;
                self.rainbow = false;
            }
            't' => {
                self.colour = COLOR_MAGENTA;
                self.rainbow = false;
            }
            'i' => {
                self.rainbow = true;
            }
            'u' => {
                self.colour = COLOR_CYAN;
                self.rainbow = false;
            }
            'y' => {
                self.colour = COLOR_WHITE;
                self.rainbow = false;
            }
            'j' => {
                self.characters = Characters::Jap;
            }
            'c' => {
                self.characters = Characters::Classic;
            }

            'p' | 'P' => self.pause = !self.pause,
            '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' | '0' => {
                self.update = keypress as usize - 48 // Sneaky way to avoid parsing
            }
            _ => {}
        }
    }
}
