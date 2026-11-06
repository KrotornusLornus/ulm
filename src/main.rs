mod app;
mod auth;
mod login;
mod theme;
mod ui;

use std::io;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use ratatui::DefaultTerminal;
use ratatui::crossterm::cursor::{Hide, Show};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::crossterm::execute;

use app::{App, MAX_ATTEMPTS, Outcome};

fn main() -> ExitCode {
    let check = std::env::args().any(|arg| arg == "--check");
    let mut terminal = ratatui::init();

    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(std::io::stdout(), Show);
        previous_hook(info);
    }));
    let _ = execute!(std::io::stdout(), Hide);

    let outcome = run(&mut terminal);

    let _ = execute!(std::io::stdout(), Show);
    ratatui::restore();

    match outcome {
        Ok(Outcome::Ok { user, mut session }) => {
            if check {
                println!("AUTH_OK user={user}");
                return ExitCode::SUCCESS;
            }

            if let Err(err) = session.open() {
                eprintln!("ulm: {err}");
                return ExitCode::from(2);
            }

            match login::exec_shell(&user) {
                Ok(()) => unreachable!(),
                Err(err) => {
                    eprintln!("ulm: {err}");
                    ExitCode::from(2)
                }
            }
        }
        Ok(Outcome::Fail(user)) => {
            println!("AUTH_FAIL user={user} attempts={MAX_ATTEMPTS}");
            ExitCode::from(1)
        }
        Ok(Outcome::Abort) => {
            println!("AUTH_ABORTED");
            ExitCode::from(1)
        }
        Err(err) => {
            eprintln!("ulm: {err}");
            ExitCode::from(2)
        }
    }
}

fn run(terminal: &mut DefaultTerminal) -> io::Result<Outcome> {
    let mut app = App::new();
    let mut cursor_on = true;
    let mut last_blink = Instant::now();

    loop {
        if last_blink.elapsed() >= Duration::from_millis(500) {
            cursor_on = !cursor_on;
            last_blink = Instant::now();
        }

        terminal.draw(|frame| ui::draw(frame, &app, cursor_on))?;

        if app.granted {
            std::thread::sleep(Duration::from_millis(700));
            let user = app.user.trim().to_string();
            let session = app.session.take().expect("authenticated session");
            return Ok(Outcome::Ok { user, session });
        }

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    cursor_on = true;
                    last_blink = Instant::now();
                    if let Some(outcome) = handle_key(&mut app, key.code, key.modifiers) {
                        return Ok(outcome);
                    }
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }
}

fn handle_key(app: &mut App, code: KeyCode, mods: KeyModifiers) -> Option<Outcome> {
    match code {
        KeyCode::Esc => Some(Outcome::Abort),
        KeyCode::Char('c') if mods.contains(KeyModifiers::CONTROL) => Some(Outcome::Abort),
        KeyCode::Tab | KeyCode::BackTab | KeyCode::Up | KeyCode::Down => {
            app.toggle_focus();
            None
        }
        KeyCode::Enter => app.submit(),
        KeyCode::Backspace => {
            app.backspace();
            None
        }
        KeyCode::Char(c) if !mods.contains(KeyModifiers::CONTROL) => {
            app.push_char(c);
            None
        }
        _ => None,
    }
}
