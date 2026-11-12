//! The board: a view over every project's backlog, never a list of its own - as GitHub Projects and
//! Linear keep each item once and show it many ways. It shows the flow Kanban asks to be seen: what is
//! in progress (by whom, its next step, for how long), what waits and on what, what is next; and what
//! needs attention - a driver holding too much at once, work in progress with no next step, work that
//! has not moved for a week (Reinertsen: queues and ageing are the hidden cost). Decision
//! 2026-11-12-the-board in this repository's data.

use crate::join::rows;
use crate::mark::Mark;
use std::path::{Path, PathBuf};

/// One backlog item, wherever it lives.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub project: String,
    pub id: String,
    pub title: String,
    pub status: String,
    pub driver: String,
    pub next: String,
    pub since: String,
    pub waits_on: String,
}

/// How many items one driver may hold in progress before the board says so.
pub const AT_ONCE: usize = 3;
/// How many days an item may sit in progress or waiting before the board says so.
pub const STALE_DAYS: i64 = 7;

/// Every marked project under `root`, two levels down (owner/repository), as the lab keeps them.
pub fn projects(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for owner in std::fs::read_dir(root).into_iter().flatten().flatten() {
        for repo in std::fs::read_dir(owner.path())
            .into_iter()
            .flatten()
            .flatten()
        {
            if repo.path().join("oco.toml").is_file() {
                found.push(repo.path());
            }
        }
    }
    found.sort();
    found
}

/// The backlog items of the projects at `dirs`.
pub fn items(dirs: &[PathBuf]) -> Vec<Item> {
    let mut all = Vec::new();
    for dir in dirs {
        let Ok(text) = std::fs::read_to_string(dir.join("oco.toml")) else {
            continue;
        };
        let Ok(mark) = Mark::parse(&text) else {
            continue;
        };
        let project = mark
            .project
            .id
            .rsplit('/')
            .next()
            .unwrap_or(&mark.project.id)
            .to_string();
        for row in rows(dir, &mark, "backlog") {
            let field = |k: &str| row[k].as_str().unwrap_or("").to_string();
            all.push(Item {
                project: project.clone(),
                id: field("id"),
                title: field("title"),
                status: field("status"),
                driver: field("driver"),
                next: field("next"),
                since: field("since"),
                waits_on: field("waits_on"),
            });
        }
    }
    all
}

/// Days since 1970-01-01 of a date written YYYY-MM-DD (Howard Hinnant's days_from_civil).
fn days(date: &str) -> Option<i64> {
    let n = |r: std::ops::Range<usize>| date.get(r)?.parse::<i64>().ok();
    let (year, month, day) = (n(0..4)?, n(5..7)?, n(8..10)?);
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    Some(era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468)
}

fn age(item: &Item, today: &str) -> Option<i64> {
    Some(days(today)? - days(&item.since)?)
}

/// The board, in words, as of `today` (YYYY-MM-DD).
pub fn board(items: &[Item], today: &str) -> String {
    fn of<'a>(items: &'a [Item], status: &'a str) -> impl Iterator<Item = &'a Item> {
        items.iter().filter(move |i| i.status == status)
    }
    let name = |i: &Item| format!("{}/{}", i.project, i.id);
    let mut text = String::new();
    let doing: Vec<&Item> = of(items, "in-progress").collect();
    text.push_str(&format!("in progress ({})\n", doing.len()));
    let mut drivers: Vec<&str> = doing
        .iter()
        .map(|i| {
            if i.driver.is_empty() {
                "nobody"
            } else {
                i.driver.as_str()
            }
        })
        .collect();
    drivers.sort();
    drivers.dedup();
    for d in &drivers {
        for i in doing.iter().filter(|i| {
            (if i.driver.is_empty() {
                "nobody"
            } else {
                i.driver.as_str()
            }) == *d
        }) {
            let since = age(i, today)
                .map(|n| format!(", {n} day(s)"))
                .unwrap_or_default();
            let next = if i.next.is_empty() {
                String::new()
            } else {
                format!(" - next: {}", i.next)
            };
            text.push_str(&format!("  {d}: {} - {}{next}{since}\n", name(i), i.title));
        }
    }
    let waiting: Vec<&Item> = of(items, "waiting").collect();
    if !waiting.is_empty() {
        text.push_str(&format!("waiting ({})\n", waiting.len()));
        for i in &waiting {
            let on = if i.waits_on.is_empty() {
                "nothing said".to_string()
            } else {
                i.waits_on.clone()
            };
            text.push_str(&format!("  {} - {} - waits on {on}\n", name(i), i.title));
        }
    }
    let next: Vec<&Item> = of(items, "next").collect();
    if !next.is_empty() {
        text.push_str(&format!("next ({})\n", next.len()));
        for i in &next {
            text.push_str(&format!("  {} - {}\n", name(i), i.title));
        }
    }
    let mut attention = Vec::new();
    for d in &drivers {
        let held = doing.iter().filter(|i| i.driver == *d).count();
        if *d != "nobody" && held > AT_ONCE {
            attention.push(format!("{d} holds {held} at once (more than {AT_ONCE})"));
        }
    }
    for i in &doing {
        if i.driver.is_empty() {
            attention.push(format!("{} is in progress, held by nobody", name(i)));
        }
        if i.next.is_empty() {
            attention.push(format!("{} is in progress with no next step", name(i)));
        }
    }
    for i in doing.iter().chain(waiting.iter()) {
        if age(i, today).is_some_and(|n| n > STALE_DAYS) {
            attention.push(format!(
                "{} has not moved for {} days",
                name(i),
                age(i, today).unwrap_or(0)
            ));
        }
    }
    if !attention.is_empty() {
        text.push_str(&format!("needs attention ({})\n", attention.len()));
        for a in attention {
            text.push_str(&format!("  {a}\n"));
        }
    }
    let projects: std::collections::BTreeSet<&str> =
        items.iter().map(|i| i.project.as_str()).collect();
    text.push_str(&format!(
        "open {} and parked {}, across {} projects\n",
        of(items, "open").count(),
        of(items, "parked").count(),
        projects.len()
    ));
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, status: &str, driver: &str, next: &str, since: &str) -> Item {
        Item {
            project: "p".into(),
            id: id.into(),
            title: format!("{id} title"),
            status: status.into(),
            driver: driver.into(),
            next: next.into(),
            since: since.into(),
            waits_on: String::new(),
        }
    }

    #[test]
    fn the_board_shows_the_flow_and_what_needs_attention() {
        let mut waits = item("w", "waiting", "claude", "", "2026-11-01");
        waits.waits_on = "the author's token".into();
        let all = vec![
            item(
                "a",
                "in-progress",
                "claude",
                "build the board",
                "2026-11-12",
            ),
            item("b", "in-progress", "", "", "2026-11-02"),
            waits,
            item("n", "next", "", "", ""),
            item("o", "open", "", "", ""),
            item("d", "done", "claude", "", "2026-11-12"),
        ];
        let text = board(&all, "2026-11-12");
        assert!(text.starts_with("in progress (2)\n  claude: p/a - a title - next: build the board, 0 day(s)\n  nobody: p/b - b title, 10 day(s)\n"), "{text}");
        assert!(
            text.contains("waiting (1)\n  p/w - w title - waits on the author's token\n"),
            "{text}"
        );
        assert!(text.contains("next (1)\n  p/n - n title\n"), "{text}");
        for want in [
            "p/b is in progress, held by nobody",
            "p/b is in progress with no next step",
            "p/b has not moved for 10 days",
            "p/w has not moved for 11 days",
        ] {
            assert!(text.contains(want), "{want}: {text}");
        }
        assert!(
            text.ends_with("open 1 and parked 0, across 1 projects\n"),
            "{text}"
        );
    }

    #[test]
    fn a_driver_holding_too_much_is_said() {
        let all: Vec<Item> = (0..4)
            .map(|n| item(&n.to_string(), "in-progress", "claude", "x", "2026-11-12"))
            .collect();
        assert!(board(&all, "2026-11-12").contains("claude holds 4 at once (more than 3)"));
    }

    #[test]
    fn a_date_is_a_count_of_days() {
        assert_eq!(days("1970-01-01"), Some(0));
        assert_eq!(
            days("2026-11-12")
                .zip(days("2026-11-02"))
                .map(|(a, b)| a - b),
            Some(10)
        );
        assert_eq!(days("soon"), None);
    }
}
