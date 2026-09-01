//! YAML frontmatter parsing for mdBook chapters.

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Deserializer};

/// Per-chapter feed inclusion control, set via the `feed` frontmatter key.
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
    // Handle `date --rfc-3339=second` output format (space instead of T separator).
    if let Ok(dt) = DateTime::parse_from_str(&date_str, "%Y-%m-%d %H:%M:%S%z") {
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
    title: Option<String>,
    #[serde(deserialize_with = "deserialize_date", default)]
    date: Option<DateTime<Utc>>,
    author: Option<String>,
    description: Option<String>,
    #[serde(default)]
    feed: Option<FeedVisibility>,
    tags: Option<Vec<String>>,
    lang: Option<String>,
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
    /// Tags/categories for this chapter.
    pub tags: Vec<String>,
    /// BCP-47 language tag for this chapter.
    pub lang: Option<String>,
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
    let title = fm_title
        .filter(|t| !t.is_empty())
        .or_else(|| first_h1(body))
        .unwrap_or_else(|| fallback.to_string());

    if title.is_empty() {
        "Untitled".to_string()
    } else {
        title
    }
}

/// Split raw markdown into an optional YAML frontmatter block and a body.
///
/// Returns `(Some(yaml), body)` when a valid frontmatter block is found
/// , or `(None, body)` when there is no frontmatter or the opening `---`
///  was never closed.
fn split_frontmatter(raw: &str) -> (Option<String>, String) {
    let mut lines = raw.lines();

    // Only treat the file as having frontmatter if the very first line is `---`.
    // This prevents horizontal rules later in the document from being mistaken
    // for the closing delimiter.
    let first = lines.next().unwrap_or("");
    if first.trim() != "---" {
        // No frontmatter, put the first line back and treat everything as body.
        let body = std::iter::once(first)
            .chain(lines)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        return (None, body);
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
        return (None, body);
    }

    let body = lines.collect::<Vec<_>>().join("\n") + "\n";
    (Some(yaml), body)
}

/// Parse frontmatter and body from raw Markdown.
///
/// Calls [`split_frontmatter`] to extract the YAML block, then interprets
/// it into a [`FrontMatter`] struct. If no frontmatter is present or parsing
/// fails, falls back gracefully using `title_hint` and `fallback_date`.
#[allow(clippy::option_if_let_else)]
#[must_use]
pub fn parse_frontmatter(
    raw: &str,
    title_hint: &str,
    fallback_date: Option<DateTime<Utc>>,
    strict: bool,
) -> (FrontMatter, String) {
    let (yaml_opt, body) = split_frontmatter(raw);
    let yaml_opt = yaml_opt.filter(|y| !y.trim().is_empty());

    let fm = match yaml_opt {
        None => FrontMatter {
            title: resolve_title(None, &body, title_hint),
            date: fallback_date,
            author: None,
            description: None,
            feed: None,
            tags: Vec::new(),
            lang: None,
        },
        Some(yaml) => match yaml_serde::from_str::<RawFrontmatter>(&yaml) {
            Ok(raw_fm) => FrontMatter {
                title: resolve_title(raw_fm.title, &body, title_hint),
                date: raw_fm.date.or(fallback_date),
                author: raw_fm.author,
                description: raw_fm.description,
                feed: raw_fm.feed,
                tags: raw_fm.tags.unwrap_or_default(),
                lang: raw_fm.lang,
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
                    tags: Vec::new(),
                    lang: None,
                }
            }
        },
    };

    (fm, body)
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_h1_finds_top_level_heading() {
        let md = "# My Title\n\nSome content.";
        assert_eq!(first_h1(md), Some("My Title".to_string()));
    }

    #[test]
    fn first_h1_ignores_h2_and_deeper() {
        let md = "## Section\n\n### Subsection\n\nContent.";
        assert_eq!(first_h1(md), None);
    }

    #[test]
    fn first_h1_skips_preceding_content() {
        let md = "Some intro text.\n\n# Actual Title\n\nBody.";
        assert_eq!(first_h1(md), Some("Actual Title".to_string()));
    }

    #[test]
    fn first_h1_trims_trailing_whitespace() {
        let md = "# Padded Title   \n\nContent.";
        assert_eq!(first_h1(md), Some("Padded Title".to_string()));
    }

    #[test]
    fn first_h1_empty_document_returns_none() {
        assert_eq!(first_h1(""), None);
    }

    #[test]
    fn resolve_title_prefers_frontmatter_title() {
        let body = "# Heading Title\n\nContent.";
        let result = resolve_title(Some("FM Title".to_string()), body, "fallback");
        assert_eq!(result, "FM Title");
    }

    #[test]
    fn resolve_title_falls_back_to_h1() {
        let body = "# Heading Title\n\nContent.";
        let result = resolve_title(None, body, "fallback");
        assert_eq!(result, "Heading Title");
    }

    #[test]
    fn resolve_title_falls_back_to_hint() {
        let body = "No heading here, just text.";
        let result = resolve_title(None, body, "my-fallback");
        assert_eq!(result, "my-fallback");
    }

    #[test]
    fn resolve_title_empty_fm_title_falls_through_to_h1() {
        let body = "# Real Title\n\nContent.";
        let result = resolve_title(Some(String::new()), body, "fallback");
        assert_eq!(result, "Real Title");
    }

    #[test]
    fn parse_frontmatter_no_frontmatter_returns_whole_body() {
        let raw = "# Plain Chapter\n\nJust content, no frontmatter.";
        let (fm, body) = parse_frontmatter(raw, "hint", None, false);
        assert_eq!(fm.title, "Plain Chapter");
        assert!(body.contains("Just content"));
        assert!(body.contains("# Plain Chapter"));
    }

    #[test]
    fn parse_frontmatter_full_yaml_block() {
        let raw = "---\ntitle: My Post\ndate: 2024-06-01\nauthor: Alice\ndescription: A summary.\n---\n\nBody content.";
        let (fm, body) = parse_frontmatter(raw, "hint", None, false);
        assert_eq!(fm.title, "My Post");
        assert_eq!(fm.author.as_deref(), Some("Alice"));
        assert_eq!(fm.description.as_deref(), Some("A summary."));
        assert!(fm.date.is_some());
        assert!(body.contains("Body content."));
    }

    #[test]
    fn parse_frontmatter_date_rfc3339() {
        let raw = "---\ndate: 2024-03-15T12:00:00Z\n---\n\nContent.";
        let (fm, _) = parse_frontmatter(raw, "hint", None, false);
        let date = fm.date.expect("should have parsed date");
        assert_eq!(date.format("%Y-%m-%d").to_string(), "2024-03-15");
    }

    #[test]
    fn parse_frontmatter_date_yyyy_mm_dd() {
        let raw = "---\ndate: 2023-11-30\n---\n\nContent.";
        let (fm, _) = parse_frontmatter(raw, "hint", None, false);
        let date = fm.date.expect("should have parsed date");
        assert_eq!(date.format("%Y-%m-%d").to_string(), "2023-11-30");
    }

    #[test]
    fn parse_frontmatter_title_from_h1_when_no_yaml_title() {
        let raw = "---\ndate: 2024-01-01\n---\n\n# Heading From Body\n\nContent.";
        let (fm, _) = parse_frontmatter(raw, "hint", None, false);
        assert_eq!(fm.title, "Heading From Body");
    }

    #[test]
    fn parse_frontmatter_title_hint_fallback() {
        let raw = "---\ndate: 2024-01-01\n---\n\nNo heading here.";
        let (fm, _) = parse_frontmatter(raw, "my-chapter", None, false);
        assert_eq!(fm.title, "my-chapter");
    }

    #[test]
    fn parse_frontmatter_unclosed_delimiter_treated_as_body() {
        // Opening `---` but no closing one — entire file is body.
        let raw = "---\ntitle: Orphaned\ndate: 2024-01-01\n";
        let (fm, body) = parse_frontmatter(raw, "fallback", None, false);
        // Title should come from the body (which contains the raw `---` and
        // yaml) or fall back to hint; importantly it should NOT crash.
        let _ = fm.title; // just check it doesn't panic
        assert!(body.contains("---"));
    }

    #[test]
    fn parse_frontmatter_empty_yaml_block() {
        let raw = "---\n---\n\n# Body Heading\n\nContent.";
        let (fm, body) = parse_frontmatter(raw, "hint", None, false);
        // Empty YAML block => title from h1.
        assert_eq!(fm.title, "Body Heading");
        assert!(body.contains("Content."));
    }

    #[test]
    fn parse_frontmatter_feed_include() {
        let raw = "---\ntitle: Included\nfeed: include\n---\n\nContent.";
        let (fm, _) = parse_frontmatter(raw, "hint", None, false);
        assert_eq!(fm.feed, Some(FeedVisibility::Include));
    }

    #[test]
    fn parse_frontmatter_feed_exclude() {
        let raw = "---\ntitle: Hidden\nfeed: exclude\n---\n\nContent.";
        let (fm, _) = parse_frontmatter(raw, "hint", None, false);
        assert_eq!(fm.feed, Some(FeedVisibility::Exclude));
    }

    #[test]
    fn parse_frontmatter_no_feed_key_is_none() {
        let raw = "---\ntitle: Normal\n---\n\nContent.";
        let (fm, _) = parse_frontmatter(raw, "hint", None, false);
        assert_eq!(fm.feed, None);
    }

    #[test]
    fn parse_frontmatter_fallback_date_used_when_no_yaml_date() {
        use chrono::TimeZone;
        let fallback = chrono::Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
        let raw = "---\ntitle: No Date\n---\n\nContent.";
        let (fm, _) = parse_frontmatter(raw, "hint", Some(fallback), false);
        assert_eq!(fm.date, Some(fallback));
    }

    #[test]
    fn parse_frontmatter_yaml_date_overrides_fallback() {
        use chrono::TimeZone;
        let fallback = chrono::Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
        let raw = "---\ntitle: Has Date\ndate: 2024-06-15\n---\n\nContent.";
        let (fm, _) = parse_frontmatter(raw, "hint", Some(fallback), false);
        let date = fm.date.expect("should have a date");
        // Should be 2024-06-15, not the 2020 fallback.
        assert_eq!(date.format("%Y-%m-%d").to_string(), "2024-06-15");
    }

    #[test]
    fn parse_frontmatter_invalid_yaml_non_strict_warns_and_continues() {
        // Malformed YAML (unclosed bracket) - should not panic in non-strict mode.
        let raw = "---\ntitle: [unclosed\n---\n\n# Heading\n\nContent.";
        let (fm, body) = parse_frontmatter(raw, "fallback", None, false);
        // Title falls back to h1 or hint.
        assert_ne!(fm.title, "");
        assert!(body.contains("Content."));
    }

    #[test]
    fn deserialize_date_accepts_rfc3339() {
        let raw = "---\ndate: 2024-07-04T00:00:00Z\n---\n\nContent.";
        let (fm, _) = parse_frontmatter(raw, "hint", None, false);
        assert!(fm.date.is_some());
        assert_eq!(
            fm.date.unwrap().format("%Y-%m-%d").to_string(),
            "2024-07-04"
        );
    }

    #[test]
    fn deserialize_date_accepts_naive_date() {
        let raw = "---\ndate: 2024-07-04\n---\n\nContent.";
        let (fm, _) = parse_frontmatter(raw, "hint", None, false);
        assert!(fm.date.is_some());
        assert_eq!(
            fm.date.unwrap().format("%Y-%m-%d").to_string(),
            "2024-07-04"
        );
    }
}
