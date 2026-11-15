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
    /// How the chosen lens is looked at: what it brings into focus, what can be glimpsed through it, or
    /// its beam - how light travels through it.
    pub looking: Looking,
    pub text: String,
    /// What is in view: this place, or every place under a root.
    pub scale: String,
    /// Whether an entry can be made from here: only when one place is stood in.
    pub can_enter: bool,
    /// An entry being filled in.
    pub form: Option<Form>,
    /// The last thing said back: what was written, or why not.
    pub note: String,
    /// The walk: the things stood on, one after another - the last is where one stands. Empty where the
    /// chosen lens is looked through as it is.
    pub stood: Vec<String>,
    /// The things named in what is shown: the line each is on, and its name.
    pub named: Vec<(usize, String)>,
    /// Which of them is pointed at, while walking.
    pub pointed: Option<usize>,
}

/// How a lens is looked at: what it brings into focus; what can be glimpsed through it (entered); or its
/// beam - how light travels through it, from what enters to where it goes on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Looking {
    InFocus,
    Glimpse,
    Beam,
}

/// What a key asks of whoever holds the dome.
#[derive(Debug, PartialEq, Eq)]
pub enum Asked {
    Nothing,
    /// Bring the chosen lens into focus again (or show its glimpse).
    Load,
    /// Write this entry with these arguments, as ocoeco glimpse would.
    Write(String, Vec<String>),
    /// A step of a walk was taken - onto a thing, or back: bring what is now looked through into focus,
    /// and find the things it names.
    Walk,
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
        // a surface said by a lens is not looked through on its own: it lays out what another brings
        let lenses: Vec<(String, String, bool)> = all
            .into_iter()
            .filter(|l| !crate::focus::lays_out_a_plane(l))
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
            looking: Looking::InFocus,
            text: String::new(),
            scale,
            can_enter: false,
            form: None,
            note: String::new(),
            stood: Vec::new(),
            named: Vec::new(),
            pointed: None,
        }
    }

    /// The chosen lens.
    pub fn lens(&self) -> &str {
        &self.lenses[self.at].0
    }

    /// What is looked through now: the chosen lens - or, on a walk, the lens of around from the thing
    /// stood on.
    pub fn looked_through(&self) -> String {
        match self.stood.last() {
            Some(thing) => format!("around@{thing}"),
            None => self.lens().to_string(),
        }
    }

    /// Points among the lines that are about a thing - as the plane says, each line with the thing it is
    /// about. The first is pointed at, or the one pointed at before where it is still there.
    pub fn point_among(&mut self, named: Vec<(usize, String)>) {
        self.named = named;
        self.pointed = match (self.pointed, self.named.len()) {
            (_, 0) => None,
            (Some(at), n) => Some(at.min(n - 1)),
            (None, _) => Some(0),
        };
        if self.pointed.is_none() {
            self.note = "nothing shown here is about a thing to stand on".into();
        }
    }

    /// Keeps what is pointed at in view.
    fn follow(&mut self) {
        if let Some((line, _)) = self.pointed.and_then(|at| self.named.get(at)) {
            self.scroll = (*line as u16 + 2).saturating_sub(6);
        }
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
        // on a walk the keys move among the things shown: down and up point at one, right stands on it,
        // left steps back to where one stood before, and escape ends the walk - not the dome
        match (self.pointed, code) {
            (Some(at), KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab) => {
                self.pointed = Some((at + 1).min(self.named.len().saturating_sub(1)));
                self.follow();
                return Asked::Nothing;
            }
            (Some(at), KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab) => {
                self.pointed = Some(at.saturating_sub(1));
                self.follow();
                return Asked::Nothing;
            }
            (Some(at), KeyCode::Right | KeyCode::Enter | KeyCode::Char('l')) => {
                let Some((_, thing)) = self.named.get(at).cloned() else {
                    return Asked::Nothing;
                };
                self.stood.push(thing);
                (self.scroll, self.looking, self.pointed) = (0, Looking::InFocus, Some(0));
                return Asked::Walk;
            }
            (Some(_), KeyCode::Left | KeyCode::Backspace | KeyCode::Char('h')) => {
                if self.stood.pop().is_some() {
                    (self.scroll, self.pointed) = (0, Some(0));
                    return Asked::Walk;
                }
                (self.pointed, self.named) = (None, Vec::new());
                return Asked::Nothing;
            }
            (Some(_), KeyCode::Esc) => {
                let stood = !self.stood.is_empty();
                (self.pointed, self.named, self.scroll) = (None, Vec::new(), 0);
                self.stood.clear();
                return if stood { Asked::Load } else { Asked::Nothing };
            }
            // to begin a walk: the things named in what is shown are found, and the first pointed at
            (None, KeyCode::Right | KeyCode::Enter | KeyCode::Char('l')) => return Asked::Walk,
            _ => {}
        }
        let choose = |dome: &mut InTerminal, at: usize| {
            dome.at = at;
            dome.scroll = 0;
            dome.looking = Looking::InFocus;
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
            // a key for each way of looking; the same key again goes back to what it brings into focus
            KeyCode::Char(key @ ('g' | 'b')) => {
                let asked = if key == 'g' {
                    Looking::Glimpse
                } else {
                    Looking::Beam
                };
                self.looking = if self.looking == asked {
                    Looking::InFocus
                } else {
                    asked
                };
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
        // on a walk what is looked through is the lens of around, from the thing stood on
        let says: &str = match self.stood.last() {
            Some(_) => self
                .lenses
                .iter()
                .find(|(lens, _, _)| lens == "around")
                .map_or("", |(_, says, _)| says.as_str()),
            None => says.as_str(),
        };
        let (title, text, keys) = match &self.form {
            Some(form) => (format!("{id}: to enter"), form.text(), " tab, up/down: field   left/right: choose   enter: next, and write at the last   F2: write it   esc: leave it".to_string()),
            None => (
                match self.looking {
                    Looking::Glimpse => format!("{id}: what can be glimpsed"),
                    Looking::Beam => format!("{id}: its beam - how light travels through it - {}", self.scale),
                    Looking::InFocus => format!("{id}: in focus - {}", self.scale),
                },
                format!("{says}\n\n{}", self.text),
                if self.note.is_empty() { " up/down: lens   right: walk   space, PgUp: scroll   g: glimpse   b: beam   e: enter   r: again   q: leave   (· no focus yet)".to_string() } else { format!(" {}", self.note) },
            ),
        };
        // on a walk: where one stands is said, the keys are those of walking, and the line that names the
        // thing pointed at is marked
        let walking = self.form.is_none() && self.pointed.is_some();
        let title = match self.stood.last() {
            Some(thing) if self.form.is_none() => {
                format!("around: standing on {thing} - {}", self.scale)
            }
            _ => title,
        };
        let keys = if walking && self.note.is_empty() {
            " up/down: a thing   right, enter: stand on it   left: back   esc: end the walk   q: leave".to_string()
        } else {
            keys
        };
        let pointed = self
            .pointed
            .and_then(|at| self.named.get(at))
            .map(|(line, _)| *line)
            .filter(|_| walking);
        let before = text
            .lines()
            .count()
            .saturating_sub(self.text.lines().count());
        let text: Vec<Line> = text
            .lines()
            .enumerate()
            .map(|(at, line)| match pointed {
                Some(line_of_it) if line_of_it + before == at => Line::styled(
                    line.to_string(),
                    Style::new().add_modifier(Modifier::REVERSED),
                ),
                _ => Line::from(line.to_string()),
            })
            .collect();
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

/// The lines of what is looked through that are about a thing: asked of the plane, where a lens is in
/// focus - a glimpse or a beam is not a plane of things to stand on.
fn walkable(
    dome: &InTerminal,
    about: &mut impl FnMut(&str) -> Vec<(usize, String)>,
) -> Vec<(usize, String)> {
    if dome.looking == Looking::InFocus {
        about(&dome.looked_through())
    } else {
        Vec::new()
    }
}

/// Opens the dome in the terminal. `load` brings a lens into focus, says what can be glimpsed
/// through it, or shows its beam, as `Looking` says. `write` makes an entry, as ocoeco glimpse would, and says what it wrote - or why
/// it did not, which is then shown in the form with nothing lost. `can_enter` is whether one place is
/// stood in, so that an entry has somewhere to go. `about` says, for what is looked through, which lines
/// are about a thing, and which - as the plane says, not as the words suggest.
pub fn open(
    scale: String,
    can_enter: bool,
    all: Vec<crate::lenses::Lens>,
    begins: Option<String>,
    mut load: impl FnMut(&str, Looking) -> String,
    mut write: impl FnMut(&str, &[String]) -> anyhow::Result<String>,
    mut about: impl FnMut(&str) -> Vec<(usize, String)>,
) -> anyhow::Result<()> {
    let mut dome = InTerminal::of(scale, all);
    // one begins on the whole the place one looks from says for itself - and, where it says none, on the
    // flow of work
    if let Some(at) = begins.and_then(|lens| dome.lenses.iter().position(|(id, _, _)| *id == lens))
    {
        dome.at = at;
    }
    dome.can_enter = can_enter;
    dome.text = load(dome.lens(), Looking::InFocus);
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
                    let text = load(&dome.looked_through(), dome.looking);
                    dome.text = text;
                    if dome.pointed.is_some() {
                        let named = walkable(&dome, &mut about);
                        dome.point_among(named);
                    }
                }
                // a step of a walk: what is now looked through is brought into focus, and the things it
                // names are found among the things there are
                Asked::Walk => {
                    let text = load(&dome.looked_through(), dome.looking);
                    dome.text = text;
                    let named = walkable(&dome, &mut about);
                    dome.point_among(named);
                }
                Asked::Write(entry, args) => match write(&entry, &args) {
                    Ok(written) => {
                        dome.form = None;
                        dome.note = written;
                        let text = load(&dome.looked_through(), dome.looking);
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
        // as if the lens beside the chosen one had no focus yet: one in view however many lenses there are
        let beside = dome
            .lenses
            .iter()
            .position(|lens| lens.0 == "work")
            .expect("the chosen lens")
            - 1;
        dome.lenses[beside].2 = false;
        let unfocused = format!("{} ·", dome.lenses[beside].0);
        let view = seen(&dome);
        for lens in [
            "knowledge",
            "looking-back",
            "signs",
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
            view.contains(&unfocused) && !view.contains("problems ·"),
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
        assert_eq!((dome.key(KeyCode::Up), dome.lens()), (Asked::Load, "signs"));
        assert_eq!(
            (dome.key(KeyCode::Char('g')), dome.looking),
            (Asked::Load, Looking::Glimpse)
        );
        dome.text = "  working - How something works".into();
        assert!(seen(&dome).contains("signs: what can be glimpsed"));
        // another key, another way of looking at the same lens; the same key again goes back
        assert_eq!(
            (dome.key(KeyCode::Char('b')), dome.looking),
            (Asked::Load, Looking::Beam)
        );
        assert!(seen(&dome).contains("signs: its beam - how light travels through it"));
        assert_eq!(
            (dome.key(KeyCode::Char('b')), dome.looking),
            (Asked::Load, Looking::InFocus)
        );
        dome.key(KeyCode::Char('g'));
        assert_eq!(
            (dome.key(KeyCode::Up), dome.looking),
            (Asked::Load, Looking::InFocus),
            "another lens is first seen in focus"
        );
        assert_eq!(
            (dome.key(KeyCode::PageDown), dome.scroll),
            (Asked::Nothing, 10)
        );
        assert_eq!(dome.key(KeyCode::Char('q')), Asked::Leave);
    }
}

#[cfg(test)]
mod a_walk {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn seen(dome: &InTerminal) -> String {
        let mut terminal = Terminal::new(TestBackend::new(110, 16)).unwrap();
        terminal.draw(|frame| dome.draw(frame)).unwrap();
        terminal.backend().to_string()
    }

    #[test]
    fn one_walks_from_what_is_shown_standing_on_a_thing_and_stepping_back() {
        let mut dome = InTerminal::new("this place".into());
        dome.text = "in progress (1)\n  claude: r/backlog:a - A\nshowing (1)\n  r/the-sign - held back by r/people:someone".into();
        // what each line is about is given, as the plane says it - the window finds nothing by itself
        let about = || {
            vec![
                (1, "r/backlog:a".to_string()),
                (3, "r/problems:the-sign".to_string()),
            ]
        };
        assert_eq!(
            dome.key(KeyCode::Right),
            Asked::Walk,
            "to walk is to ask which lines are about a thing"
        );
        dome.point_among(about());
        assert_eq!(dome.pointed, Some(0));
        assert!(
            seen(&dome).contains("right, enter: stand on it"),
            "the keys say how to walk"
        );
        assert_eq!(
            (dome.key(KeyCode::Down), dome.pointed),
            (Asked::Nothing, Some(1))
        );
        assert_eq!(
            (dome.key(KeyCode::Down), dome.pointed),
            (Asked::Nothing, Some(1)),
            "and stop at the last"
        );
        assert_eq!(dome.key(KeyCode::Right), Asked::Walk);
        assert_eq!(
            dome.looked_through(),
            "around@r/problems:the-sign",
            "standing on a thing is the lens of around, from it"
        );
        dome.text = "standing on (1)\n  r/problems:the-sign - it does not fit\nto here - what points at it (1)\n  r/people:someone halted this".into();
        dome.point_among(vec![
            (1, "r/problems:the-sign".to_string()),
            (3, "r/people:someone".to_string()),
        ]);
        assert!(
            seen(&dome).contains("around: standing on r/problems:the-sign"),
            "where one stands is said:\n{}",
            seen(&dome)
        );
        assert_eq!(
            (dome.key(KeyCode::Left), dome.looked_through().as_str()),
            (Asked::Walk, "work"),
            "a step back, to the lens one walked from"
        );
        assert_eq!(
            (dome.key(KeyCode::Esc), dome.pointed),
            (Asked::Nothing, None),
            "escape ends the walk"
        );
        assert_eq!(
            dome.key(KeyCode::Esc),
            Asked::Leave,
            "and only then leaves the dome"
        );
        // where nothing shown is about a thing, there is nothing to point at - and that is said
        dome.point_among(Vec::new());
        assert!(
            dome.named.is_empty() && dome.pointed.is_none() && dome.note.contains("about a thing")
        );
    }
}
