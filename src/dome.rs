//! The dome: what a star's shine can be focused onto. A lens says what comes into focus; a dome is the
//! one thing every surface must be able to do with it - lay it out. Plain text, Markdown, a terminal,
//! HTML and CSS, and whatever someone makes later are surfaces: each a dome made real. A lens knows of
//! none of them, so a lens made before a surface existed shows on it as it stands. The fulldome is every
//! lens, in focus, on a dome.

use crate::focus::Brought;
use anyhow::{bail, Result};

/// The port: lays out what a lens brought into focus.
pub trait Dome {
    fn lay_out(&self, parts: &[Brought]) -> String;
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
            for line in &part.lines {
                text.push_str(inset);
                text.push_str(line);
                text.push('\n');
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
                    for line in &part.lines {
                        text.push_str(&format!("- {line}\n"));
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
}

/// The surface of that name.
pub fn surface(name: &str) -> Result<Box<dyn Dome>> {
    Ok(match name {
        "text" => Box::new(Text),
        "markdown" => Box::new(Markdown),
        other => bail!("no surface is called {other}: there are text and markdown (and the terminal: ocoeco fulldome)"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts() -> Vec<Brought> {
        vec![
            Brought {
                title: Some("in progress (1)".into()),
                lines: vec!["claude: r/a - A".into()],
            },
            Brought {
                title: Some("waiting (0)".into()),
                lines: vec![],
            },
            Brought {
                title: None,
                lines: vec!["open 2, across 1 stars".into()],
            },
        ]
    }

    #[test]
    fn the_same_focus_is_laid_out_on_each_surface_in_its_own_way() {
        assert_eq!(
            Text.lay_out(&parts()),
            "in progress (1)\n  claude: r/a - A\nwaiting (0)\nopen 2, across 1 stars\n"
        );
        assert_eq!(Markdown.lay_out(&parts()), "### in progress (1)\n\n- claude: r/a - A\n\n### waiting (0)\n\nNothing.\n\nopen 2, across 1 stars\n");
    }

    #[test]
    fn a_surface_is_found_by_its_name() {
        assert_eq!(
            surface("markdown").unwrap().lay_out(&parts()),
            Markdown.lay_out(&parts())
        );
        assert!(surface("paper")
            .err()
            .unwrap()
            .to_string()
            .contains("there are text and markdown"));
    }
}
