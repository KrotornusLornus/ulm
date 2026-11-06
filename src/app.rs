use crate::auth::{self, AuthResult, PamSession};

pub const MAX_ATTEMPTS: u8 = 3;
pub const INPUT_WIDTH: usize = 22;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    User,
    Pass,
}

pub enum Outcome {
    Ok { user: String, session: PamSession },
    Fail(String),
    Abort,
}

pub struct App {
    pub user: String,
    pub pass: String,
    pub focus: Focus,
    pub attempts: u8,
    pub error: Option<String>,
    pub granted: bool,
    pub session: Option<PamSession>,
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::User => Focus::Pass,
            Focus::Pass => Focus::User,
        };
        self.error = None;
    }

    pub fn backspace(&mut self) {
        self.error = None;
        match self.focus {
            Focus::User => {
                self.user.pop();
            }
            Focus::Pass => {
                self.pass.pop();
            }
        }
    }

    pub fn push_char(&mut self, c: char) {
        self.error = None;
        match self.focus {
            Focus::User if self.user.chars().count() < 32 => self.user.push(c),
            Focus::Pass if self.pass.chars().count() < 64 => self.pass.push(c),
            _ => {}
        }
    }

    pub fn submit(&mut self) -> Option<Outcome> {
        if self.user.trim().is_empty() {
            self.error = Some("EMPLOYEE ID REQUIRED".to_string());
            return None;
        }
        if self.pass.is_empty() {
            self.error = Some("PASSCODE REQUIRED".to_string());
            return None;
        }

        match auth::authenticate(self.user.trim(), &self.pass) {
            AuthResult::Success(session) => {
                self.error = None;
                self.session = Some(session);
                self.granted = true;
                None
            }
            AuthResult::Invalid => {
                self.attempts += 1;
                self.pass.clear();
                if self.attempts >= MAX_ATTEMPTS {
                    return Some(Outcome::Fail(self.user.trim().to_string()));
                }
                self.error = Some(format!(
                    "ACCESS DENIED - ATTEMPT {}/{}",
                    self.attempts, MAX_ATTEMPTS
                ));
                None
            }
            AuthResult::Unavailable => {
                self.error = Some("AUTH SERVICE UNAVAILABLE".to_string());
                None
            }
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self {
            user: String::new(),
            pass: String::new(),
            focus: Focus::User,
            attempts: 0,
            error: None,
            granted: false,
            session: None,
        }
    }
}
