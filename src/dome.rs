//! The dome: what a star's shine can be focused onto. A lens says what comes into focus; a dome is the
//! one thing every surface must be able to do with it - lay it out. Plain text, a screen of a given width,
//! a terminal, and whatever someone makes later are surfaces: each a dome made real. A surface may be
//! said by a lens too, and the page is: as-page, with how it looks in what the lens says. A lens knows of
//! none of them, so a lens made before a surface existed shows on it as it stands. The fulldome is every
//! lens, in focus, on a dome.

use crate::focus::Brought;
use anyhow::{bail, Result};

/// The port: lays out a plane - what a lens brought into focus, planes placed within it included.
pub trait Dome {
    fn lay_out(&self, parts: &[Brought]) -> String;
    /// The whole: a plane laid out within the surface's own frame - its title, and for a page its style.
    fn whole(&self, scale: &str, plane: &[Brought]) -> String;
    /// What this surface requires of whatever shows it, for the shapes to look as they were fitted -
    /// so that when it looks wrong, it can be said why. A surface that flows requires nothing.
    fn requires(&self) -> Option<String> {
        None
    }
}

/// A measure of a whole, as ten cells: how the text surfaces show it.
pub fn bar(so_much: f64, of: f64) -> String {
    let filled = if of > 0.0 {
        ((so_much / of) * 10.0).round().clamp(0.0, 10.0) as usize
    } else {
        0
    };
    format!("{}{}", "█".repeat(filled), "░".repeat(10 - filled))
}

/// The order a part's lines are laid out in, and how far along each is. A list is as it was written.
/// A chain is walked from where it begins: a line, then the lines that go on from where it arrives -
/// so what followed from what is seen, whatever order the lines were kept in. Every surface walks a
/// part this one way; how far along looks is each surface's own.
#[allow(clippy::needless_range_loop)]
pub fn walked(part: &Brought) -> Vec<(usize, usize)> {
    let Some((from, to)) = &part.chain else {
        return (0..part.lines.len()).map(|at| (at, 0)).collect();
    };
    let value = |at: usize, name: &str| {
        part.values
            .get(at)
            .and_then(|v| v.get(name))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };
    let starts: Vec<String> = (0..part.lines.len()).map(|at| value(at, from)).collect();
    let ends: Vec<Vec<String>> = (0..part.lines.len())
        .map(|at| {
            value(at, to)
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter(|t| !t.is_empty())
                .map(str::to_string)
                .collect()
        })
        .collect();
    fn follow(
        node: &str,
        along: usize,
        starts: &[String],
        ends: &[Vec<String>],
        done: &mut [bool],
        order: &mut Vec<(usize, usize)>,
    ) {
        for at in 0..starts.len() {
            if !done[at] && starts[at] == node {
                done[at] = true;
                order.push((at, along));
                for end in &ends[at] {
                    follow(end, along + 1, starts, ends, done, order);
                }
            }
        }
    }
    let arrived: std::collections::BTreeSet<&String> = ends.iter().flatten().collect();
    let (mut done, mut order) = (vec![false; starts.len()], Vec::new());
    // from what nothing leads to; then whatever is left, which only a ring can leave
    for begins in [true, false] {
        for at in 0..starts.len() {
            if !done[at] && (!begins || !arrived.contains(&starts[at])) {
                follow(
                    &starts[at].clone(),
                    0,
                    &starts,
                    &ends,
                    &mut done,
                    &mut order,
                );
            }
        }
    }
    order
}

/// The plain-text surface: a heading on its own line, its lines beneath it, set in by two spaces.
pub struct Text;

impl Dome for Text {
    fn lay_out(&self, parts: &[Brought]) -> String {
        let mut text = String::new();
        for part in parts {
            // a plane placed within this one: its name and what it says, then its own shapes - or why none
            if let Some(about) = &part.about {
                text.push_str(&format!(
                    "\n{}: {about}\n",
                    part.title.as_deref().unwrap_or_default()
                ));
                match &part.nothing_because {
                    Some(why) => text.push_str(&format!("  ({why})\n")),
                    None => text.push_str(&self.lay_out(&part.within)),
                }
                continue;
            }
            let inset = if part.title.is_some() { "  " } else { "" };
            if let Some(title) = &part.title {
                text.push_str(title);
                text.push('\n');
            }
            for (at, along) in walked(part) {
                let (line, along) = (&part.lines[at], "  ".repeat(along));
                text.push_str(inset);
                text.push_str(&along);
                if let Some((so_much, of)) = part.measures.get(at).copied().flatten() {
                    text.push_str(&bar(so_much, of));
                    text.push(' ');
                }
                text.push_str(line);
                text.push('\n');
            }
        }
        text
    }

    fn whole(&self, scale: &str, plane: &[Brought]) -> String {
        format!("the fulldome - {scale}\n{}", self.lay_out(plane))
    }
}

/// Text made safe to stand inside a page of the web.
pub fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// How a page offers itself in the other languages it can be read in: for its head, where each other one
/// is; and for a reader, every language by its own name - the one being read as it is, each other as a way
/// to the same page in it. The site is laid out once for each language: the one it is written in - the
/// first of `languages` - where it always was, each other under a folder of its tag, the same paths within.
/// Nothing where there is one language only.
pub fn in_other_languages(
    path: &str,
    lang: &str,
    languages: &[(String, String)],
) -> (String, String) {
    let Some((written_in, _)) = languages.first().filter(|_| languages.len() > 1) else {
        return Default::default();
    };
    let deep = path.matches('/').count() + usize::from(lang != written_in);
    let way = |to: &str| {
        let under = if to == written_in {
            String::new()
        } else {
            format!("{to}/")
        };
        escaped(&format!("{}{under}{path}", "../".repeat(deep)))
    };
    let others = languages.iter().filter(|(tag, _)| tag != lang);
    let head = others
        .clone()
        .map(|(tag, _)| {
            format!(
                "<link rel=\"alternate\" hreflang=\"{tag}\" href=\"{}\">\n",
                way(tag)
            )
        })
        .collect();
    let offered = languages
        .iter()
        .map(|(tag, name)| {
            if tag == lang {
                format!("<span lang=\"{tag}\">{}</span>", escaped(name))
            } else {
                format!(
                    "<a href=\"{}\" hreflang=\"{tag}\" lang=\"{tag}\">{}</a>",
                    way(tag),
                    escaped(name)
                )
            }
        })
        .collect::<Vec<_>>()
        .join(" · ");
    (head, format!("<p class=\"tongues\">{offered}</p>\n"))
}

/// How many columns a text takes on a screen of monospaced cells.
fn wide(text: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(text)
}

/// Words fitted into lines no wider than `width` columns. A word wider than a whole line is broken
/// where the line ends: that is its joint - nothing may reach past the volume.
pub fn fitted(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let mut word = word.to_string();
        loop {
            let room = width - wide(&line);
            let gap = usize::from(!line.is_empty());
            if wide(&word) + gap <= room {
                if gap == 1 {
                    line.push(' ');
                }
                line.push_str(&word);
                break;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
                continue;
            }
            // wider than a whole line: as much as fits, and the rest goes on
            let mut head = String::new();
            let mut rest = String::new();
            for c in word.chars() {
                if rest.is_empty()
                    && (head.is_empty()
                        || wide(&head) + unicode_width::UnicodeWidthChar::width(c).unwrap_or(0)
                            <= width)
                {
                    head.push(c);
                } else {
                    rest.push(c);
                }
            }
            lines.push(head);
            if rest.is_empty() {
                break;
            }
            word = rest;
        }
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    lines
}

/// Text as a screen: a volume of so many columns of monospaced characters, unwrapped. Shapes are fitted
/// into it - a heading, a line whose continuation sits under its start, a bar that is never broken - so
/// no line reaches past the width, whatever the data. It says what it requires of whatever shows it,
/// and it looks at its own shape before a character is written.
pub struct Screen {
    pub width: usize,
}

impl Screen {
    /// `text` fitted after `first`, its continuation under where it began.
    fn put(&self, out: &mut Vec<String>, first: &str, text: &str) {
        let under = " ".repeat(wide(first));
        for (at, fitted) in fitted(text, self.width.saturating_sub(wide(first)))
            .into_iter()
            .enumerate()
        {
            out.push(format!("{}{fitted}", if at == 0 { first } else { &under }));
        }
    }

    fn parts(&self, out: &mut Vec<String>, parts: &[Brought]) {
        for part in parts {
            if let Some(about) = &part.about {
                out.push(String::new());
                self.put(
                    out,
                    &format!("{}: ", part.title.as_deref().unwrap_or_default()),
                    about,
                );
                match &part.nothing_because {
                    Some(why) => self.put(out, "  ", &format!("({why})")),
                    None => self.parts(out, &part.within),
                }
                continue;
            }
            let inset = if part.title.is_some() { "  " } else { "" };
            if let Some(title) = &part.title {
                self.put(out, "", title);
            }
            for (at, along) in walked(part) {
                let (line, along) = (&part.lines[at], "  ".repeat(along));
                match part.measures.get(at).copied().flatten() {
                    Some((so_much, of)) => {
                        self.put(out, &format!("{inset}{} ", bar(so_much, of)), line)
                    }
                    None => self.put(out, &format!("{inset}{along}"), line),
                }
            }
        }
    }

    /// The screen written out - after it has looked at its own shape: every line within the width.
    fn written(&self, mut lines: Vec<String>) -> String {
        let beyond = lines.iter().filter(|line| wide(line) > self.width).count();
        if beyond > 0 {
            lines.push(format!(
                "this screen is not sound: {beyond} line(s) reach past {} columns",
                self.width
            ));
        }
        lines.join("\n") + "\n"
    }
}

impl Dome for Screen {
    fn lay_out(&self, parts: &[Brought]) -> String {
        let mut out = Vec::new();
        self.parts(&mut out, parts);
        self.written(out)
    }

    fn whole(&self, scale: &str, plane: &[Brought]) -> String {
        let mut out = Vec::new();
        self.put(&mut out, "", &format!("the fulldome - {scale}"));
        self.put(
            &mut out,
            "",
            &format!("(laid out for {})", self.requires().unwrap_or_default()),
        );
        self.parts(&mut out, plane);
        self.written(out)
    }

    fn requires(&self) -> Option<String> {
        Some(format!(
            "{} columns of monospaced characters, unwrapped",
            self.width
        ))
    }
}

/// The projection as it is, before any pot: its parts, each with its lines, what each line is about and
/// what it means - for a program, or an agent, to read as structure and not as laid-out text.
pub struct Json;

impl Dome for Json {
    fn lay_out(&self, parts: &[Brought]) -> String {
        serde_json::to_string_pretty(parts).unwrap_or_default() + "\n"
    }

    fn whole(&self, scale: &str, plane: &[Brought]) -> String {
        // the plane as it is, planes within it included: nothing of its shape is lost
        serde_json::to_string_pretty(&serde_json::json!({"scale": scale, "plane": plane}))
            .unwrap_or_default()
            + "\n"
    }
}

/// The surfaces a projection can be laid out on, by name - besides the terminal, which is a place to stand.
pub const SURFACES: [&str; 3] = ["text", "screen", "json"];

/// The lines alone, as they are: for a plane whose lines are laid out already - what a surface said by a
/// lens brings into focus.
pub struct Lines;

impl Dome for Lines {
    fn lay_out(&self, parts: &[Brought]) -> String {
        parts
            .iter()
            .flat_map(|part| part.lines.iter())
            .map(|line| format!("{line}\n"))
            .collect()
    }

    fn whole(&self, _scale: &str, plane: &[Brought]) -> String {
        self.lay_out(plane)
    }
}

/// The surface of that name; `width` is the screen's volume, in columns.
pub fn surface(name: &str, width: usize) -> Result<Box<dyn Dome>> {
    Ok(match name {
        "text" => Box::new(Text),
        "lines" => Box::new(Lines),
        "json" => Box::new(Json),
        "screen" => Box::new(Screen { width }),
        other => bail!(
            "no surface is called {other}: there are {} and {} (and the terminal: ocoeco fulldome) - or a lens that says a surface, as as-page says the page",
            SURFACES[..SURFACES.len() - 1].join(", "),
            SURFACES[SURFACES.len() - 1]
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_offers_itself_in_each_other_language_by_its_own_name() {
        let both = [
            ("en".to_string(), "English".to_string()),
            ("ru".to_string(), "Русский".to_string()),
        ];
        let (head, offered) = in_other_languages("places/a.html", "en", &both);
        assert_eq!(
            head,
            "<link rel=\"alternate\" hreflang=\"ru\" href=\"../ru/places/a.html\">\n"
        );
        assert_eq!(offered, "<p class=\"tongues\"><span lang=\"en\">English</span> · <a href=\"../ru/places/a.html\" hreflang=\"ru\" lang=\"ru\">Русский</a></p>\n");
        // from the other language, the same page is one folder further up
        let (head, offered) = in_other_languages("places/a.html", "ru", &both);
        assert!(head.contains("href=\"../../places/a.html\""), "{head}");
        assert!(
            offered.contains("<span lang=\"ru\">Русский</span>"),
            "{offered}"
        );
        // one language only: nothing to offer
        assert_eq!(
            in_other_languages("a.html", "en", &both[..1]),
            Default::default()
        );
    }

    fn parts() -> Vec<Brought> {
        vec![
            Brought {
                title: Some("in progress (1)".into()),
                lines: vec!["claude: r/a - A <b> & co".into()],
                measures: vec![],
                keys: vec![],
                ..Default::default()
            },
            Brought {
                title: Some("waiting (0)".into()),
                lines: vec![],
                measures: vec![],
                keys: vec![],
                ..Default::default()
            },
            Brought {
                title: None,
                lines: vec!["open 2, across 1 stars".into()],
                measures: vec![],
                keys: vec![],
                ..Default::default()
            },
        ]
    }

    #[test]
    fn the_same_focus_is_laid_out_on_each_surface_in_its_own_way() {
        assert_eq!(
            Text.lay_out(&parts()),
            "in progress (1)\n  claude: r/a - A <b> & co\nwaiting (0)\nopen 2, across 1 stars\n"
        );
    }

    #[test]
    fn the_fulldome_is_every_lens_on_a_surface_and_says_which_brought_nothing() {
        let lenses = [
            Brought {
                title: Some("work".into()),
                about: Some("The flow of work.".into()),
                within: parts(),
                ..Default::default()
            },
            Brought {
                title: Some("purpose".into()),
                about: Some("Why a star exists.".into()),
                nothing_because: Some("the lens purpose has no focus yet".into()),
                ..Default::default()
            },
        ];
        let text = Text.whole("this place", &lenses);
        assert!(
            text.starts_with(
                "the fulldome - this place\n\nwork: The flow of work.\nin progress (1)\n"
            ) && text
                .contains("purpose: Why a star exists.\n  (the lens purpose has no focus yet)\n"),
            "{text}"
        );
    }

    #[test]
    fn a_measure_is_one_meaning_shown_by_each_surface_in_its_own_way() {
        let reached = [Brought {
            title: Some("being reached (2)".into()),
            lines: vec!["r - Published: 8 of 9".into(), "r - Begun".into()],
            measures: vec![Some((8.0, 9.0)), None],
            keys: vec![],
            ..Default::default()
        }];
        assert_eq!(bar(8.0, 9.0), "█████████░");
        assert_eq!(
            (bar(0.0, 9.0), bar(12.0, 9.0), bar(1.0, 0.0)),
            (
                "░░░░░░░░░░".to_string(),
                "██████████".to_string(),
                "░░░░░░░░░░".to_string()
            )
        );
        assert_eq!(
            Text.lay_out(&reached),
            "being reached (2)\n  █████████░ r - Published: 8 of 9\n  r - Begun\n"
        );
    }

    #[test]
    fn words_are_fitted_into_a_width_and_nothing_reaches_past_it() {
        assert_eq!(
            fitted("one two three four", 9),
            ["one two", "three", "four"]
        );
        assert_eq!(
            fitted("https://a-very-long-address.example/x", 12),
            ["https://a-ve", "ry-long-addr", "ess.example/", "x"],
            "a word wider than a line is broken where the line ends"
        );
        assert_eq!(fitted("", 9), [""]);
        for width in [1, 2, 7, 40] {
            assert!(
                fitted(
                    "the same light, through a filter █████ shows what the plain view leaves flat",
                    width
                )
                .iter()
                .all(|l| wide(l) <= width),
                "width {width}"
            );
        }
    }

    #[test]
    fn a_screen_fits_every_shape_into_its_volume_and_says_what_it_requires() {
        let parts = [
            Brought {
                title: Some("being reached (2)".into()),
                lines: vec![
                    "argenv - Published and used by everything that starts: 8 of 9".into(),
                    "docs - Begun".into(),
                ],
                measures: vec![Some((8.0, 9.0)), None],
                keys: vec![],
                ..Default::default()
            },
            Brought {
                title: None,
                lines: vec!["95 milestones in 15 stars".into()],
                measures: vec![],
                keys: vec![],
                ..Default::default()
            },
        ];
        let screen = Screen { width: 40 };
        let laid = screen.lay_out(&parts);
        assert_eq!(laid, "being reached (2)\n  █████████░ argenv - Published and used\n             by everything that starts:\n             8 of 9\n  docs - Begun\n95 milestones in 15 stars\n");
        assert!(
            laid.lines().all(|l| wide(l) <= 40),
            "the bar is never broken, and a line's continuation sits under its start"
        );
        assert_eq!(
            screen.requires().unwrap(),
            "40 columns of monospaced characters, unwrapped"
        );
        // the same words in the same order as the text that flows: fitting loses nothing
        let words = |text: &str| {
            text.split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>()
        };
        assert_eq!(words(&laid), words(&Text.lay_out(&parts)));
        // a volume too small for a shape's own beginning cannot be met: the screen says so, in itself
        let cramped = Screen { width: 8 }.lay_out(&parts);
        assert!(
            cramped.contains("this screen is not sound: ")
                && cramped.contains(" line(s) reach past 8 columns"),
            "{cramped}"
        );
        let whole = screen.whole(
            "this place",
            &[Brought {
                title: Some("work".into()),
                about: Some("The flow of work.".into()),
                within: parts.to_vec(),
                ..Default::default()
            }],
        );
        assert!(whole.starts_with("the fulldome - this place\n(laid out for 40 columns of monospaced\ncharacters, unwrapped)\n\nwork: The flow of work.\n") && whole.lines().all(|l| wide(l) <= 40), "{whole}");
    }

    #[test]
    fn the_projection_itself_is_a_surface_for_programs() {
        let parts = [Brought {
            title: Some("next (1)".into()),
            lines: vec!["r/a - A".into()],
            measures: vec![],
            keys: vec!["r/a".into()],
            ..Default::default()
        }];
        let read: Vec<Brought> = serde_json::from_str(&Json.lay_out(&parts)).unwrap();
        assert_eq!(
            read, parts,
            "what is read back is the projection, whole - what each line is about with it"
        );
        let whole: serde_json::Value = serde_json::from_str(&Json.whole(
            "this place",
            &[
                Brought {
                    title: Some("work".into()),
                    about: Some("The flow.".into()),
                    within: parts.to_vec(),
                    ..Default::default()
                },
                Brought {
                    title: Some("purpose".into()),
                    about: Some("Why.".into()),
                    nothing_because: Some("nothing yet".into()),
                    ..Default::default()
                },
            ],
        ))
        .unwrap();
        assert_eq!(
            (
                whole["plane"][0]["within"][0]["keys"][0].as_str(),
                whole["plane"][1]["nothing_because"].as_str()
            ),
            (Some("r/a"), Some("nothing yet"))
        );
    }

    #[test]
    fn a_chain_is_walked_from_where_it_begins_whatever_order_it_was_kept_in() {
        let row = |from: &str, to: &str| -> serde_json::Map<String, serde_json::Value> {
            serde_json::from_value(serde_json::json!({"this": from, "that": to})).unwrap()
        };
        // kept out of order, as a table sorted by name keeps them; g follows from y and from z
        let kept = [
            ("y", "g"),
            ("x", "y z"),
            ("q", "x"),
            ("z", "g"),
            ("g", "h"),
            ("lone", "end"),
        ];
        let chain = Brought {
            title: Some("a red line".into()),
            lines: kept
                .iter()
                .map(|(a, b)| format!("{a} led to {b}"))
                .collect(),
            keys: kept.iter().map(|(a, b)| format!("{a}>{b}")).collect(),
            values: kept.iter().map(|(a, b)| row(a, b)).collect(),
            chain: Some(("this".into(), "that".into())),
            ..Default::default()
        };
        let order: Vec<(&str, usize)> = walked(&chain)
            .into_iter()
            .map(|(at, along)| (chain.lines[at].as_str(), along))
            .collect();
        assert_eq!(
            order,
            [
                ("q led to x", 0),
                ("x led to y z", 1),
                ("y led to g", 2),
                ("g led to h", 3),
                ("z led to g", 2),
                ("lone led to end", 0)
            ],
            "each line once, and what follows it beneath it"
        );
        assert_eq!(Text.lay_out(std::slice::from_ref(&chain)), "a red line\n  q led to x\n    x led to y z\n      y led to g\n        g led to h\n      z led to g\n  lone led to end\n");
        assert!(Screen { width: 20 }
            .lay_out(std::slice::from_ref(&chain))
            .lines()
            .all(|l| wide(l) <= 20));
        // a ring has no beginning: it is still shown whole, once
        let ring = Brought {
            lines: vec!["a to b".into(), "b to a".into()],
            keys: vec!["1".into(), "2".into()],
            values: vec![row("a", "b"), row("b", "a")],
            chain: Some(("this".into(), "that".into())),
            ..Default::default()
        };
        assert_eq!(walked(&ring), [(0, 0), (1, 1)]);
        // a list is as it was written
        assert_eq!(
            walked(&Brought {
                lines: vec!["one".into(), "two".into()],
                keys: vec!["one".into(), "two".into()],
                ..Default::default()
            }),
            [(0, 0), (1, 0)]
        );
    }

    #[test]
    fn a_surface_is_found_by_its_name() {
        assert_eq!(
            surface("text", 80).unwrap().lay_out(&parts()),
            Text.lay_out(&parts())
        );
        assert!(surface("paper", 80)
            .err()
            .unwrap()
            .to_string()
            .contains("there are text, screen and json"));
        // the page is no surface of the program's: a lens says it
        assert!(surface("html", 80).is_err());
    }
}
