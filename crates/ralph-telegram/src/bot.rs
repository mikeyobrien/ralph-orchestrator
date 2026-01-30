use std::path::Path;

use async_trait::async_trait;

use crate::error::{TelegramError, TelegramResult};

/// Trait abstracting Telegram bot operations for testability.
///
/// Production code uses [`TelegramBot`]; tests can provide a mock implementation.
#[async_trait]
pub trait BotApi: Send + Sync {
    /// Send a text message to the given chat.
    ///
    /// Returns the Telegram message ID of the sent message.
    async fn send_message(&self, chat_id: i64, text: &str) -> TelegramResult<i32>;

    /// Send a document (file) to the given chat with an optional caption.
    ///
    /// Returns the Telegram message ID of the sent message.
    async fn send_document(
        &self,
        chat_id: i64,
        file_path: &Path,
        caption: Option<&str>,
    ) -> TelegramResult<i32>;

    /// Send a photo to the given chat with an optional caption.
    ///
    /// Returns the Telegram message ID of the sent message.
    async fn send_photo(
        &self,
        chat_id: i64,
        file_path: &Path,
        caption: Option<&str>,
    ) -> TelegramResult<i32>;
}

/// Wraps a `teloxide::Bot` and provides formatted messaging for Ralph.
pub struct TelegramBot {
    bot: teloxide::Bot,
}

impl TelegramBot {
    /// Create a new TelegramBot from a bot token.
    pub fn new(token: &str) -> Self {
        Self {
            bot: teloxide::Bot::new(token),
        }
    }

    /// Format an outgoing question message using Telegram HTML.
    ///
    /// Includes emoji, hat name, iteration number, and the question text.
    /// The question body is converted from markdown to Telegram HTML for
    /// rich rendering. The hat and loop ID are HTML-escaped for safety.
    pub fn format_question(hat: &str, iteration: u32, loop_id: &str, question: &str) -> String {
        let escaped_hat = escape_html(hat);
        let escaped_loop = escape_html(loop_id);
        let formatted_question = markdown_to_telegram_html(question);
        format!(
            "❓ <b>{escaped_hat}</b> (iteration {iteration}, loop <code>{escaped_loop}</code>)\n\n{formatted_question}",
        )
    }

    /// Format a greeting message sent when the bot starts.
    pub fn format_greeting(loop_id: &str) -> String {
        let escaped = escape_html(loop_id);
        format!("🤖 Ralph bot online — monitoring loop <code>{escaped}</code>")
    }

    /// Format a farewell message sent when the bot shuts down.
    pub fn format_farewell(loop_id: &str) -> String {
        let escaped = escape_html(loop_id);
        format!("👋 Ralph bot shutting down — loop <code>{escaped}</code> complete")
    }
}

/// Escape special HTML characters for Telegram's HTML parse mode.
///
/// Telegram requires `<`, `>`, and `&` to be escaped in HTML-formatted messages.
pub fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Convert Ralph-generated markdown to Telegram HTML.
///
/// Handles the subset of markdown that Ralph produces:
/// - `**bold**` → `<b>bold</b>`
/// - `` `inline code` `` → `<code>inline code</code>`
/// - ````code blocks```` → `<pre>code</pre>`
/// - `# Header` → `<b>Header</b>`
/// - `- item` / `* item` → `• item`
///
/// Text that isn't markdown is HTML-escaped to prevent injection.
/// This function is for Ralph-generated content; use [`escape_html`] for
/// user-supplied text.
pub fn markdown_to_telegram_html(md: &str) -> String {
    let mut result = String::with_capacity(md.len());
    let mut in_code_block = false;
    let mut code_block_content = String::new();

    for line in md.lines() {
        // Handle fenced code blocks (``` or ```)
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            if in_code_block {
                // Closing code fence
                result.push_str("<pre>");
                result.push_str(&escape_html(&code_block_content));
                result.push_str("</pre>");
                result.push('\n');
                code_block_content.clear();
                in_code_block = false;
            } else {
                // Opening code fence (ignore language specifier)
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            if !code_block_content.is_empty() {
                code_block_content.push('\n');
            }
            code_block_content.push_str(line);
            continue;
        }

        // Headers: # ... → bold line
        if let Some(header_text) = strip_header(trimmed) {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str("<b>");
            result.push_str(&escape_html(header_text));
            result.push_str("</b>");
            continue;
        }

        // List items: - item or * item → • item
        if let Some(item_text) = strip_list_item(trimmed) {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str("• ");
            result.push_str(&convert_inline(&escape_html(item_text)));
            continue;
        }

        // Regular line: apply inline formatting
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(&convert_inline(&escape_html(line)));
    }

    // Handle unclosed code block
    if in_code_block && !code_block_content.is_empty() {
        result.push_str("<pre>");
        result.push_str(&escape_html(&code_block_content));
        result.push_str("</pre>");
    }

    result
}

/// Strip markdown header prefix (# to ######) and return the header text.
fn strip_header(line: &str) -> Option<&str> {
    if !line.starts_with('#') {
        return None;
    }
    let hash_count = line.chars().take_while(|c| *c == '#').count();
    if hash_count > 6 {
        return None;
    }
    let rest = &line[hash_count..];
    if rest.starts_with(' ') {
        Some(rest.trim())
    } else {
        None
    }
}

/// Strip list item prefix (- or *) and return the item text.
fn strip_list_item(line: &str) -> Option<&str> {
    if let Some(rest) = line.strip_prefix("- ") {
        Some(rest)
    } else if let Some(rest) = line.strip_prefix("* ") {
        Some(rest)
    } else {
        None
    }
}

/// Convert inline markdown (bold and inline code) within already-escaped HTML text.
///
/// Processes `**bold**` → `<b>bold</b>` and `` `code` `` → `<code>code</code>`.
/// Since input is already HTML-escaped, bold delimiters (`**`) and backticks
/// appear literally and won't conflict with HTML entities.
fn convert_inline(escaped: &str) -> String {
    let mut out = String::with_capacity(escaped.len());
    let chars: Vec<char> = escaped.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Inline code: `...`
        if chars[i] == '`'
            && let Some(end) = find_closing_backtick(&chars, i + 1)
        {
            out.push_str("<code>");
            for c in &chars[i + 1..end] {
                out.push(*c);
            }
            out.push_str("</code>");
            i = end + 1;
            continue;
        }

        // Bold: **...**
        if i + 1 < len
            && chars[i] == '*'
            && chars[i + 1] == '*'
            && let Some(end) = find_closing_double_star(&chars, i + 2)
        {
            out.push_str("<b>");
            for c in &chars[i + 2..end] {
                out.push(*c);
            }
            out.push_str("</b>");
            i = end + 2;
            continue;
        }

        out.push(chars[i]);
        i += 1;
    }

    out
}

/// Find closing backtick starting from position `start`.
fn find_closing_backtick(chars: &[char], start: usize) -> Option<usize> {
    (start..chars.len()).find(|&j| chars[j] == '`')
}

/// Find closing `**` starting from position `start`.
fn find_closing_double_star(chars: &[char], start: usize) -> Option<usize> {
    let len = chars.len();
    let mut j = start;
    while j + 1 < len {
        if chars[j] == '*' && chars[j + 1] == '*' {
            return Some(j);
        }
        j += 1;
    }
    None
}

#[async_trait]
impl BotApi for TelegramBot {
    async fn send_message(&self, chat_id: i64, text: &str) -> TelegramResult<i32> {
        use teloxide::payloads::SendMessageSetters;
        use teloxide::prelude::*;
        use teloxide::types::ParseMode;

        let result = self
            .bot
            .send_message(teloxide::types::ChatId(chat_id), text)
            .parse_mode(ParseMode::Html)
            .await
            .map_err(|e| TelegramError::Send {
                attempts: 1,
                reason: e.to_string(),
            })?;

        Ok(result.id.0)
    }

    async fn send_document(
        &self,
        chat_id: i64,
        file_path: &Path,
        caption: Option<&str>,
    ) -> TelegramResult<i32> {
        use teloxide::payloads::SendDocumentSetters;
        use teloxide::prelude::*;
        use teloxide::types::{InputFile, ParseMode};

        let input_file = InputFile::file(file_path);
        let mut request = self
            .bot
            .send_document(teloxide::types::ChatId(chat_id), input_file);

        if let Some(cap) = caption {
            request = request.caption(cap).parse_mode(ParseMode::Html);
        }

        let result = request.await.map_err(|e| TelegramError::Send {
            attempts: 1,
            reason: e.to_string(),
        })?;

        Ok(result.id.0)
    }

    async fn send_photo(
        &self,
        chat_id: i64,
        file_path: &Path,
        caption: Option<&str>,
    ) -> TelegramResult<i32> {
        use teloxide::payloads::SendPhotoSetters;
        use teloxide::prelude::*;
        use teloxide::types::{InputFile, ParseMode};

        let input_file = InputFile::file(file_path);
        let mut request = self
            .bot
            .send_photo(teloxide::types::ChatId(chat_id), input_file);

        if let Some(cap) = caption {
            request = request.caption(cap).parse_mode(ParseMode::Html);
        }

        let result = request.await.map_err(|e| TelegramError::Send {
            attempts: 1,
            reason: e.to_string(),
        })?;

        Ok(result.id.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// A mock BotApi for testing that records sent messages.
    struct MockBot {
        sent: Arc<Mutex<Vec<(i64, String)>>>,
        next_id: Arc<Mutex<i32>>,
        should_fail: bool,
    }

    impl MockBot {
        fn new() -> Self {
            Self {
                sent: Arc::new(Mutex::new(Vec::new())),
                next_id: Arc::new(Mutex::new(1)),
                should_fail: false,
            }
        }

        fn failing() -> Self {
            Self {
                sent: Arc::new(Mutex::new(Vec::new())),
                next_id: Arc::new(Mutex::new(1)),
                should_fail: true,
            }
        }

        fn sent_messages(&self) -> Vec<(i64, String)> {
            self.sent.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl BotApi for MockBot {
        async fn send_message(&self, chat_id: i64, text: &str) -> TelegramResult<i32> {
            if self.should_fail {
                return Err(TelegramError::Send {
                    attempts: 1,
                    reason: "mock failure".to_string(),
                });
            }
            self.sent.lock().unwrap().push((chat_id, text.to_string()));
            let mut id = self.next_id.lock().unwrap();
            let current = *id;
            *id += 1;
            Ok(current)
        }

        async fn send_document(
            &self,
            chat_id: i64,
            file_path: &Path,
            caption: Option<&str>,
        ) -> TelegramResult<i32> {
            if self.should_fail {
                return Err(TelegramError::Send {
                    attempts: 1,
                    reason: "mock failure".to_string(),
                });
            }
            let label = format!(
                "[doc:{}]{}",
                file_path.display(),
                caption.map(|c| format!(" {c}")).unwrap_or_default()
            );
            self.sent.lock().unwrap().push((chat_id, label));
            let mut id = self.next_id.lock().unwrap();
            let current = *id;
            *id += 1;
            Ok(current)
        }

        async fn send_photo(
            &self,
            chat_id: i64,
            file_path: &Path,
            caption: Option<&str>,
        ) -> TelegramResult<i32> {
            if self.should_fail {
                return Err(TelegramError::Send {
                    attempts: 1,
                    reason: "mock failure".to_string(),
                });
            }
            let label = format!(
                "[photo:{}]{}",
                file_path.display(),
                caption.map(|c| format!(" {c}")).unwrap_or_default()
            );
            self.sent.lock().unwrap().push((chat_id, label));
            let mut id = self.next_id.lock().unwrap();
            let current = *id;
            *id += 1;
            Ok(current)
        }
    }

    #[test]
    fn format_question_includes_hat_and_loop() {
        let msg = TelegramBot::format_question("Builder", 3, "main", "Which DB should I use?");
        assert!(msg.contains("<b>Builder</b>"));
        assert!(msg.contains("iteration 3"));
        assert!(msg.contains("<code>main</code>"));
        assert!(msg.contains("Which DB should I use?"));
    }

    #[test]
    fn format_question_escapes_html_in_content() {
        let msg = TelegramBot::format_question("Hat", 1, "loop-1", "Use <b>this</b> & that?");
        assert!(msg.contains("&lt;b&gt;this&lt;/b&gt;"));
        assert!(msg.contains("&amp; that?"));
    }

    #[test]
    fn format_question_renders_markdown() {
        let msg = TelegramBot::format_question(
            "Builder",
            5,
            "main",
            "Should I use **async** or `sync` here?",
        );
        assert!(msg.contains("<b>async</b>"));
        assert!(msg.contains("<code>sync</code>"));
    }

    #[test]
    fn format_greeting_includes_loop_id() {
        let msg = TelegramBot::format_greeting("feature-auth");
        assert!(msg.contains("<code>feature-auth</code>"));
        assert!(msg.contains("online"));
    }

    #[test]
    fn format_farewell_includes_loop_id() {
        let msg = TelegramBot::format_farewell("main");
        assert!(msg.contains("<code>main</code>"));
        assert!(msg.contains("shutting down"));
    }

    #[test]
    fn escape_html_handles_special_chars() {
        assert_eq!(
            super::escape_html("a < b & c > d"),
            "a &lt; b &amp; c &gt; d"
        );
        assert_eq!(super::escape_html("no specials"), "no specials");
        assert_eq!(super::escape_html(""), "");
    }

    // ---- markdown_to_telegram_html tests ----

    #[test]
    fn md_to_html_bold_text() {
        assert_eq!(
            super::markdown_to_telegram_html("This is **bold** text"),
            "This is <b>bold</b> text"
        );
    }

    #[test]
    fn md_to_html_inline_code() {
        assert_eq!(
            super::markdown_to_telegram_html("Run `cargo test` now"),
            "Run <code>cargo test</code> now"
        );
    }

    #[test]
    fn md_to_html_code_block() {
        let input = "Before\n```rust\nfn main() {}\n```\nAfter";
        let result = super::markdown_to_telegram_html(input);
        assert!(result.contains("<pre>fn main() {}</pre>"));
        assert!(result.contains("Before"));
        assert!(result.contains("After"));
    }

    #[test]
    fn md_to_html_headers() {
        assert_eq!(super::markdown_to_telegram_html("# Title"), "<b>Title</b>");
        assert_eq!(
            super::markdown_to_telegram_html("## Subtitle"),
            "<b>Subtitle</b>"
        );
        assert_eq!(super::markdown_to_telegram_html("### Deep"), "<b>Deep</b>");
    }

    #[test]
    fn md_to_html_list_items() {
        let input = "- first item\n- second item\n* third item";
        let result = super::markdown_to_telegram_html(input);
        assert_eq!(result, "• first item\n• second item\n• third item");
    }

    #[test]
    fn md_to_html_escapes_html_in_content() {
        assert_eq!(
            super::markdown_to_telegram_html("Use <div> & <span>"),
            "Use &lt;div&gt; &amp; &lt;span&gt;"
        );
    }

    #[test]
    fn md_to_html_escapes_html_in_bold() {
        assert_eq!(
            super::markdown_to_telegram_html("**<script>alert(1)</script>**"),
            "<b>&lt;script&gt;alert(1)&lt;/script&gt;</b>"
        );
    }

    #[test]
    fn md_to_html_escapes_html_in_code_block() {
        let input = "```\n<div>html</div>\n```";
        let result = super::markdown_to_telegram_html(input);
        assert_eq!(result, "<pre>&lt;div&gt;html&lt;/div&gt;</pre>\n");
    }

    #[test]
    fn md_to_html_plain_text_passthrough() {
        assert_eq!(
            super::markdown_to_telegram_html("Just plain text"),
            "Just plain text"
        );
    }

    #[test]
    fn md_to_html_empty_string() {
        assert_eq!(super::markdown_to_telegram_html(""), "");
    }

    #[test]
    fn md_to_html_mixed_formatting() {
        let input = "# Status\n\nBuild **passed** with `0 errors`.\n\n- Tests: 42\n- Coverage: 85%";
        let result = super::markdown_to_telegram_html(input);
        assert!(result.contains("<b>Status</b>"));
        assert!(result.contains("<b>passed</b>"));
        assert!(result.contains("<code>0 errors</code>"));
        assert!(result.contains("• Tests: 42"));
        assert!(result.contains("• Coverage: 85%"));
    }

    #[test]
    fn md_to_html_unclosed_code_block() {
        let input = "```\nunclosed code";
        let result = super::markdown_to_telegram_html(input);
        assert_eq!(result, "<pre>unclosed code</pre>");
    }

    #[test]
    fn md_to_html_list_items_with_inline_formatting() {
        let input = "- **bold** item\n- `code` item";
        let result = super::markdown_to_telegram_html(input);
        assert_eq!(result, "• <b>bold</b> item\n• <code>code</code> item");
    }

    #[tokio::test]
    async fn mock_bot_send_message_succeeds() {
        let bot = MockBot::new();
        let id = bot.send_message(123, "hello").await.unwrap();
        assert_eq!(id, 1);

        let sent = bot.sent_messages();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0], (123, "hello".to_string()));
    }

    #[tokio::test]
    async fn mock_bot_send_message_increments_id() {
        let bot = MockBot::new();
        let id1 = bot.send_message(123, "first").await.unwrap();
        let id2 = bot.send_message(123, "second").await.unwrap();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[tokio::test]
    async fn mock_bot_failure_returns_send_error() {
        let bot = MockBot::failing();
        let result = bot.send_message(123, "hello").await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TelegramError::Send { attempts: 1, .. }
        ));
    }
}
