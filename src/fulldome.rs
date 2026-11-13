//! The fulldome, in a terminal: every lens, and what the chosen one brings into focus - to move through,
//! not only to read. It is not a lens and no lens knows of it: a lens says what comes into focus, and
//! this is one surface that lays it out - a dome made real (dome.rs), as plain text is another. A lens made before this existed
//! shows here as it stands.

use crate::entries::Asks;
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
    /// Whether an entry can be made from here: only when one place is stood in.
    pub can_enter: bool,
    /// An entry being filled in.
    pub form: Option<Form>,
    /// The last thing said back: what was written, or why not.
    pub note: String,
}

/// What a key asks of whoever holds the dome.
#[derive(Debug, PartialEq, Eq)]
pub enum Asked {
    Nothing,
    /// Bring the chosen lens into focus again (or show its glimpse).
    Load,
    /// Write this entry with these arguments, as ocoeco glimpse would.
    Write(String, Vec<String>),
    Leave,
}

/// An entry of a lens, being filled in: a person's own hands in the fulldome. It asks only what the
/// entry asks a person for; a field with set values is chosen, not typed; giving the id of a row that
/// exists changes that row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    pub entries: Vec<Asks>,
    pub which: usize,
    /// What has been given for each field; the first is the id.
    pub given: Vec<String>,
    pub at: usize,
    /// Why it was not written, if it was not.
    pub said: String,
}

impl Form {
    pub fn new(entries: Vec<Asks>) -> Form {
        let fields = entries.first().map_or(0, |e| e.fields.len()) + 1;
        Form {
            entries,
            which: 0,
            given: vec![String::new(); fields],
            at: 0,
            said: String::new(),
        }
    }

    pub fn entry(&self) -> &Asks {
        &self.entries[self.which]
    }

    /// The field at a row: its name, whether it is needed, the values it allows. The first row is the id.
    fn field(&self, row: usize) -> (String, bool, Vec<String>) {
        if row == 0 {
            ("id".to_string(), true, Vec::new())
        } else {
            self.entry().fields[row - 1].clone()
        }
    }

    /// What was given, as ocoeco glimpse takes it: --FIELD VALUE for each field given.
    pub fn args(&self) -> Vec<String> {
        let mut args = Vec::new();
        for (row, value) in self
            .given
            .iter()
            .enumerate()
            .filter(|(_, v)| !v.trim().is_empty())
        {
            args.push(format!("--{}", self.field(row).0));
            args.push(value.trim().to_string());
        }
        args
    }

    /// What a key does: Some(true) to write the entry, Some(false) to leave it, None to go on filling.
    pub fn key(&mut self, code: KeyCode) -> Option<bool> {
        let last = self.given.len() - 1;
        let allowed = self.field(self.at).2;
        self.said.clear();
        match code {
            KeyCode::Esc => return Some(false),
            KeyCode::F(2) => return Some(true),
            KeyCode::Enter if self.at == last => return Some(true),
            KeyCode::Enter | KeyCode::Tab | KeyCode::Down => self.at = (self.at + 1).min(last),
            KeyCode::BackTab | KeyCode::Up => self.at = self.at.saturating_sub(1),
            KeyCode::F(3) if self.entries.len() > 1 => {
                // another kind of entry of this lens: what was given as its id is kept
                self.which = (self.which + 1) % self.entries.len();
                let id = std::mem::take(&mut self.given[0]);
                self.given = vec![String::new(); self.entry().fields.len() + 1];
                self.given[0] = id;
                self.at = self.at.min(self.given.len() - 1);
            }
            KeyCode::Right | KeyCode::Left if !allowed.is_empty() => {
                let (now, count) = (
                    allowed.iter().position(|a| *a == self.given[self.at]),
                    allowed.len(),
                );
                let next = match (now, code == KeyCode::Right) {
                    (None, true) => 0,
                    (None, false) => count - 1,
                    (Some(at), true) => (at + 1) % count,
                    (Some(at), false) => (at + count - 1) % count,
                };
                self.given[self.at] = allowed[next].clone();
            }
            KeyCode::Backspace if allowed.is_empty() => {
                self.given[self.at].pop();
            }
            KeyCode::Backspace => self.given[self.at].clear(),
            KeyCode::Char(c) if allowed.is_empty() => self.given[self.at].push(c),
            _ => {}
        }
        None
    }

    /// The form, laid out: each field on a line, the one being filled marked.
    pub fn text(&self) -> String {
        let entry = self.entry();
        let mut text = format!(
            "{} - a new one, or a change to one by its id\n{}\n",
            entry.entry, entry.says
        );
        // why it was not written goes first, where it is seen on any terminal - never below the edge
        if !self.said.is_empty() {
            text.push_str(&format!("not written: {}\n", self.said));
        }
        if self.entries.len() > 1 {
            let kinds: Vec<&str> = self.entries.iter().map(|e| e.entry.as_str()).collect();
            text.push_str(&format!("(F3: another kind - {})\n", kinds.join(", ")));
        }
        text.push('\n');
        for (row, value) in self.given.iter().enumerate() {
            let (name, needed, allowed) = self.field(row);
            let here = row == self.at;
            let shown = if allowed.is_empty() {
                format!("{value}{}", if here { "_" } else { "" })
            } else if here {
                format!(
                    "< {} >   left/right: {}",
                    if value.is_empty() { "-" } else { value },
                    allowed.join(", ")
                )
            } else {
                value.clone()
            };
            text.push_str(&format!(
                "{} {:<14}{} {shown}\n",
                if here { ">" } else { " " },
                name,
                if needed { "*" } else { " " }
            ));
        }
        text.push_str("\n* needed for a new one\n");
        if !entry.supplied.is_empty() {
            text.push_str(&format!(
                "the computer supplies: {}\n",
                entry.supplied.join("; ")
            ));
        }
        text
    }
}

impl InTerminal {
    /// Every lens, the first with a focus chosen.
    pub fn new(scale: String) -> InTerminal {
        InTerminal::of(scale, lenses())
    }

    /// Every lens given - those built in, and those the places in view bring.
    pub fn of(scale: String, all: Vec<crate::lenses::Lens>) -> InTerminal {
        let lenses: Vec<(String, String, bool)> = all
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
            can_enter: false,
            form: None,
            note: String::new(),
        }
    }

    /// The chosen lens.
    pub fn lens(&self) -> &str {
        &self.lenses[self.at].0
    }

    /// Moves as a key says, and says what is needed next.
    pub fn key(&mut self, code: KeyCode) -> Asked {
        // while an entry is being filled in, every key is the form's
        if let Some(form) = &mut self.form {
            return match form.key(code) {
                Some(true) => Asked::Write(form.entry().entry.clone(), form.args()),
                Some(false) => {
                    self.form = None;
                    Asked::Nothing
                }
                None => Asked::Nothing,
            };
        }
        self.note.clear();
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
            KeyCode::Char('e') => {
                let entries = crate::entries::asks_of(self.lens());
                if !self.can_enter {
                    self.note = "to enter, stand in one place: open the fulldome there, without --root or --observed".into();
                } else if entries.is_empty() {
                    self.note = format!("the lens {} has no entries", self.lens());
                } else {
                    self.form = Some(Form::new(entries));
                }
                Asked::Nothing
            }
            KeyCode::Char('r') => Asked::Load,
            _ => Asked::Nothing,
        }
    }

    /// Lays the dome out: the lenses on the left, the chosen one's focus - or the entry being filled in -
    /// on the right, the keys below.
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
        let (title, text, keys) = match &self.form {
            Some(form) => (format!("{id}: to enter"), form.text(), " tab, up/down: field   left/right: choose   enter: next, and write at the last   F2: write it   esc: leave it".to_string()),
            None => (
                if self.glimpse { format!("{id}: what can be glimpsed") } else { format!("{id}: in focus - {}", self.scale) },
                format!("{says}\n\n{}", self.text),
                if self.note.is_empty() { " up/down: lens   space, PgUp: scroll   g: glimpse   e: enter   r: again   q: leave   (· no focus yet)".to_string() } else { format!(" {}", self.note) },
            ),
        };
        frame.render_widget(
            Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .scroll((if self.form.is_some() { 0 } else { self.scroll }, 0))
                .block(Block::bordered().title(title)),
            right,
        );
        frame.render_widget(Line::from(keys), foot);
    }
}

/// Opens the dome in the terminal. `load` brings a lens into focus (false) or says what can be glimpsed
/// through it (true). `write` makes an entry, as ocoeco glimpse would, and says what it wrote - or why
/// it did not, which is then shown in the form with nothing lost. `can_enter` is whether one place is
/// stood in, so that an entry has somewhere to go.
pub fn open(
    scale: String,
    can_enter: bool,
    all: Vec<crate::lenses::Lens>,
    mut load: impl FnMut(&str, bool) -> String,
    mut write: impl FnMut(&str, &[String]) -> anyhow::Result<String>,
) -> anyhow::Result<()> {
    let mut dome = InTerminal::of(scale, all);
    dome.can_enter = can_enter;
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
                Asked::Write(entry, args) => match write(&entry, &args) {
                    Ok(written) => {
                        dome.form = None;
                        dome.note = written;
                        let text = load(dome.lens(), dome.glimpse);
                        dome.text = text;
                    }
                    Err(why) => {
                        if let Some(form) = &mut dome.form {
                            form.said = format!("{why:#}");
                        }
                    }
                },
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
            view.contains("insight ·") && !view.contains("problems ·"),
            "a lens with no focus yet is marked:\n{view}"
        );
    }

    #[test]
    fn an_entry_is_filled_in_the_dome_and_given_to_be_written() {
        let mut dome = InTerminal::new("this place".into());
        // where many places are in view an entry has nowhere to go: said, not done
        assert_eq!(
            (dome.key(KeyCode::Char('e')), dome.form.is_none()),
            (Asked::Nothing, true)
        );
        assert!(dome.note.contains("stand in one place"), "{}", dome.note);
        dome.can_enter = true;
        dome.key(KeyCode::Char('e'));
        assert!(
            seen(&dome).contains("work: to enter")
                && seen(&dome).contains("item - a new one, or a change to one by its id"),
            "{}",
            seen(&dome)
        );
        // typed where typing is asked; a letter that would leave the dome is only a letter here
        for c in "q1".chars() {
            assert_eq!(dome.key(KeyCode::Char(c)), Asked::Nothing);
        }
        dome.key(KeyCode::Tab);
        for c in "A thing".chars() {
            dome.key(KeyCode::Char(c));
        }
        dome.key(KeyCode::Tab);
        // a field with set values is chosen, not typed
        dome.key(KeyCode::Char('x'));
        dome.key(KeyCode::Right);
        dome.key(KeyCode::Right);
        let view = seen(&dome);
        assert!(
            view.contains("> status        * < next >")
                && view.contains("left/right: open, next, in-progress"),
            "{view}"
        );
        assert!(dome
            .form
            .as_ref()
            .unwrap()
            .text()
            .contains("the computer supplies: since (today, when status changes)"));
        dome.key(KeyCode::Left);
        assert_eq!(
            dome.key(KeyCode::F(2)),
            Asked::Write(
                "item".into(),
                ["--id", "q1", "--title", "A thing", "--status", "open"]
                    .map(String::from)
                    .to_vec()
            )
        );
        // not written: the reason stays in the form, and so does everything given
        dome.form.as_mut().unwrap().said =
            "nothing written - it would add a link that points nowhere".into();
        assert!(
            seen(&dome).contains("not written: nothing written") && seen(&dome).contains("A thing")
        );
        assert_eq!(
            (dome.key(KeyCode::Esc), dome.form.is_none()),
            (Asked::Nothing, true)
        );
    }

    #[test]
    fn a_lens_with_several_entries_offers_each_and_keeps_the_id() {
        let mut dome = InTerminal::new("this place".into());
        dome.can_enter = true;
        dome.at = dome
            .lenses
            .iter()
            .position(|(id, _, _)| id == "knowledge")
            .unwrap();
        dome.key(KeyCode::Char('e'));
        dome.key(KeyCode::Char('w'));
        assert!(
            seen(&dome).contains("(F3: another kind - research, source, concept, word)"),
            "{}",
            seen(&dome)
        );
        dome.key(KeyCode::F(3));
        let form = dome.form.as_ref().unwrap();
        assert_eq!(
            (
                form.entry().entry.as_str(),
                form.given[0].as_str(),
                form.given.len()
            ),
            ("source", "w", form.entry().fields.len() + 1)
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
