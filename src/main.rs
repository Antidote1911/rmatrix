use std::io;
#[cfg(unix)]
use std::sync::Arc;
#[cfg(unix)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyModifiers};
#[cfg(unix)]
use signal_hook::consts::signal::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};
#[cfg(unix)]
use signal_hook::flag;

use r_matrix::Matrix;
use r_matrix::Terminal;
use r_matrix::config::Config;

/// Exit signals sent from outside the terminal (e.g. `kill`).
/// Resizes and Ctrl-C are received as terminal events instead.
#[cfg(unix)]
struct ExitSignal(Arc<AtomicBool>);

#[cfg(unix)]
impl ExitSignal {
    fn new() -> io::Result<Self> {
        let exit_signal = Arc::new(AtomicBool::new(false));
        for signal in [SIGHUP, SIGINT, SIGTERM, SIGQUIT] {
            flag::register(signal, Arc::clone(&exit_signal))?;
        }
        Ok(ExitSignal(exit_signal))
    }

    fn received(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[cfg(not(unix))]
struct ExitSignal;

#[cfg(not(unix))]
impl ExitSignal {
    fn new() -> io::Result<Self> {
        Ok(ExitSignal)
    }

    fn received(&self) -> bool {
        false
    }
}

/// Handle a terminal event. Returns true if the program should exit.
fn handle_event(
    event: Event,
    config: &mut Config,
    terminal: &mut Terminal,
    matrix: &mut Matrix,
) -> io::Result<bool> {
    match event {
        Event::Resize(_, _) => {
            // Make a new matrix for the new terminal size.
            terminal.resize_window()?;
            *matrix = Matrix::from_terminal_size();
        }
        Event::Key(key) => {
            if config.screensaver
                || key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('c' | 'C'))
            {
                return Ok(true);
            }

            if let KeyCode::Char(c) = key.code {
                return Ok(config.handle_keypress(c));
            }
        }
        _ => {}
    }
    Ok(false)
}

fn main() -> io::Result<()> {
    // Get command line args
    let mut config = Config::from_args();

    // Save the terminal state and start up the terminal
    let mut terminal = Terminal::new()?;

    let exit_signal = ExitSignal::new()?;

    // Create the board
    let mut matrix = Matrix::from_terminal_size();

    // Main event loop
    'main: while !exit_signal.received() {
        // Handle every pending terminal event.
        while let Some(event) = terminal.get_event(Duration::ZERO)? {
            if handle_event(event, &mut config, &mut terminal, &mut matrix)? {
                break 'main;
            }
        }

        // Update and redraw the board (frozen while paused).
        if !config.pause {
            matrix.arrange(&config);
        }
        matrix.draw(&mut terminal, &config)?;
    }

    terminal.finish()
}
