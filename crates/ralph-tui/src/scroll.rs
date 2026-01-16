//! Scroll mode management for terminal output.

use crossterm::event::{KeyCode, KeyEvent};

/// Search direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchDirection {
    Forward,
    Backward,
}

/// Search state.
#[derive(Debug, Clone)]
pub struct SearchState {
    pub query: String,
    pub direction: SearchDirection,
    pub matches: Vec<usize>,  // Line numbers with matches
    pub current_match: usize, // Index into matches vec
}

/// Manages scroll state for terminal output.
#[derive(Debug, Clone)]
pub struct ScrollManager {
    /// Current scroll offset (0 = bottom/live output).
    offset: usize,
    /// Total lines available in terminal history.
    total_lines: usize,
    /// Viewport height (visible lines).
    viewport_height: usize,
    /// Active search state.
    search: Option<SearchState>,
}

impl ScrollManager {
    /// Creates a new scroll manager.
    pub fn new() -> Self {
        Self {
            offset: 0,
            total_lines: 0,
            viewport_height: 24,
            search: None,
        }
    }

    /// Updates total lines and viewport height.
    pub fn update_dimensions(&mut self, total_lines: usize, viewport_height: usize) {
        self.total_lines = total_lines;
        self.viewport_height = viewport_height;
        self.clamp_offset();
    }

    /// Returns current scroll offset.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Handles navigation key in scroll mode.
    pub fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.scroll_down(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll_up(1),
            KeyCode::PageDown => self.scroll_down(self.viewport_height),
            KeyCode::PageUp => self.scroll_up(self.viewport_height),
            KeyCode::Char('g') => self.jump_to_top(),
            KeyCode::Char('G') => self.jump_to_bottom(),
            _ => {}
        }
    }

    /// Scrolls down by n lines (toward live output).
    pub fn scroll_down(&mut self, n: usize) {
        self.offset = self.offset.saturating_sub(n);
    }

    /// Scrolls up by n lines (into history).
    pub fn scroll_up(&mut self, n: usize) {
        self.offset = (self.offset + n).min(self.max_offset());
    }

    /// Jumps to top of history.
    fn jump_to_top(&mut self) {
        self.offset = self.max_offset();
    }

    /// Jumps to bottom (live output).
    fn jump_to_bottom(&mut self) {
        self.offset = 0;
    }

    /// Returns maximum valid offset.
    fn max_offset(&self) -> usize {
        self.total_lines.saturating_sub(self.viewport_height)
    }

    /// Clamps offset to valid range.
    fn clamp_offset(&mut self) {
        self.offset = self.offset.min(self.max_offset());
    }

    /// Resets to live output (bottom).
    pub fn reset(&mut self) {
        self.offset = 0;
    }

    /// Starts a search with the given query and direction.
    pub fn start_search(&mut self, query: String, direction: SearchDirection, lines: &[String]) {
        let matches = Self::find_matches(&query, lines);
        let current_match = if matches.is_empty() {
            0
        } else {
            match direction {
                SearchDirection::Forward => 0,
                SearchDirection::Backward => matches.len().saturating_sub(1),
            }
        };

        self.search = Some(SearchState {
            query,
            direction,
            matches: matches.clone(),
            current_match,
        });

        if !matches.is_empty() {
            self.jump_to_line(matches[current_match]);
        }
    }

    /// Finds all line numbers containing the query (case-insensitive).
    fn find_matches(query: &str, lines: &[String]) -> Vec<usize> {
        let query_lower = query.to_lowercase();
        lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.to_lowercase().contains(&query_lower))
            .map(|(i, _)| i)
            .collect()
    }

    /// Jumps to next match.
    pub fn next_match(&mut self) {
        if let Some(ref mut search) = self.search
            && !search.matches.is_empty()
        {
            search.current_match = (search.current_match + 1) % search.matches.len();
            let line = search.matches[search.current_match];
            let _ = search; // End the mutable borrow
            self.jump_to_line(line);
        }
    }

    /// Jumps to previous match.
    pub fn prev_match(&mut self) {
        if let Some(ref mut search) = self.search
            && !search.matches.is_empty()
        {
            search.current_match = if search.current_match == 0 {
                search.matches.len() - 1
            } else {
                search.current_match - 1
            };
            let line = search.matches[search.current_match];
            let _ = search; // End the mutable borrow
            self.jump_to_line(line);
        }
    }

    /// Jumps to a specific line number.
    fn jump_to_line(&mut self, line: usize) {
        // Calculate offset to center the line in viewport
        let target_offset = self
            .total_lines
            .saturating_sub(line + self.viewport_height / 2);
        self.offset = target_offset.min(self.max_offset());
    }

    /// Returns current search state.
    pub fn search_state(&self) -> Option<&SearchState> {
        self.search.as_ref()
    }

    /// Clears search state.
    pub fn clear_search(&mut self) {
        self.search = None;
    }
}

impl Default for ScrollManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    #[test]
    fn new_scroll_manager_starts_at_bottom() {
        let sm = ScrollManager::new();
        assert_eq!(sm.offset(), 0);
    }

    #[test]
    fn scroll_up_increases_offset() {
        let mut sm = ScrollManager::new();
        sm.update_dimensions(100, 24);
        sm.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        assert_eq!(sm.offset(), 1);
    }

    #[test]
    fn scroll_down_decreases_offset() {
        let mut sm = ScrollManager::new();
        sm.update_dimensions(100, 24);
        sm.offset = 10;
        sm.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(sm.offset(), 9);
    }

    #[test]
    fn scroll_down_stops_at_zero() {
        let mut sm = ScrollManager::new();
        sm.update_dimensions(100, 24);
        sm.offset = 0;
        sm.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(sm.offset(), 0);
    }

    #[test]
    fn scroll_up_stops_at_max() {
        let mut sm = ScrollManager::new();
        sm.update_dimensions(100, 24);
        for _ in 0..200 {
            sm.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        }
        assert_eq!(sm.offset(), 76); // 100 - 24
    }

    #[test]
    fn page_down_scrolls_viewport_height() {
        let mut sm = ScrollManager::new();
        sm.update_dimensions(100, 24);
        sm.offset = 50;
        sm.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        assert_eq!(sm.offset(), 26); // 50 - 24
    }

    #[test]
    fn page_up_scrolls_viewport_height() {
        let mut sm = ScrollManager::new();
        sm.update_dimensions(100, 24);
        sm.offset = 10;
        sm.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
        assert_eq!(sm.offset(), 34); // 10 + 24
    }

    #[test]
    fn g_jumps_to_top() {
        let mut sm = ScrollManager::new();
        sm.update_dimensions(100, 24);
        sm.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        assert_eq!(sm.offset(), 76); // max offset
    }

    #[test]
    fn capital_g_jumps_to_bottom() {
        let mut sm = ScrollManager::new();
        sm.update_dimensions(100, 24);
        sm.offset = 50;
        sm.handle_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE));
        assert_eq!(sm.offset(), 0);
    }

    #[test]
    fn reset_returns_to_bottom() {
        let mut sm = ScrollManager::new();
        sm.update_dimensions(100, 24);
        sm.offset = 50;
        sm.reset();
        assert_eq!(sm.offset(), 0);
    }

    #[test]
    fn arrow_keys_work_like_jk() {
        let mut sm = ScrollManager::new();
        sm.update_dimensions(100, 24);
        sm.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(sm.offset(), 1);
        sm.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(sm.offset(), 0);
    }

    #[test]
    fn search_finds_matches() {
        let mut sm = ScrollManager::new();
        sm.update_dimensions(100, 24);
        let lines = vec![
            "hello world".to_string(),
            "error: something failed".to_string(),
            "info: all good".to_string(),
            "error: another issue".to_string(),
        ];
        sm.start_search("error".to_string(), super::SearchDirection::Forward, &lines);
        let search = sm.search_state().unwrap();
        assert_eq!(search.matches.len(), 2);
        assert_eq!(search.matches[0], 1);
        assert_eq!(search.matches[1], 3);
    }

    #[test]
    fn search_case_insensitive() {
        let mut sm = ScrollManager::new();
        sm.update_dimensions(100, 24);
        let lines = vec![
            "ERROR: big problem".to_string(),
            "Error: medium problem".to_string(),
            "error: small problem".to_string(),
        ];
        sm.start_search("error".to_string(), super::SearchDirection::Forward, &lines);
        let search = sm.search_state().unwrap();
        assert_eq!(search.matches.len(), 3);
    }

    #[test]
    fn next_match_cycles_forward() {
        let mut sm = ScrollManager::new();
        sm.update_dimensions(100, 24);
        let lines = vec![
            "match 1".to_string(),
            "match 2".to_string(),
            "match 3".to_string(),
        ];
        sm.start_search("match".to_string(), super::SearchDirection::Forward, &lines);
        assert_eq!(sm.search_state().unwrap().current_match, 0);
        sm.next_match();
        assert_eq!(sm.search_state().unwrap().current_match, 1);
        sm.next_match();
        assert_eq!(sm.search_state().unwrap().current_match, 2);
        sm.next_match();
        assert_eq!(sm.search_state().unwrap().current_match, 0); // wraps
    }

    #[test]
    fn prev_match_cycles_backward() {
        let mut sm = ScrollManager::new();
        sm.update_dimensions(100, 24);
        let lines = vec![
            "match 1".to_string(),
            "match 2".to_string(),
            "match 3".to_string(),
        ];
        sm.start_search("match".to_string(), super::SearchDirection::Forward, &lines);
        assert_eq!(sm.search_state().unwrap().current_match, 0);
        sm.prev_match();
        assert_eq!(sm.search_state().unwrap().current_match, 2); // wraps
        sm.prev_match();
        assert_eq!(sm.search_state().unwrap().current_match, 1);
    }

    #[test]
    fn search_with_no_matches() {
        let mut sm = ScrollManager::new();
        sm.update_dimensions(100, 24);
        let lines = vec!["hello".to_string(), "world".to_string()];
        sm.start_search(
            "notfound".to_string(),
            super::SearchDirection::Forward,
            &lines,
        );
        let search = sm.search_state().unwrap();
        assert_eq!(search.matches.len(), 0);
    }

    #[test]
    fn clear_search_removes_state() {
        let mut sm = ScrollManager::new();
        sm.update_dimensions(100, 24);
        let lines = vec!["error".to_string()];
        sm.start_search("error".to_string(), super::SearchDirection::Forward, &lines);
        assert!(sm.search_state().is_some());
        sm.clear_search();
        assert!(sm.search_state().is_none());
    }
}
