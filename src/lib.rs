//! mdbook-rss-feed core library.
//!
//! This module scans an mdBook src directory for chapters, extracts frontmatter
//! and content, and turns them into one or more RSS 2.0 channels suitable for
//! static hosting.

mod article;
mod error;
mod feed;
mod frontmatter;
mod preview;

use chrono::{DateTime, Utc};
use rss::Channel;
use serde_json::Value as JsonValue;
use std::time::SystemTime;

// Re-exports
pub use article::{collect_articles, parse_markdown_file, Article};
pub use feed::{build_feed, BuildResult, FeedOptions, FeedPage};

// Minimal JSON Feed 1.1 model for this crate
#[derive(serde::Serialize)]
pub struct JsonFeed {
    pub version: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home_page_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_url: Option<String>,
    pub items: Vec<JsonFeedItem>,
}

#[derive(serde::Serialize)]
pub struct JsonFeedItem {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_published: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<JsonValue>,
}

// Optional Atom support
use atom_syndication::{
    Content as AtomContent, Entry as AtomEntry, Feed as AtomFeed, Link as AtomLink,
    Text as AtomText,
};

// Convert file modification time → UTC
fn systemtime_to_utc(st: SystemTime) -> DateTime<Utc> {
    DateTime::<Utc>::from(st)
}

/// Convert an RSS 2.0 channel into a JSON Feed 1.1 structure.
///
/// Used when `json-feed = true` in the configuration.
#[must_use]
pub fn rss_to_json_feed(
    channel: &Channel,
    feed_url: Option<&str>,
    next_url: Option<&str>,
) -> JsonFeed {
    let items: Vec<JsonFeedItem> = channel
        .items()
        .iter()
        .map(|item| {
            let id = item
                .guid()
                .map(|g| g.value().to_string())
                .or_else(|| item.link().map(std::string::ToString::to_string))
                .unwrap_or_else(|| item.title().unwrap_or("").to_string());

            let url = item.link().map(std::string::ToString::to_string);
            let title = item.title().map(std::string::ToString::to_string);
            let content_html = item.description().map(std::string::ToString::to_string);
            let date_published = item.pub_date().and_then(|d| {
                DateTime::parse_from_rfc2822(d)
                    .ok()
                    .map(|dt| dt.to_rfc3339())
            });

            let author = item.author().map(|a| serde_json::json!({ "name": a }));

            JsonFeedItem {
                id,
                url,
                title,
                content_html,
                date_published,
                author,
            }
        })
        .collect();

    JsonFeed {
        version: "https://jsonfeed.org/version/1.1".to_string(),
        title: channel.title().to_string(),
        home_page_url: Some(channel.link().to_string()),
        feed_url: feed_url.map(std::string::ToString::to_string),
        description: Some(channel.description().to_string()),
        next_url: next_url.map(std::string::ToString::to_string),
        items,
    }
}
/// Convert an RSS 2.0 channel into a minimal Atom 1.0 feed.
///
/// This is a best-effort mapping used when `atom = true` in the configuration.
/// It copies titles, links, descriptions (as HTML content), and dates where
/// available.
#[must_use]
pub fn rss_to_atom(channel: &Channel) -> AtomFeed {
    let entries: Vec<AtomEntry> = channel
        .items()
        .iter()
        .map(|item| {
            let mut entry = AtomEntry::default();

            // Stable per-entry id: prefer guid, then link, then title
            let entry_id = item
                .guid()
                .map(|g| g.value().to_string())
                .or_else(|| item.link().map(std::string::ToString::to_string))
                .unwrap_or_else(|| item.title().unwrap_or("").to_string());
            entry.set_id(entry_id);

            if let Some(title) = item.title() {
                entry.set_title(title.to_string());
            }

            if let Some(link) = item.link() {
                entry.set_links(vec![AtomLink {
                    href: link.to_string(),
                    ..Default::default()
                }]);
            }

            if let Some(desc) = item.description() {
                let mut content = AtomContent::default();
                content.set_content_type("html".to_string());
                content.set_value(Some(desc.to_string()));
                entry.set_content(Some(content));
            }

            if let Some(Ok(dt)) = item.pub_date().map(DateTime::parse_from_rfc2822) {
                entry.set_updated(dt);
            }

            entry
        })
        .collect();

    let mut feed = AtomFeed::default();
    feed.set_title(channel.title().to_string());
    feed.set_entries(entries);

    let link = channel.link();
    if link.is_empty() {
        // Fallback id if link is somehow empty
        feed.set_id(channel.title().to_string());
    } else {
        feed.set_links(vec![AtomLink {
            href: link.to_string(),
            ..Default::default()
        }]);
        // Use the public feed URL as a stable Atom feed id
        feed.set_id(link.to_string());
    }

    let desc = channel.description();
    if !desc.is_empty() {
        feed.set_subtitle(Some(AtomText {
            value: desc.to_string(),
            ..Default::default()
        }));
    }

    feed
}
