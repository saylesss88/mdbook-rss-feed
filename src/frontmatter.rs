//! YAML frontmatter parsing for mdBook chapters.

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Deserializer};

/// Per-chapter feed inclusion control, set via the `feed` frontmatter key.
///
/// ```yaml
/// ---
/// feed: include   # always include this chapter regardless of default-behavior
/// feed: exclude   # always exclude this chapter regardless of default-behavior
/// ---
/// ```
///
/// When absent, the chapter follows the book-level `default-behavior` setting
/// (`include-all` by default).
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FeedVisibility {
    Include,
    Exclude,
}

/// Parse front-matter date formats (RFC3339 or `YYYY-MM-DD`).
pub fn deserialize_date<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    let Some(date_str) = s else {
        return Ok(None);
    };

    if let Ok(dt) = DateTime::parse_from_rfc3339(&date_str) {
        return Ok(Some(dt.with_timezone(&Utc)));
    }
    if let Ok(nd) = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
        // NaiveDate::and_hms_opt(0, 0, 0) only fails for an invalid hms,
        // which 0,0,0 never is, so this expect documents an invariant
        // rather than a real failure mode.
        let midnight = nd
            .and_hms_opt(0, 0, 0)
            .expect("midnight is always a valid time");
        return Ok(Some(Utc.from_utc_datetime(&midnight)));
    }

    Err(serde::de::Error::custom(format!(
        "invalid date '{date_str}': expected RFC3339 or YYYY-MM-DD"
    )))
}

/// Raw deserialization target. `title` is optional so that a chapter with
/// only `date:` (and no `title:`) doesn't cause a hard parse failure.
#[derive(Debug, Deserialize, Clone)]
struct RawFrontmatter {
    pub title: Option<String>,
    #[serde(deserialize_with = "deserialize_date", default)]
    pub date: Option<DateTime<Utc>>,
    pub author: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub feed: Option<FeedVisibility>,
}

/// Parsed YAML frontmatter for a single chapter.
///
/// Fields are used for feed metadata:
/// - `title`: item title shown in the feed.
/// - `date`: publish date for sorting and `pubDate` (RFC3339 or `YYYY-MM-DD`).
/// - `author`: optional item author.
/// - `description`: optional summary/preview override.
/// - `feed`: per-chapter inclusion override (`include` or `exclude`).
#[derive(Debug, Clone)]
pub struct FrontMatter {
    pub title: String,
    pub date: Option<DateTime<Utc>>,
    pub author: Option<String>,
    /// User-supplied summary, used as a fallback preview source.
    pub description: Option<String>,
    /// Per-chapter feed inclusion override. When absent, the chapter follows
    /// the book-level `default-behavior` (`include-all` by default).
    pub feed: Option<FeedVisibility>,
}

/// Extract the text of the first `# Heading` in a Markdown body.
///
/// Scans lines for the first one starting with `# ` and returns the heading
/// text without the `#` prefix. Returns `None` if no level-1 heading is found.
#[must_use]
pub fn first_h1(content: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let trimmed = line.trim();
        trimmed.strip_prefix("# ").map(|t| t.trim().to_string())
    })
}

/// Resolve a title using the priority chain:
///   1. `title` field in frontmatter YAML
///   2. First `# Heading` in the chapter body
///   3. `fallback` (chapter name from SUMMARY.md / file stem)
#[must_use]
pub fn resolve_title(fm_title: Option<String>, body: &str, fallback: &str) -> String {
    fm_title
        .filter(|t| !t.is_empty())
        .or_else(|| first_h1(body))
        .unwrap_or_else(|| fallback.to_string())
}

/// Parse a YAML frontmatter block and body from raw Markdown.
///
/// Resolution priority for `title`:
///   1. `title:` in YAML frontmatter
///   2. First `# Heading` in the body
///   3. `title_hint` (chapter name from SUMMARY.md or file stem)
///
/// A warning is printed to stderr when YAML is present but fails to parse.
/// If `strict` is `true`, the process exits with code 1 instead of warning.
pub fn parse_frontmatter(
    raw: &str,
    title_hint: &str,
    fallback_date: Option<DateTime<Utc>>,
    strict: bool,
) -> (FrontMatter, String) {
    let mut lines = raw.lines();

    // Only treat the file as having frontmatter if the very first line is `---`.
    // This prevents horizontal rules later in the document from being mistaken
    // for the closing delimiter.
    let first = lines.next().unwrap_or("");
    if first.trim() != "---" {
        // No frontmatter — put the first line back and treat everything as body.
        let body = std::iter::once(first)
            .chain(lines)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        return (
            FrontMatter {
                title: resolve_title(None, &body, title_hint),
                date: fallback_date,
                author: None,
                description: None,
                feed: None,
            },
            body,
        );
    }

    // First line was `---` read YAML until closing `---`
    let mut yaml = String::new();
    let mut closed = false;
    for line in lines.by_ref() {
        if line.trim() == "---" {
            closed = true;
            break;
        }
        yaml.push_str(line);
        yaml.push('\n');
    }

    // If we never found the closing `---`, treat the whole file as body.
    if !closed {
        let body = std::iter::once("---")
            .chain(std::iter::once(yaml.as_str()))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        return (
            FrontMatter {
                title: resolve_title(None, &body, title_hint),
                date: fallback_date,
                author: None,
                description: None,
                feed: None,
            },
            body,
        );
    }

    let body = lines.collect::<Vec<_>>().join("\n") + "\n";

    let fm = if yaml.trim().is_empty() {
        // No frontmatter block at all — derive title from heading or hint.
        FrontMatter {
            title: resolve_title(None, &body, title_hint),
            date: fallback_date,
            author: None,
            description: None,
            feed: None,
        }
    } else {
        match yaml_serde::from_str::<RawFrontmatter>(&yaml) {
            Ok(raw_fm) => FrontMatter {
                title: resolve_title(raw_fm.title, &body, title_hint),
                date: raw_fm.date.or(fallback_date),
                author: raw_fm.author,
                description: raw_fm.description,
                feed: raw_fm.feed,
            },
            Err(e) => {
                let msg = format!(
                    "mdbook-rss-feed: failed to parse frontmatter for \
                     '{title_hint}': {e}"
                );
                if strict {
                    eprintln!("error: {msg}");
                    std::process::exit(1);
                }
                eprintln!("warning: {msg} (use strict = true to fail the build)");
                FrontMatter {
                    title: resolve_title(None, &body, title_hint),
                    date: fallback_date,
                    author: None,
                    description: None,
                    feed: None,
                }
            }
        }
    };

    (fm, body)
}
