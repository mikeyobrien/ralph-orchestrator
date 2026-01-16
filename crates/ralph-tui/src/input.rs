//! Input routing for TUI prefix commands.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Input routing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    AwaitingCommand,
    Scroll,
    Search,
}

/// Prefix commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Quit,
    Help,
    Pause,
    Skip,
    Abort,
    EnterScroll,
    Unknown,
}

/// Result of routing a key event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteResult {
    Forward(KeyEvent),
    Command(Command),
    ScrollKey(KeyEvent),
    ExitScroll,
    EnterSearch { forward: bool },
    SearchInput(KeyEvent),
    ExecuteSearch,
    CancelSearch,
    Consumed,
}

/// Routes input between normal mode and command mode.
pub struct InputRouter {
    mode: InputMode,
    prefix_key: KeyCode,
    prefix_modifiers: KeyModifiers,
}

impl InputRouter {
    pub fn new() -> Self {
        Self {
            mode: InputMode::Normal,
            prefix_key: KeyCode::Char('a'),
            prefix_modifiers: KeyModifiers::CONTROL,
        }
    }

    /// Creates a new `InputRouter` with a custom prefix key.
    pub fn with_prefix(prefix_key: KeyCode, prefix_modifiers: KeyModifiers) -> Self {
        Self {
            mode: InputMode::Normal,
            prefix_key,
            prefix_modifiers,
        }
    }

    /// Routes a key event based on current mode.
    pub fn route_key(&mut self, key: KeyEvent) -> RouteResult {
        match self.mode {
            InputMode::Normal => {
                if self.is_prefix(key) {
                    self.mode = InputMode::AwaitingCommand;
                    RouteResult::Consumed
                } else {
                    RouteResult::Forward(key)
                }
            }
            InputMode::AwaitingCommand => {
                self.mode = InputMode::Normal;
                if let Some(c) = extract_char(key) {
                    RouteResult::Command(match c {
                        'q' => Command::Quit,
                        '?' => Command::Help,
                        'p' => Command::Pause,
                        'n' => Command::Skip,
                        'a' => Command::Abort,
                        '[' => Command::EnterScroll,
                        _ => Command::Unknown,
                    })
                } else {
                    RouteResult::Consumed
                }
            }
            InputMode::Scroll => {
                // Handle search initiation
                if matches!(key.code, KeyCode::Char('/')) {
                    self.mode = InputMode::Search;
                    return RouteResult::EnterSearch { forward: true };
                }
                if matches!(key.code, KeyCode::Char('?')) {
                    self.mode = InputMode::Search;
                    return RouteResult::EnterSearch { forward: false };
                }
                // Handle search navigation
                if matches!(key.code, KeyCode::Char('n')) {
                    return RouteResult::ScrollKey(key);
                }
                if matches!(key.code, KeyCode::Char('N')) {
                    return RouteResult::ScrollKey(key);
                }
                // Exit scroll mode on q, Escape, or Enter
                if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter) {
                    self.mode = InputMode::Normal;
                    RouteResult::ExitScroll
                } else {
                    RouteResult::ScrollKey(key)
                }
            }
            InputMode::Search => match key.code {
                KeyCode::Enter => {
                    self.mode = InputMode::Scroll;
                    RouteResult::ExecuteSearch
                }
                KeyCode::Esc => {
                    self.mode = InputMode::Scroll;
                    RouteResult::CancelSearch
                }
                _ => RouteResult::SearchInput(key),
            },
        }
    }

    /// Enters scroll mode.
    pub fn enter_scroll_mode(&mut self) {
        self.mode = InputMode::Scroll;
    }

    /// Exits scroll mode back to normal.
    pub fn exit_scroll_mode(&mut self) {
        self.mode = InputMode::Normal;
    }

    fn is_prefix(&self, key: KeyEvent) -> bool {
        key.code == self.prefix_key && key.modifiers.contains(self.prefix_modifiers)
    }
}

impl Default for InputRouter {
    fn default() -> Self {
        Self::new()
    }
}

fn extract_char(key: KeyEvent) -> Option<char> {
    match key.code {
        KeyCode::Char(c) => Some(c),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_mode_forwards_regular_keys() {
        let mut router = InputRouter::new();
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        assert_eq!(router.route_key(key), RouteResult::Forward(key));
    }

    #[test]
    fn ctrl_a_switches_to_awaiting_command() {
        let mut router = InputRouter::new();
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert_eq!(router.route_key(key), RouteResult::Consumed);
    }

    #[test]
    fn next_key_after_ctrl_a_returns_command() {
        let mut router = InputRouter::new();
        let prefix = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        router.route_key(prefix);

        let cmd = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(router.route_key(cmd), RouteResult::Command(Command::Quit));
    }

    #[test]
    fn state_resets_to_normal_after_command() {
        let mut router = InputRouter::new();
        let prefix = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        router.route_key(prefix);

        let cmd = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        router.route_key(cmd);

        let next = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        assert_eq!(router.route_key(next), RouteResult::Forward(next));
    }

    #[test]
    fn quit_command_returns_q() {
        let mut router = InputRouter::new();
        let prefix = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        router.route_key(prefix);

        let cmd = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(router.route_key(cmd), RouteResult::Command(Command::Quit));
    }

    #[test]
    fn help_command_returns_question_mark() {
        let mut router = InputRouter::new();
        let prefix = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        router.route_key(prefix);

        let cmd = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT);
        assert_eq!(router.route_key(cmd), RouteResult::Command(Command::Help));
    }

    #[test]
    fn unknown_command_returns_unknown() {
        let mut router = InputRouter::new();
        let prefix = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        router.route_key(prefix);

        let cmd = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        assert_eq!(
            router.route_key(cmd),
            RouteResult::Command(Command::Unknown)
        );
    }

    #[test]
    fn pause_command_returns_p() {
        let mut router = InputRouter::new();
        let prefix = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        router.route_key(prefix);

        let cmd = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE);
        assert_eq!(router.route_key(cmd), RouteResult::Command(Command::Pause));
    }

    #[test]
    fn skip_command_returns_n() {
        let mut router = InputRouter::new();
        let prefix = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        router.route_key(prefix);

        let cmd = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE);
        assert_eq!(router.route_key(cmd), RouteResult::Command(Command::Skip));
    }

    #[test]
    fn abort_command_returns_a() {
        let mut router = InputRouter::new();
        let prefix = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        router.route_key(prefix);

        let cmd = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(router.route_key(cmd), RouteResult::Command(Command::Abort));
    }

    #[test]
    fn enter_scroll_command_returns_bracket() {
        let mut router = InputRouter::new();
        let prefix = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        router.route_key(prefix);

        let cmd = KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE);
        assert_eq!(
            router.route_key(cmd),
            RouteResult::Command(Command::EnterScroll)
        );
    }

    #[test]
    fn scroll_mode_routes_navigation_keys() {
        let mut router = InputRouter::new();
        router.enter_scroll_mode();

        let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(router.route_key(key), RouteResult::ScrollKey(key));
    }

    #[test]
    fn scroll_mode_exits_on_q() {
        let mut router = InputRouter::new();
        router.enter_scroll_mode();

        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(router.route_key(key), RouteResult::ExitScroll);
    }

    #[test]
    fn scroll_mode_exits_on_escape() {
        let mut router = InputRouter::new();
        router.enter_scroll_mode();

        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(router.route_key(key), RouteResult::ExitScroll);
    }

    #[test]
    fn scroll_mode_exits_on_enter() {
        let mut router = InputRouter::new();
        router.enter_scroll_mode();

        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(router.route_key(key), RouteResult::ExitScroll);
    }

    #[test]
    fn scroll_mode_enters_forward_search() {
        let mut router = InputRouter::new();
        router.enter_scroll_mode();

        let key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
        assert_eq!(
            router.route_key(key),
            RouteResult::EnterSearch { forward: true }
        );
    }

    #[test]
    fn scroll_mode_enters_backward_search() {
        let mut router = InputRouter::new();
        router.enter_scroll_mode();

        let key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
        assert_eq!(
            router.route_key(key),
            RouteResult::EnterSearch { forward: false }
        );
    }

    #[test]
    fn search_mode_captures_input() {
        let mut router = InputRouter::new();
        router.enter_scroll_mode();
        router.route_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));

        let key = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE);
        assert_eq!(router.route_key(key), RouteResult::SearchInput(key));
    }

    #[test]
    fn search_mode_executes_on_enter() {
        let mut router = InputRouter::new();
        router.enter_scroll_mode();
        router.route_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));

        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(router.route_key(key), RouteResult::ExecuteSearch);
    }

    #[test]
    fn search_mode_cancels_on_escape() {
        let mut router = InputRouter::new();
        router.enter_scroll_mode();
        router.route_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));

        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(router.route_key(key), RouteResult::CancelSearch);
    }

    #[test]
    fn scroll_mode_handles_n_for_next_match() {
        let mut router = InputRouter::new();
        router.enter_scroll_mode();

        let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE);
        assert_eq!(router.route_key(key), RouteResult::ScrollKey(key));
    }

    #[test]
    fn scroll_mode_handles_shift_n_for_prev_match() {
        let mut router = InputRouter::new();
        router.enter_scroll_mode();

        let key = KeyEvent::new(KeyCode::Char('N'), KeyModifiers::SHIFT);
        assert_eq!(router.route_key(key), RouteResult::ScrollKey(key));
    }

    #[test]
    fn custom_prefix_ctrl_b_works() {
        let mut router = InputRouter::with_prefix(KeyCode::Char('b'), KeyModifiers::CONTROL);

        // Ctrl+B should trigger command mode
        let prefix = KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL);
        assert_eq!(router.route_key(prefix), RouteResult::Consumed);

        // Next key should be interpreted as command
        let cmd = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(router.route_key(cmd), RouteResult::Command(Command::Quit));
    }

    #[test]
    fn custom_prefix_ctrl_b_ignores_ctrl_a() {
        let mut router = InputRouter::with_prefix(KeyCode::Char('b'), KeyModifiers::CONTROL);

        // Ctrl+A should be forwarded, not consumed
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert_eq!(router.route_key(key), RouteResult::Forward(key));
    }
}
