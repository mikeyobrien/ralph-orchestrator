use tui_term::vt100::Parser;

pub struct TerminalWidget {
    parser: Parser,
}

impl Default for TerminalWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalWidget {
    pub fn new() -> Self {
        Self {
            parser: Parser::new(24, 80, 0),
        }
    }

    /// Creates a new terminal widget with the specified dimensions.
    pub fn with_size(rows: u16, cols: u16) -> Self {
        Self {
            parser: Parser::new(rows, cols, 0),
        }
    }

    pub fn process(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    pub fn parser(&self) -> &Parser {
        &self.parser
    }

    /// Returns total lines in scrollback.
    pub fn total_lines(&self) -> usize {
        let (rows, _cols) = self.parser.screen().size();
        self.parser.screen().scrollback() + rows as usize
    }

    /// Resizes the terminal to new dimensions.
    ///
    /// Only resizes if dimensions actually changed to avoid
    /// unnecessary parser recreation.
    pub fn resize(&mut self, rows: u16, cols: u16) {
        let (current_rows, current_cols) = self.parser.screen().size();
        if current_rows != rows || current_cols != cols {
            self.parser = Parser::new(rows, cols, 0);
        }
    }

    /// Returns the current terminal dimensions as (rows, cols).
    pub fn size(&self) -> (u16, u16) {
        self.parser.screen().size()
    }

    /// Clears the terminal screen and scrollback.
    pub fn clear(&mut self) {
        let (rows, cols) = self.parser.screen().size();
        self.parser = Parser::new(rows, cols, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_default_size() {
        let widget = TerminalWidget::new();
        let (rows, cols) = widget.size();
        assert_eq!(rows, 24);
        assert_eq!(cols, 80);
    }

    #[test]
    fn test_with_size_creates_custom_dimensions() {
        let widget = TerminalWidget::with_size(40, 120);
        let (rows, cols) = widget.size();
        assert_eq!(rows, 40);
        assert_eq!(cols, 120);
    }

    #[test]
    fn test_resize_changes_dimensions() {
        let mut widget = TerminalWidget::new();
        assert_eq!(widget.size(), (24, 80));

        widget.resize(50, 160);
        assert_eq!(widget.size(), (50, 160));
    }

    #[test]
    fn test_resize_noop_when_same_size() {
        let mut widget = TerminalWidget::with_size(30, 100);
        // Process some data to have state
        widget.process(b"Hello");

        // Resize to same dimensions should be a no-op
        widget.resize(30, 100);
        assert_eq!(widget.size(), (30, 100));
    }

    #[test]
    fn test_clear_preserves_size() {
        let mut widget = TerminalWidget::with_size(40, 120);
        widget.process(b"Some content\n");

        widget.clear();

        // Size should be preserved after clear
        assert_eq!(widget.size(), (40, 120));
    }
}
