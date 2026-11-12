//! The dome: what a star's shine can be focused onto. A lens says what comes into focus; a dome is the
//! one thing every surface must be able to do with it - lay it out. Plain text, Markdown, HTML and CSS,
//! a terminal, and whatever someone makes later are surfaces: each a dome made real. A lens knows of
//! none of them, so a lens made before a surface existed shows on it as it stands. The fulldome is every
//! lens, in focus, on a dome.

use crate::focus::Brought;
use anyhow::{bail, Result};

/// One lens in the fulldome: its id, what it says of itself, and what it brought into focus - or why
/// it brought nothing (it has no focus yet, or could not come into focus).
pub struct InDome {
    pub id: String,
    pub says: String,
    pub brought: std::result::Result<Vec<Brought>, String>,
}

/// The port: lays out what a lens brought into focus, and the fulldome - every lens together.
pub trait Dome {
    fn lay_out(&self, parts: &[Brought]) -> String;
    /// Every lens, in focus, on this surface; `scale` says what is in view.
    fn fulldome(&self, scale: &str, lenses: &[InDome]) -> String;
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

/// The plain-text surface: a heading on its own line, its lines beneath it, set in by two spaces.
pub struct Text;

impl Dome for Text {
    fn lay_out(&self, parts: &[Brought]) -> String {
        let mut text = String::new();
        for part in parts {
            let inset = if part.title.is_some() { "  " } else { "" };
            if let Some(title) = &part.title {
                text.push_str(title);
                text.push('\n');
            }
            for (at, line) in part.lines.iter().enumerate() {
                text.push_str(inset);
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

    fn fulldome(&self, scale: &str, lenses: &[InDome]) -> String {
        let mut text = format!("the fulldome - {scale}\n");
        for lens in lenses {
            text.push_str(&format!("\n{}: {}\n", lens.id, lens.says));
            match &lens.brought {
                Ok(parts) => text.push_str(&self.lay_out(parts)),
                Err(why) => text.push_str(&format!("  ({why})\n")),
            }
        }
        text
    }
}

/// The Markdown surface: a heading as a heading, its lines as a list; a part with no heading as plain
/// lines.
pub struct Markdown;

impl Dome for Markdown {
    fn lay_out(&self, parts: &[Brought]) -> String {
        let mut text = String::new();
        for part in parts {
            if !text.is_empty() {
                text.push('\n');
            }
            match &part.title {
                Some(title) => {
                    text.push_str(&format!("### {title}\n\n"));
                    for (at, line) in part.lines.iter().enumerate() {
                        match part.measures.get(at).copied().flatten() {
                            Some((so_much, of)) => {
                                text.push_str(&format!("- `{}` {line}\n", bar(so_much, of)))
                            }
                            None => text.push_str(&format!("- {line}\n")),
                        }
                    }
                    if part.lines.is_empty() {
                        text.push_str("Nothing.\n");
                    }
                }
                None => {
                    for line in &part.lines {
                        text.push_str(&format!("{line}\n"));
                    }
                }
            }
        }
        text
    }

    fn fulldome(&self, scale: &str, lenses: &[InDome]) -> String {
        let mut text = format!("# The fulldome\n\n{scale}\n");
        for lens in lenses {
            text.push_str(&format!("\n## {}\n\n{}\n\n", lens.id, lens.says));
            match &lens.brought {
                Ok(parts) => text.push_str(&self.lay_out(parts)),
                Err(why) => text.push_str(&format!("*{why}*\n")),
            }
        }
        text
    }
}

/// Text made safe to stand inside HTML.
pub fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The HTML and CSS surface: each part a section - its heading, and its lines as a list.
pub struct Html;

/// How the fulldome's page looks: quiet, readable in light and dark, nothing fetched from anywhere.
const STYLE: &str = "body{font:16px/1.5 system-ui,sans-serif;max-width:60rem;margin:2rem auto;padding:0 1rem;color:#1c1c1c;background:#fcfcfa}h1{font-size:1.6rem}h2{margin-top:2.2rem;border-bottom:1px solid #8884;padding-bottom:.2rem}h3{font-size:1rem;margin:1.2rem 0 .3rem}ul{margin:.2rem 0;padding-left:1.2rem}li{margin:.15rem 0}.says,.scale,.none{color:#555}.none{font-style:italic}@media(prefers-color-scheme:dark){body{color:#e8e8e4;background:#16171a}.says,.scale,.none{color:#a0a0a0}}";

impl Dome for Html {
    fn lay_out(&self, parts: &[Brought]) -> String {
        let mut text = String::new();
        for part in parts {
            text.push_str("<section>\n");
            match &part.title {
                Some(title) => {
                    text.push_str(&format!("<h3>{}</h3>\n", escaped(title)));
                    if part.lines.is_empty() {
                        text.push_str("<p class=\"none\">Nothing.</p>\n");
                    } else {
                        text.push_str("<ul>\n");
                        for (at, line) in part.lines.iter().enumerate() {
                            match part.measures.get(at).copied().flatten() {
                                Some((so_much, of)) => text.push_str(&format!("<li><meter value=\"{so_much}\" max=\"{of}\">{so_much} of {of}</meter> {}</li>\n", escaped(line))),
                                None => text.push_str(&format!("<li>{}</li>\n", escaped(line))),
                            }
                        }
                        text.push_str("</ul>\n");
                    }
                }
                None => {
                    for line in &part.lines {
                        text.push_str(&format!("<p>{}</p>\n", escaped(line)));
                    }
                }
            }
            text.push_str("</section>\n");
        }
        text
    }

    fn fulldome(&self, scale: &str, lenses: &[InDome]) -> String {
        let mut text = format!("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>The fulldome</title>\n<style>{STYLE}</style>\n</head>\n<body>\n<h1>The fulldome</h1>\n<p class=\"scale\">{}</p>\n", escaped(scale));
        for lens in lenses {
            text.push_str(&format!(
                "<h2 id=\"{}\">{}</h2>\n<p class=\"says\">{}</p>\n",
                escaped(&lens.id),
                escaped(&lens.id),
                escaped(&lens.says)
            ));
            match &lens.brought {
                Ok(parts) => text.push_str(&self.lay_out(parts)),
                Err(why) => text.push_str(&format!("<p class=\"none\">{}</p>\n", escaped(why))),
            }
        }
        text.push_str("</body>\n</html>\n");
        text
    }
}

/// The surface of that name.
pub fn surface(name: &str) -> Result<Box<dyn Dome>> {
    Ok(match name {
        "text" => Box::new(Text),
        "markdown" => Box::new(Markdown),
        "html" => Box::new(Html),
        other => bail!("no surface is called {other}: there are text, markdown and html (and the terminal: ocoeco fulldome)"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts() -> Vec<Brought> {
        vec![
            Brought {
                title: Some("in progress (1)".into()),
                lines: vec!["claude: r/a - A <b> & co".into()],
                measures: vec![],
                keys: vec![],
            },
            Brought {
                title: Some("waiting (0)".into()),
                lines: vec![],
                measures: vec![],
                keys: vec![],
            },
            Brought {
                title: None,
                lines: vec!["open 2, across 1 stars".into()],
                measures: vec![],
                keys: vec![],
            },
        ]
    }

    #[test]
    fn the_same_focus_is_laid_out_on_each_surface_in_its_own_way() {
        assert_eq!(
            Text.lay_out(&parts()),
            "in progress (1)\n  claude: r/a - A <b> & co\nwaiting (0)\nopen 2, across 1 stars\n"
        );
        assert_eq!(Markdown.lay_out(&parts()), "### in progress (1)\n\n- claude: r/a - A <b> & co\n\n### waiting (0)\n\nNothing.\n\nopen 2, across 1 stars\n");
        let html = Html.lay_out(&parts());
        assert!(html.contains("<h3>in progress (1)</h3>\n<ul>\n<li>claude: r/a - A &lt;b&gt; &amp; co</li>\n</ul>"), "what a row holds cannot become markup:\n{html}");
        assert!(
            html.contains("<h3>waiting (0)</h3>\n<p class=\"none\">Nothing.</p>")
                && html.contains("<p>open 2, across 1 stars</p>"),
            "{html}"
        );
    }

    #[test]
    fn the_fulldome_is_every_lens_on_a_surface_and_says_which_brought_nothing() {
        let lenses = [
            InDome {
                id: "work".into(),
                says: "The flow of work.".into(),
                brought: Ok(parts()),
            },
            InDome {
                id: "purpose".into(),
                says: "Why a star exists.".into(),
                brought: Err("the lens purpose has no focus yet".into()),
            },
        ];
        let text = Text.fulldome("this place", &lenses);
        assert!(
            text.starts_with(
                "the fulldome - this place\n\nwork: The flow of work.\nin progress (1)\n"
            ) && text
                .contains("purpose: Why a star exists.\n  (the lens purpose has no focus yet)\n"),
            "{text}"
        );
        let page = Html.fulldome("every place observed", &lenses);
        assert!(
            page.starts_with("<!doctype html>")
                && page.ends_with("</body>\n</html>\n")
                && page.contains("<h2 id=\"work\">work</h2>")
                && page.contains("<p class=\"none\">the lens purpose has no focus yet</p>"),
            "{page}"
        );
        assert_eq!(
            page.matches("<section>").count(),
            page.matches("</section>").count()
        );
        assert!(Markdown
            .fulldome("this place", &lenses)
            .contains("## purpose\n\nWhy a star exists.\n\n*the lens purpose has no focus yet*\n"));
    }

    #[test]
    fn a_measure_is_one_meaning_shown_by_each_surface_in_its_own_way() {
        let reached = [Brought {
            title: Some("being reached (2)".into()),
            lines: vec!["r - Published: 8 of 9".into(), "r - Begun".into()],
            measures: vec![Some((8.0, 9.0)), None],
            keys: vec![],
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
        assert!(Markdown
            .lay_out(&reached)
            .contains("- `█████████░` r - Published: 8 of 9\n- r - Begun\n"));
        assert!(Html.lay_out(&reached).contains("<li><meter value=\"8\" max=\"9\">8 of 9</meter> r - Published: 8 of 9</li>\n<li>r - Begun</li>"));
    }

    #[test]
    fn a_surface_is_found_by_its_name() {
        assert_eq!(
            surface("html").unwrap().lay_out(&parts()),
            Html.lay_out(&parts())
        );
        assert!(surface("paper")
            .err()
            .unwrap()
            .to_string()
            .contains("there are text, markdown and html"));
    }
}
