//! Simple key-to-action input handling for observation-only TUI.
//!
//! All keys map directly to actions - no modal input or prefix keys needed
//! since the TUI is read-only and doesn't forward input to agents.

use crossterm::event::{KeyCode, KeyEvent};

// =============================================================================
// NEW API: Simple key-to-action mapping (Task 10)
// =============================================================================

/// Actions that can be triggered by key presses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Exit the TUI
    Quit,
    /// Navigate to next iteration
    NextIteration,
    /// Navigate to previous iteration
    PrevIteration,
    /// Scroll down one line
    ScrollDown,
    /// Scroll up one line
    ScrollUp,
    /// Jump to top of content
    ScrollTop,
    /// Jump to bottom of content
    ScrollBottom,
    /// Enter search mode
    StartSearch,
    /// Jump to next search match
    SearchNext,
    /// Jump to previous search match
    SearchPrev,
    /// Show help overlay
    ShowHelp,
    /// Dismiss help overlay or cancel search
    DismissHelp,
    /// Open guidance input for next iteration
    GuidanceNext,
    /// Open guidance input for current iteration (urgent)
    GuidanceNow,
    /// Key not mapped to any action
    None,
}

/// Maps a key event to its corresponding action.
///
/// Supports both arrow keys and vim-style navigation:
/// - `q`: Quit
/// - `←`/`h`: Previous iteration
/// - `→`/`l`: Next iteration
/// - `↓`/`j`: Scroll down
/// - `↑`/`k`: Scroll up
/// - `g`: Scroll to top
/// - `G`: Scroll to bottom
/// - `/`: Start search
/// - `n`: Next search match
/// - `N`: Previous search match
/// - `?`: Show help
/// - `Esc`: Dismiss help/cancel search
pub fn map_key(key: KeyEvent) -> Action {
    match key.code {
        // Quit
        KeyCode::Char('q') => Action::Quit,

        // Iteration navigation
        KeyCode::Right | KeyCode::Char('l') => Action::NextIteration,
        KeyCode::Left | KeyCode::Char('h') => Action::PrevIteration,

        // Scroll
        KeyCode::Down | KeyCode::Char('j') => Action::ScrollDown,
        KeyCode::Up | KeyCode::Char('k') => Action::ScrollUp,
        KeyCode::Char('g') => Action::ScrollTop,
        KeyCode::Char('G') => Action::ScrollBottom,

        // Search
        KeyCode::Char('/') => Action::StartSearch,
        KeyCode::Char('n') => Action::SearchNext,
        KeyCode::Char('N') => Action::SearchPrev,

        // Guidance
        KeyCode::Char(':') => Action::GuidanceNext,
        KeyCode::Char('!') => Action::GuidanceNow,

        // Help
        KeyCode::Char('?') => Action::ShowHelp,
        KeyCode::Esc => Action::DismissHelp,

        // Unknown
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    // AC1: q Quits
    #[test]
    fn q_returns_quit() {
        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(map_key(key), Action::Quit);
    }

    // AC2: Right Arrow Next Iteration
    #[test]
    fn right_arrow_returns_next_iteration() {
        let key = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(map_key(key), Action::NextIteration);
    }

    // AC3: Left Arrow Prev Iteration
    #[test]
    fn left_arrow_returns_prev_iteration() {
        let key = KeyEvent::new(KeyCode::Left, KeyModifiers::NONE);
        assert_eq!(map_key(key), Action::PrevIteration);
    }

    // AC4: j Scroll Down
    #[test]
    fn j_returns_scroll_down() {
        let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(map_key(key), Action::ScrollDown);
    }

    // AC5: k Scroll Up
    #[test]
    fn k_returns_scroll_up() {
        let key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
        assert_eq!(map_key(key), Action::ScrollUp);
    }

    // AC6: g Scroll Top
    #[test]
    fn g_returns_scroll_top() {
        let key = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        assert_eq!(map_key(key), Action::ScrollTop);
    }

    // AC7: G Scroll Bottom
    #[test]
    fn shift_g_returns_scroll_bottom() {
        let key = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT);
        assert_eq!(map_key(key), Action::ScrollBottom);
    }

    // AC8: / Start Search
    #[test]
    fn slash_returns_start_search() {
        let key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
        assert_eq!(map_key(key), Action::StartSearch);
    }

    // AC9: n Search Next
    #[test]
    fn n_returns_search_next() {
        let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE);
        assert_eq!(map_key(key), Action::SearchNext);
    }

    // AC10: N Search Prev
    #[test]
    fn shift_n_returns_search_prev() {
        let key = KeyEvent::new(KeyCode::Char('N'), KeyModifiers::SHIFT);
        assert_eq!(map_key(key), Action::SearchPrev);
    }

    // AC11: ? Show Help
    #[test]
    fn question_mark_returns_show_help() {
        let key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT);
        assert_eq!(map_key(key), Action::ShowHelp);
    }

    // AC12: Esc Dismiss Help
    #[test]
    fn esc_returns_dismiss_help() {
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(map_key(key), Action::DismissHelp);
    }

    // AC13: Vim l Next Iteration
    #[test]
    fn l_returns_next_iteration() {
        let key = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE);
        assert_eq!(map_key(key), Action::NextIteration);
    }

    // AC14: Vim h Prev Iteration
    #[test]
    fn h_returns_prev_iteration() {
        let key = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
        assert_eq!(map_key(key), Action::PrevIteration);
    }

    // AC15: : Opens Guidance Next
    #[test]
    fn colon_returns_guidance_next() {
        let key = KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE);
        assert_eq!(map_key(key), Action::GuidanceNext);
    }

    // AC16: ! Opens Guidance Now
    #[test]
    fn bang_returns_guidance_now() {
        let key = KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE);
        assert_eq!(map_key(key), Action::GuidanceNow);
    }

    // AC17: Unknown Key Returns None
    #[test]
    fn unknown_key_returns_none() {
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        assert_eq!(map_key(key), Action::None);
    }

    // Additional tests for arrow key alternatives
    #[test]
    fn down_arrow_returns_scroll_down() {
        let key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(map_key(key), Action::ScrollDown);
    }

    #[test]
    fn up_arrow_returns_scroll_up() {
        let key = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(map_key(key), Action::ScrollUp);
    }
}
