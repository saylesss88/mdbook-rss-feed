//! YAML frontmatter parsing for mdBook chapters.

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Deserializer};

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

    Ok(None)
}

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

/// Parsed YAML frontmatter for a single chapter.
///
/// Fields are used for feed metadata:
/// - `title`: item title shown in the feed.
/// - `date`: publish date for sorting and `pubDate` (RFC3339 or `YYYY-MM-DD`).
/// - `author`: optional item author.
/// - `description`: optional summary/preview override.
#[derive(Debug, Deserialize, Clone)]
pub struct FrontMatter {
    pub title: String,
    #[serde(deserialize_with = "deserialize_date")]
    pub date: Option<DateTime<Utc>>,
    pub author: Option<String>,
    /// User-supplied summary, used as a fallback preview source.
    pub description: Option<String>,
    /// Per-chapter feed inclusion override. When absent, the chapter follows
    /// the book-level `default-behavior` (`include-all` by default).
    #[serde(default)]
    pub feed: Option<FeedVisibility>,
}
