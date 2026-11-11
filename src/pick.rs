//! Choosing a manual-work record without typing it again: an interactive list in the terminal. Type
//! a few words to narrow it (busiest first), arrows to move, Enter to choose; then the minutes and the
//! load are offered with their usual values, and Enter records it. "+ new" adds what was typed. A
//! person never types an action twice, so the ledger does not split into near-duplicates.
use crate::mark::Mark;
use crate::note::Row;
use anyhow::{Context, Result};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;
use std::path::Path;

/// A record as the list shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Rec {
    pub id: String,
    pub action: String,
    pub count: i64,
    pub usual_minutes: i64,
    pub load: i64,
}

/// What the person chose.
#[derive(Debug, Clone, PartialEq)]
pub struct Pick {
    pub id: Option<String>,
    pub action: String,
    pub new: bool,
    pub minutes: Option<i64>,
    pub load: Option<i64>,
}

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Cancel,
    Chosen(Pick),
}

#[derive(Debug, PartialEq)]
enum Step {
    Choose,
    Details,
}

/// The records whose action holds every typed word, busiest first.
pub fn narrow<'a>(recs: &'a [Rec], typed: &str) -> Vec<&'a Rec> {
    let words: Vec<String> = typed
        .to_lowercase()
        .split_whitespace()
        .map(String::from)
        .collect();
    let mut v: Vec<&Rec> = recs
        .iter()
        .filter(|r| {
            let a = r.action.to_lowercase();
            words.iter().all(|w| a.contains(w.as_str()))
        })
        .collect();
    v.sort_by_key(|r| std::cmp::Reverse(r.count));
    v
}

/// The list and the choice being made - kept apart from the terminal, so it can be tested.
pub struct State {
    recs: Vec<Rec>,
    pub filter: String,
    at: usize,
    step: Step,
    chosen: Option<Rec>,
    minutes: String,
    load: String,
    field: usize,
}

impl State {
    pub fn new(recs: Vec<Rec>) -> Self {
        State {
            recs,
            filter: String::new(),
            at: 0,
            step: Step::Choose,
            chosen: None,
            minutes: String::new(),
            load: String::new(),
            field: 0,
        }
    }
    fn shown(&self) -> Vec<&Rec> {
        narrow(&self.recs, &self.filter)
    }
    /// The entries on offer: the matching records, then "+ new" when something was typed.
    fn entries(&self) -> usize {
        self.shown().len() + usize::from(!self.filter.trim().is_empty())
    }
    /// One key; Some when the person is done.
    pub fn key(&mut self, k: KeyCode) -> Option<Outcome> {
        match self.step {
            Step::Choose => match k {
                KeyCode::Esc => return Some(Outcome::Cancel),
                KeyCode::Up => self.at = self.at.saturating_sub(1),
                KeyCode::Down if self.at + 1 < self.entries() => self.at += 1,
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.at = 0;
                }
                KeyCode::Char(c) => {
                    self.filter.push(c);
                    self.at = 0;
                }
                KeyCode::Enter => {
                    let r = match self.shown().get(self.at) {
                        Some(r) => (*r).clone(),
                        None if !self.filter.trim().is_empty() => Rec {
                            id: String::new(),
                            action: self.filter.trim().to_string(),
                            count: 0,
                            usual_minutes: 1,
                            load: 5,
                        },
                        None => return None,
                    };
                    self.minutes = r.usual_minutes.to_string();
                    self.load = r.load.to_string();
                    self.chosen = Some(r);
                    self.field = 0;
                    self.step = Step::Details;
                }
                _ => {}
            },
            Step::Details => match k {
                KeyCode::Esc => self.step = Step::Choose,
                KeyCode::Tab | KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down => {
                    self.field = 1 - self.field
                }
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    let f = if self.field == 0 {
                        &mut self.minutes
                    } else {
                        &mut self.load
                    };
                    f.push(c);
                }
                KeyCode::Backspace => {
                    let f = if self.field == 0 {
                        &mut self.minutes
                    } else {
                        &mut self.load
                    };
                    f.pop();
                }
                KeyCode::Enter => {
                    let r = self.chosen.clone()?;
                    let new = r.id.is_empty();
                    return Some(Outcome::Chosen(Pick {
                        id: (!new).then_some(r.id),
                        action: r.action,
                        new,
                        minutes: self.minutes.parse().ok(),
                        load: self.load.parse::<i64>().ok().map(|l| l.min(10)),
                    }));
                }
                _ => {}
            },
        }
        None
    }
}

/// Draws the state: what was typed, the list, and what the keys do.
pub fn render(f: &mut Frame, st: &State) {
    let [top, mid, bottom] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(f.area());
    match st.step {
        Step::Choose => {
            f.render_widget(
                Paragraph::new(format!("› {}", st.filter)).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" manual work - type to narrow "),
                ),
                top,
            );
            let mut items: Vec<ListItem> = st
                .shown()
                .iter()
                .map(|r| ListItem::new(format!("{:>4}×  {}", r.count, r.action)))
                .collect();
            if !st.filter.trim().is_empty() {
                items.push(ListItem::new(format!("   +   new: {}", st.filter.trim())));
            }
            let mut ls = ListState::default().with_selected(Some(st.at));
            f.render_stateful_widget(
                List::new(items).highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
                mid,
                &mut ls,
            );
            f.render_widget(Paragraph::new("↑↓ move · Enter choose · Esc leave"), bottom);
        }
        Step::Details => {
            let action = st
                .chosen
                .as_ref()
                .map(|r| r.action.clone())
                .unwrap_or_default();
            f.render_widget(
                Paragraph::new(format!("› {action}"))
                    .block(Block::default().borders(Borders::ALL).title(" this time ")),
                top,
            );
            let mark = |i: usize| if st.field == i { "▸" } else { " " };
            let body = format!(
                "{} minutes: {}\n{} load (0 nothing · 2 easy · 5 hard · 10 maximal): {}",
                mark(0),
                st.minutes,
                mark(1),
                st.load
            );
            f.render_widget(Paragraph::new(body), mid);
            f.render_widget(
                Paragraph::new("digits edit · Tab switch · Enter record · Esc back"),
                bottom,
            );
        }
    }
}

/// The project's manual-work records, for the list.
pub fn records(dir: &Path) -> Result<Vec<Rec>> {
    let mark =
        Mark::parse(&std::fs::read_to_string(dir.join("oco.toml")).context("no oco.toml here")?)?;
    let path = mark
        .exports
        .get("manual_work")
        .context("this repository does not export a manual_work table")?;
    let text = std::fs::read_to_string(dir.join(path)).unwrap_or_default();
    let mut recs = Vec::new();
    for l in text.lines().filter(|l| !l.trim().is_empty()) {
        let r: Row = serde_json::from_str(l)?;
        let int = |k: &str| r.text(k).parse::<i64>().unwrap_or(0);
        let count = int("count").max(1);
        recs.push(Rec {
            id: r.text("id"),
            action: r.text("action"),
            count: int("count"),
            usual_minutes: (int("minutes") / count).max(1),
            load: if r.text("load").is_empty() {
                5
            } else {
                int("load")
            },
        });
    }
    Ok(recs)
}

/// The interactive list in the terminal; None when the person leaves without choosing.
pub fn run(dir: &Path) -> Result<Option<Pick>> {
    let mut st = State::new(records(dir)?);
    let mut terminal = ratatui::init();
    let result = (|| -> Result<Option<Pick>> {
        loop {
            terminal.draw(|f| render(f, &st))?;
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    match st.key(k.code) {
                        Some(Outcome::Cancel) => return Ok(None),
                        Some(Outcome::Chosen(p)) => return Ok(Some(p)),
                        None => {}
                    }
                }
            }
        }
    })();
    ratatui::restore();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn recs() -> Vec<Rec> {
        vec![
            Rec {
                id: "patch".into(),
                action: "patch source code by matching text".into(),
                count: 19,
                usual_minutes: 3,
                load: 4,
            },
            Rec {
                id: "boot".into(),
                action: "boot a test kernel and capture the phone's state".into(),
                count: 9,
                usual_minutes: 6,
                load: 5,
            },
            Rec {
                id: "watch".into(),
                action: "watch a CI run until it finishes".into(),
                count: 9,
                usual_minutes: 2,
                load: 5,
            },
        ]
    }

    #[test]
    fn every_typed_word_narrows_and_the_busiest_comes_first() {
        let r = recs();
        assert_eq!(narrow(&r, "").first().unwrap().id, "patch");
        let n = narrow(&r, "kernel boot");
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].id, "boot");
        assert!(narrow(&r, "nothing like this").is_empty());
    }

    #[test]
    fn choosing_bumps_with_the_usual_minutes_unless_changed() {
        let mut st = State::new(recs());
        for c in "ci run".chars() {
            assert_eq!(st.key(KeyCode::Char(c)), None);
        }
        st.key(KeyCode::Enter);
        st.key(KeyCode::Backspace);
        st.key(KeyCode::Char('4'));
        let got = st.key(KeyCode::Enter);
        assert_eq!(
            got,
            Some(Outcome::Chosen(Pick {
                id: Some("watch".into()),
                action: "watch a CI run until it finishes".into(),
                new: false,
                minutes: Some(4),
                load: Some(5)
            }))
        );
    }

    #[test]
    fn what_matches_nothing_is_added_as_new_and_esc_leaves() {
        let mut st = State::new(recs());
        for c in "flash the phone".chars() {
            st.key(KeyCode::Char(c));
        }
        st.key(KeyCode::Enter);
        match st.key(KeyCode::Enter) {
            Some(Outcome::Chosen(p)) => assert!(
                p.new
                    && p.id.is_none()
                    && p.action == "flash the phone"
                    && p.minutes == Some(1)
                    && p.load == Some(5)
            ),
            other => panic!("{other:?}"),
        }
        assert_eq!(State::new(recs()).key(KeyCode::Esc), Some(Outcome::Cancel));
    }

    #[test]
    fn it_draws_what_was_typed_and_the_list() {
        let mut st = State::new(recs());
        st.key(KeyCode::Char('w'));
        let mut t = Terminal::new(TestBackend::new(70, 9)).unwrap();
        t.draw(|f| render(f, &st)).unwrap();
        let screen: String = t
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(
            screen.contains("› w")
                && screen.contains("watch a CI run")
                && screen.contains("+   new: w"),
            "{screen}"
        );
    }
}
