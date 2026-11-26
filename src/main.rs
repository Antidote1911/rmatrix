use std::io;
#[cfg(unix)]
use std::sync::Arc;
#[cfg(unix)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
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

/// How often exit signals are checked while the animation is paused.
const PAUSE_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Handle a terminal event. Returns true if the program should exit.
fn handle_event(
    event: Event,
    config: &mut Config,
    terminal: &mut Terminal,
    matrix: &mut Matrix,
) -> io::Result<bool> {
    match event {
        Event::Resize(_, _) => {
            // Fit the matrix to the new terminal size, keeping the streams.
            terminal.resize_window()?;
            matrix.resize_to_terminal();
            // Redraw now: when paused, nothing else would.
            matrix.draw(terminal, config)?;
        }
        // Only key presses count: Windows also reports key releases.
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            if config.screensaver
                || key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('c' | 'C'))
            {
                return Ok(true);
            }

            match key.code {
                KeyCode::Char(c) => return Ok(config.handle_keypress(c)),
                // The arrows work on every keyboard layout, unlike the digits.
                KeyCode::Up => config.speed_up(),
                KeyCode::Down => config.slow_down(),
                _ => {}
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
    let mut next_frame = Instant::now();
    while !exit_signal.received() {
        // Wait for the next frame, handling terminal events as soon as they arrive.
        let timeout = next_frame.saturating_duration_since(Instant::now());
        if let Some(event) = terminal.get_event(timeout)? {
            if handle_event(event, &mut config, &mut terminal, &mut matrix)? {
                break;
            }
            continue;
        }

        if config.pause {
            // Nothing changes on screen: just keep checking for events and signals.
            next_frame = Instant::now() + PAUSE_POLL_INTERVAL;
        } else {
            next_frame = Instant::now() + config.frame_delay();
            matrix.arrange(&config);
            matrix.draw(&mut terminal, &config)?;
        }
    }

    terminal.finish()
}
