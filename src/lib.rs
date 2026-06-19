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

#[cfg(feature = "json-feed")]
mod json_feed;

use chrono::{DateTime, Utc};
use rss::Channel;
use serde_json::Value as JsonValue;
use std::time::SystemTime;

// Re-exports
pub use article::{collect_articles, parse_markdown_file, Article};
pub use error::{FeedError, Result};
pub use feed::{build_feed, BuildResult, FeedOptions, FeedPage};
pub use frontmatter::FrontMatter;
#[cfg(feature = "json-feed")]
pub use json_feed::{rss_to_json_feed, JsonFeed, JsonFeedItem};

// Optional Atom support
use atom_syndication::{
    Content as AtomContent, Entry as AtomEntry, Feed as AtomFeed, Link as AtomLink,
    Text as AtomText,
};

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
