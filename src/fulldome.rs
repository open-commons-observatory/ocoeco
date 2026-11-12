//! The fulldome, in a terminal: every lens, and what the chosen one brings into focus - to move through,
//! not only to read. It is not a lens and no lens knows of it: a lens says what comes into focus, and
//! this is one surface that lays it out - a dome made real (dome.rs), as plain text is another. A lens made before this existed
//! shows here as it stands.

use crate::lenses::lenses;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

/// What the dome shows: the lenses, the one chosen, and what it brings into focus.
pub struct InTerminal {
    /// Each lens: its id, what it says of itself, and whether it has a focus.
    pub lenses: Vec<(String, String, bool)>,
    pub at: usize,
    pub scroll: u16,
    /// Showing what can be glimpsed through the lens, rather than what it brings into focus.
    pub glimpse: bool,
    pub text: String,
    /// What is in view: this place, or every place under a root.
    pub scale: String,
}

/// What a key asks of whoever holds the dome.
#[derive(Debug, PartialEq, Eq)]
pub enum Asked {
    Nothing,
    /// Bring the chosen lens into focus again (or show its glimpse).
    Load,
    Leave,
}

impl InTerminal {
    /// Every lens, the first with a focus chosen.
    pub fn new(scale: String) -> InTerminal {
        let lenses: Vec<(String, String, bool)> = lenses()
            .into_iter()
            .map(|l| (l.lens.id, l.lens.says, !l.focus.is_empty()))
            .collect();
        let at = lenses
            .iter()
            .position(|(id, _, _)| id == "work")
            .or_else(|| lenses.iter().position(|(_, _, has)| *has))
            .unwrap_or(0);
        InTerminal {
            lenses,
            at,
            scroll: 0,
            glimpse: false,
            text: String::new(),
            scale,
        }
    }

    /// The chosen lens.
    pub fn lens(&self) -> &str {
        &self.lenses[self.at].0
    }

    /// Moves as a key says, and says what is needed next.
    pub fn key(&mut self, code: KeyCode) -> Asked {
        let choose = |dome: &mut InTerminal, at: usize| {
            dome.at = at;
            dome.scroll = 0;
            dome.glimpse = false;
            Asked::Load
        };
        match code {
            KeyCode::Char('q') | KeyCode::Esc => Asked::Leave,
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab
                if self.at + 1 < self.lenses.len() =>
            {
                let below = self.at + 1;
                choose(self, below)
            }
            KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab if self.at > 0 => {
                let above = self.at - 1;
                choose(self, above)
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.scroll = self.scroll.saturating_add(10);
                Asked::Nothing
            }
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_sub(10);
                Asked::Nothing
            }
            KeyCode::Home => {
                self.scroll = 0;
                Asked::Nothing
            }
            KeyCode::Char('g') => {
                self.glimpse = !self.glimpse;
                self.scroll = 0;
                Asked::Load
            }
            KeyCode::Char('r') => Asked::Load,
            _ => Asked::Nothing,
        }
    }

    /// Lays the dome out: the lenses on the left, the chosen one's focus on the right, the keys below.
    pub fn draw(&self, frame: &mut Frame) {
        let [body, foot] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());
        let [left, right] =
            Layout::horizontal([Constraint::Length(20), Constraint::Min(0)]).areas(body);
        let items: Vec<ListItem> = self
            .lenses
            .iter()
            .map(|(id, _, has)| ListItem::new(if *has { id.clone() } else { format!("{id} ·") }))
            .collect();
        let mut chosen = ListState::default().with_selected(Some(self.at));
        frame.render_stateful_widget(
            List::new(items)
                .block(Block::bordered().title("lenses"))
                .highlight_style(Style::new().add_modifier(Modifier::REVERSED)),
            left,
            &mut chosen,
        );
        let (id, says, _) = &self.lenses[self.at];
        let title = if self.glimpse {
            format!("{id}: what can be glimpsed")
        } else {
            format!("{id}: in focus - {}", self.scale)
        };
        frame.render_widget(
            Paragraph::new(format!("{says}\n\n{}", self.text))
                .wrap(Wrap { trim: false })
                .scroll((self.scroll, 0))
                .block(Block::bordered().title(title)),
            right,
        );
        frame.render_widget(Line::from(" up/down: lens   space, PgUp: scroll   g: glimpse   r: again   q: leave   (· no focus yet)"), foot);
    }
}

/// Opens the dome in the terminal. `load` brings a lens into focus (false) or says what can be glimpsed
/// through it (true); it is asked again whenever the lens changes.
pub fn open(scale: String, mut load: impl FnMut(&str, bool) -> String) -> anyhow::Result<()> {
    let mut dome = InTerminal::new(scale);
    dome.text = load(dome.lens(), false);
    let mut terminal = ratatui::init();
    let ended = (|| -> anyhow::Result<()> {
        loop {
            terminal.draw(|frame| dome.draw(frame))?;
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match dome.key(key.code) {
                Asked::Leave => return Ok(()),
                Asked::Load => {
                    let text = load(dome.lens(), dome.glimpse);
                    dome.text = text;
                }
                Asked::Nothing => {}
            }
        }
    })();
    ratatui::restore();
    ended
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn seen(dome: &InTerminal) -> String {
        let mut terminal = Terminal::new(TestBackend::new(110, 16)).unwrap();
        terminal.draw(|frame| dome.draw(frame)).unwrap();
        terminal.backend().to_string()
    }

    #[test]
    fn the_dome_shows_every_lens_and_what_the_chosen_one_brings_into_focus() {
        let mut dome = InTerminal::new("this place".into());
        assert_eq!(dome.lens(), "work", "the flow of work is where one begins");
        dome.text = "in progress (1)\n  claude: r/a - A".into();
        dome.lenses[0].2 = false; // as if the first lens had no focus yet
        let view = seen(&dome);
        for lens in [
            "knowledge",
            "looking-back",
            "problems",
            "purpose",
            "reasoning",
            "results",
            "work",
        ] {
            assert!(view.contains(lens), "{lens} is not shown:\n{view}");
        }
        assert!(
            view.contains("work: in focus - this place")
                && view.contains("in progress (1)")
                && view.contains("claude: r/a - A"),
            "{view}"
        );
        assert!(
            view.contains("knowledge ·") && !view.contains("problems ·"),
            "a lens with no focus yet is marked:\n{view}"
        );
    }

    #[test]
    fn keys_move_between_lenses_and_say_what_is_needed() {
        let mut dome = InTerminal::new("this place".into());
        let last = dome.lenses.len() - 1;
        assert_eq!(
            (dome.at, dome.key(KeyCode::Down)),
            (last, Asked::Nothing),
            "work is last: nothing below it"
        );
        assert_eq!(
            (dome.key(KeyCode::Up), dome.lens()),
            (Asked::Load, "results")
        );
        assert_eq!(
            (dome.key(KeyCode::Char('g')), dome.glimpse),
            (Asked::Load, true)
        );
        dome.text = "  working - How something works".into();
        assert!(seen(&dome).contains("results: what can be glimpsed"));
        assert_eq!(
            (dome.key(KeyCode::Up), dome.glimpse),
            (Asked::Load, false),
            "another lens is first seen in focus"
        );
        assert_eq!(
            (dome.key(KeyCode::PageDown), dome.scroll),
            (Asked::Nothing, 10)
        );
        assert_eq!(dome.key(KeyCode::Char('q')), Asked::Leave);
    }
}
