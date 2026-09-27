//! Priority-based item selection for one-row status bars.
//!
//! Each item offers its forms richest first (full, compact, minimal). A
//! required item always renders, starting from its smallest form; optional
//! items start hidden. Items then upgrade in priority order, each to the
//! richest form that still fits the real render width. Nothing is built as one
//! long string and clipped at the tail: a lower-priority item simply gets a
//! smaller form or none, so a warning displaces decoration instead of being
//! cut off.

use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

/// One semantic item in a status bar.
pub struct Item {
    /// Lower is more important.
    pub priority: u8,
    /// Always rendered (in its smallest form at worst).
    pub required: bool,
    /// Candidate renderings, richest first. Each form carries its own leading
    /// separator so an omitted neighbour leaves no dangling ` | `.
    pub forms: Vec<Vec<Span<'static>>>,
}

impl Item {
    pub fn required(priority: u8, forms: Vec<Vec<Span<'static>>>) -> Self {
        Self {
            priority,
            required: true,
            forms,
        }
    }

    pub fn optional(priority: u8, forms: Vec<Vec<Span<'static>>>) -> Self {
        Self {
            priority,
            required: false,
            forms,
        }
    }
}

/// Display width of a run of spans.
pub fn spans_width(spans: &[Span<'_>]) -> usize {
    spans
        .iter()
        .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
        .sum()
}

/// Chooses a form for each item (in display order) so the row fits `width`.
///
/// Returns the chosen spans concatenated in display order and their width.
pub fn fit(items: Vec<Item>, width: usize) -> (Vec<Span<'static>>, usize) {
    let widths: Vec<Vec<usize>> = items
        .iter()
        .map(|item| item.forms.iter().map(|form| spans_width(form)).collect())
        .collect();
    let mut chosen: Vec<Option<usize>> = items
        .iter()
        .map(|item| (item.required && !item.forms.is_empty()).then(|| item.forms.len() - 1))
        .collect();
    let width_of = |index: usize, form: Option<usize>| form.map_or(0, |form| widths[index][form]);
    let mut used: usize = chosen
        .iter()
        .enumerate()
        .map(|(index, form)| width_of(index, *form))
        .sum();

    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by_key(|&index| items[index].priority);
    for index in order {
        let current = width_of(index, chosen[index]);
        let upgrade = (0..items[index].forms.len())
            .take_while(|&form| Some(form) != chosen[index])
            .find(|&form| used - current + widths[index][form] <= width);
        if let Some(form) = upgrade {
            used = used - current + widths[index][form];
            chosen[index] = Some(form);
        }
    }

    let spans = items
        .into_iter()
        .zip(chosen)
        .filter_map(|(mut item, form)| form.map(|form| item.forms.swap_remove(form)))
        .flatten()
        .collect();
    (spans, used)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(text: &str) -> Vec<Span<'static>> {
        vec![Span::raw(text.to_string())]
    }

    fn text(spans: &[Span<'_>]) -> String {
        spans.iter().map(|span| span.content.as_ref()).collect()
    }

    #[test]
    fn higher_priority_items_upgrade_first_and_nothing_is_clipped() {
        let items = vec![
            Item::required(1, vec![form("[iter 3/10]")]),
            Item::optional(5, vec![form(" | decoration")]),
            Item::required(2, vec![form(" | WARNING: 3 skipped"), form(" | !3")]),
        ];
        let (spans, used) = fit(items, 32);
        assert_eq!(text(&spans), "[iter 3/10] | WARNING: 3 skipped");
        assert_eq!(used, 32);
    }

    #[test]
    fn required_items_fall_back_to_their_smallest_form() {
        let items = vec![
            Item::required(1, vec![form("[iter 3/10]")]),
            Item::required(2, vec![form(" | WARNING: 3 skipped"), form(" | !3")]),
        ];
        let (spans, _) = fit(items, 16);
        assert_eq!(text(&spans), "[iter 3/10] | !3");
    }
}
