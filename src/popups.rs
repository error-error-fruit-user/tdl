//! Small windows that ask or tell one thing, then get out of the way. They share one look: plain
//! border, a blank row around the text, and wrapped lines so they stay small on any terminal.

pub mod cancel;
pub mod dialog;
pub mod hashing;
pub mod legend;
pub mod progress;
pub mod resize;
pub mod running;
pub mod status;
pub mod three_way_dialog;
pub mod warning;

use ratatui::{
    crossterm::event::KeyCode,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
};

use crate::{
    gamepad::{self, Family, Prompt},
    layout,
    ritual::{self, Page, Phase},
    theme,
};

/// The widest a popup's text gets before it wraps, so popups stay small.
pub const MAX_TEXT_WIDTH: u16 = 46;

/// `text` wrapped to fit a popup in `area`.
pub fn wrap(text: &str, area: Rect) -> Vec<String> {
    let room = area
        .width
        .saturating_sub(2 * layout::MARGIN + 2 + 2 * layout::PADDING);
    layout::wrap(text, MAX_TEXT_WIDTH.min(room) as usize)
}

/// The answers as `[Y]es  [N]o`: the first letter of each is the key that picks it. The one
/// `selected` with the arrows, if any, is drawn as the cursor.
pub fn answers(options: &[&str], selected: Option<usize>) -> Line<'static> {
    answers_for(options, selected, gamepad::shown())
}

/// The prompts that answer `count` questions' answers on a controller: A and B for a yes or no,
/// and for three answers A, X and Y (B backs out without answering). None for other counts.
pub fn buttons(count: usize) -> Option<&'static [Prompt]> {
    match count {
        2 => Some(&[Prompt::Confirm, Prompt::Back]),
        3 => Some(&[Prompt::Confirm, Prompt::Search, Prompt::Cut]),
        _ => None,
    }
}

/// Which of `count` answers a controller's `key_code` gives, if it is one of the buttons that
/// answer ([`buttons`]): `Enter` is A, `Esc` is B, `/` is X and `x` is Y.
pub fn pad_answer(count: usize, key_code: KeyCode) -> Option<usize> {
    let prompt = [Prompt::Confirm, Prompt::Back, Prompt::Cut, Prompt::Search]
        .into_iter()
        .find(|prompt| prompt.chant() == key_code)?;
    buttons(count)?.iter().position(|answer| *answer == prompt)
}

/// [`answers`], drawn for a controller (`pad`: its family) or
/// the keyboard (`None`): each answer is led by the button that picks it instead of by its first
/// letter, when it has one.
pub fn answers_for(
    options: &[&str],
    selected: Option<usize>,
    pad: Option<Family>,
) -> Line<'static> {
    if let (Some(family), Some(buttons)) = (pad, buttons(options.len())) {
        let mut spans = Vec::new();
        for (option, button) in options.iter().zip(buttons) {
            if !spans.is_empty() {
                spans.push(Span::raw("  "));
            }
            spans.push(Span::styled(
                button.label(family),
                Style::new().fg(theme::worn().colors.key),
            ));
            spans.push(Span::styled(
                format!(" {option}"),
                Style::new().fg(theme::worn().colors.text),
            ));
        }
        return Line::from(spans);
    }
    let mut spans = Vec::new();
    for (i, option) in options.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        let mut letters = option.chars();
        let first = letters.next().map(String::from).unwrap_or_default();
        let rest = letters.as_str().to_string();
        if selected == Some(i) {
            spans.push(Span::styled(
                format!("[{first}]{rest}"),
                theme::worn().styles.cursor,
            ));
            continue;
        }
        spans.push(Span::styled(
            "[",
            Style::new().fg(theme::worn().colors.muted),
        ));
        spans.push(Span::styled(
            first,
            Style::new().fg(theme::worn().colors.key),
        ));
        spans.push(Span::styled(
            "]",
            Style::new().fg(theme::worn().colors.muted),
        ));
        spans.push(Span::styled(
            rest,
            Style::new().fg(theme::worn().colors.text),
        ));
    }
    Line::from(spans)
}

/// Picking one of `count` answers with the arrows: `←`/`→` (or `Tab`/`Backtab`) move the
/// highlight, wrapping around, and `Enter` takes the highlighted answer. Nothing is highlighted
/// to begin with, so `Enter` alone never answers a question by accident.
///
/// `None` if the key isn't one of these, for the dialog's own keys.
pub fn pick(selected: &mut Option<usize>, count: usize, key_code: KeyCode) -> Option<Phase<usize>> {
    match key_code {
        KeyCode::Right | KeyCode::Tab => {
            *selected = Some(selected.map_or(0, |i| (i + 1) % count));
            Some(Phase::Render)
        }
        KeyCode::Left | KeyCode::BackTab => {
            *selected = Some(selected.map_or(count - 1, |i| (i + count - 1) % count));
            Some(Phase::Render)
        }
        KeyCode::Enter => Some(selected.map_or(Phase::Prompt, Phase::Return)),
        _ => None,
    }
}

/// Draws a popup: `lines` centered, a blank row above and below them, and `footer` (usually the
/// answers) one blank row further down.
pub fn draw(page: &mut Page, title: &str, border: Color, lines: &[Line], footer: Option<Line>) {
    let width = lines
        .iter()
        .chain(&footer)
        .chain([&Line::from(format!("[ {title} ]"))])
        .map(|line| line.width() as u16)
        .max()
        .unwrap_or(0);
    let height = lines.len() as u16 + 2 + footer.as_ref().map_or(0, |_| 2);
    let (width, height) = layout::window(width, height);
    let (veil, area, ..) = ritual::conjure_window(title, width, height, border, page.area());
    let veil = veil.border_type(theme::worn().borders.popup);
    let inner = veil.inner(area);
    page.render_widget(veil, area);
    let row =
        |offset: u16| Rect::new(inner.x, inner.y + offset, inner.width, 1).intersection(inner);
    for (i, line) in lines.iter().enumerate() {
        page.render_widget(line.clone().centered(), row(1 + i as u16));
    }
    if let Some(footer) = footer {
        page.render_widget(footer.centered(), row(lines.len() as u16 + 2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_highlight_their_first_letter() {
        let line = answers(&["Yes", "No"], None);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "[Y]es  [N]o");
        assert_eq!(line.spans[1].style.fg, Some(theme::worn().colors.key));
    }

    #[test]
    fn on_a_controller_the_answers_are_led_by_their_buttons() {
        let text = |line: Line| {
            line.spans
                .iter()
                .map(|s| s.content.to_string())
                .collect::<String>()
        };
        let yes_no = answers_for(&["Yes", "No"], None, Some(Family::Xbox));
        assert_eq!(text(yes_no), "(A) Yes  (B) No");
        let three = ["Save", "Discard", "Back"];
        let line = answers_for(&three, None, Some(Family::PlayStation));
        assert_eq!(text(line), "(✕) Save  (□) Discard  (△) Back");
        // 🔤 four answers have no buttons to go round, so they keep their letters
        let four = answers_for(&["a", "b", "c", "d"], None, Some(Family::Xbox));
        assert_eq!(text(four), "[a]  [b]  [c]  [d]");
    }

    #[test]
    fn dialog_answers_are_on_the_physical_buttons_the_screen_shows() {
        use gilrs::Button::{East, North, South, West};
        for family in [
            Family::Xbox,
            Family::PlayStation,
            Family::SteamDeck,
            Family::Generic,
        ] {
            let answer = |count, button| {
                Prompt::of(button, family).and_then(|prompt| pad_answer(count, prompt.chant()))
            };
            // yes or no: A (south) and B (east)
            assert_eq!((answer(2, South), answer(2, East)), (Some(0), Some(1)));
            assert_eq!((answer(2, West), answer(2, North)), (None, None));
            // three: A (south), X (west) and Y (north), B backs out
            assert_eq!(answer(3, South), Some(0), "{family:?}");
            assert_eq!(answer(3, West), Some(1), "{family:?}");
            assert_eq!(answer(3, North), Some(2), "{family:?}");
            assert_eq!(answer(3, East), None, "{family:?}");
            // 🏷️ and the answers are labeled with the buttons that do those
            let labeled = answers_for(&["Tag", "Name", "Skip"], None, Some(family))
                .spans
                .iter()
                .map(|span| span.content.to_string())
                .collect::<String>();
            let name = |prompt: Prompt| prompt.label(family);
            let wanted = format!(
                "{} Tag  {} Name  {} Skip",
                name(Prompt::Confirm),
                name(Prompt::Search),
                name(Prompt::Cut)
            );
            assert_eq!(labeled, wanted);
        }
        // the Nintendo layout has A on the right, and still answers with it
        let nintendo = |button| {
            Prompt::of(button, Family::Nintendo).and_then(|prompt| pad_answer(2, prompt.chant()))
        };
        assert_eq!((nintendo(East), nintendo(South)), (Some(0), Some(1)));
    }

    #[test]
    fn a_controllers_buttons_answer_by_their_place() {
        assert_eq!(pad_answer(2, KeyCode::Enter), Some(0));
        assert_eq!(pad_answer(2, KeyCode::Esc), Some(1));
        assert_eq!(pad_answer(2, KeyCode::Char('x')), None);
        assert_eq!(pad_answer(3, KeyCode::Enter), Some(0));
        assert_eq!(pad_answer(3, KeyCode::Char('/')), Some(1));
        assert_eq!(pad_answer(3, KeyCode::Char('x')), Some(2));
        assert_eq!(pad_answer(3, KeyCode::Esc), None, "B backs out of three");
    }

    #[test]
    fn the_arrows_move_between_answers_and_enter_takes_one() {
        let mut selected = None;
        // nothing is picked until an arrow says so
        assert!(pick(&mut selected, 2, KeyCode::Enter) == Some(Phase::Prompt));
        assert!(pick(&mut selected, 2, KeyCode::Right) == Some(Phase::Render));
        assert_eq!(selected, Some(0));
        pick(&mut selected, 2, KeyCode::Right);
        assert_eq!(selected, Some(1));
        pick(&mut selected, 2, KeyCode::Right);
        assert_eq!(selected, Some(0), "wrapping around");
        pick(&mut selected, 2, KeyCode::Left);
        assert!(pick(&mut selected, 2, KeyCode::Enter) == Some(Phase::Return(1)));
        // from nothing, ← starts at the last answer
        let mut fresh = None;
        pick(&mut fresh, 3, KeyCode::Left);
        assert_eq!(fresh, Some(2));
        assert!(pick(&mut fresh, 3, KeyCode::Char('y')).is_none());
    }

    #[test]
    fn the_picked_answer_is_drawn_as_the_cursor() {
        let line = answers(&["Yes", "No"], Some(1));
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "[Y]es  [N]o");
        let picked = line
            .spans
            .iter()
            .find(|span| span.content == "[N]o")
            .unwrap();
        assert_eq!(picked.style, theme::worn().styles.cursor);
    }

    #[test]
    fn popup_text_wraps_to_stay_small() {
        let wide = Rect::new(0, 0, 200, 40);
        let lines = wrap(&"word ".repeat(40), wide);
        assert!(
            lines
                .iter()
                .all(|l| l.chars().count() <= MAX_TEXT_WIDTH as usize)
        );
        // and to fit a narrow terminal
        let narrow = Rect::new(0, 0, 30, 40);
        assert!(
            wrap(&"word ".repeat(40), narrow)
                .iter()
                .all(|l| l.chars().count() <= 24)
        );
    }
}
