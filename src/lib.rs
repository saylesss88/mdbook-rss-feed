//! mdbook-rss-feed core library.
//!
//! This module scans an mdBook src directory for chapters, extracts frontmatter
//! and content, and turns them into one or more RSS 2.0 channels suitable for
//! static hosting.

mod article;
mod error;
mod frontmatter;
mod preview;

use crate::error::Result;
use crate::preview::{
    html_first_paragraphs, markdown_to_html, strip_leading_boilerplate, utf8_prefix,
};
pub use article::{collect_articles, parse_markdown_file, Article};
use chrono::{DateTime, Utc};
use rss::{Channel, ChannelBuilder, Guid, Item, ItemBuilder};
use serde_json::Value as JsonValue;
use std::{path::Path, time::SystemTime};

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

/// One generated RSS feed file.
///
/// `filename` is the relative file name written into `src/` (for example
/// `rss.xml` or `rss2.xml`). `channel` is the corresponding RSS 2.0 channel.
pub struct FeedPage {
    pub filename: String, // e.g. "rss.xml", "rss2.xml"
    pub channel: Channel,
}

/// Result of building feeds for a book.
///
/// In simple setups this will contain a single `rss.xml` page. When pagination
/// is enabled it contains multiple `FeedPage`s (e.g. `rss.xml`, `rss2.xml`,
/// `rss3.xml`, …) each with a slice of the overall item list.
pub struct BuildResult {
    pub pages: Vec<FeedPage>,
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

/// Build one or more RSS 2.0 feeds for an mdBook.
///
/// This scans `src_dir` for chapters, extracts frontmatter, generates HTML
/// previews, and returns a `BuildResult` containing one or more `FeedPage`s.
/// The first page is always `rss.xml`; when `paginated` is `true` and
/// `max_items > 0`, additional pages `rss2.xml`, `rss3.xml`, … are created.
///
/// Arguments:
/// - `src_dir`: mdBook `src` directory to scan for `.md` files.
/// - `title`: feed title, usually `config.book.title`.
/// - `site_url`: public base URL of the rendered site (no trailing slash).
/// - `description`: top-level feed description.
/// - `full_preview`: when `true`, include full chapter content instead of a
///   shortened preview in `<description>`.
/// - `max_items`: maximum items per feed page when pagination is enabled.
/// - `paginated`: enable or disable multi-page feeds.
/// # Errors
/// On success, the caller is responsible for writing each `FeedPage`'s channel
/// to disk at `pages[i].filename`.
/// Will return `Err` if:
/// - The `src_dir` can't be accessed or doesn't exist
/// - `collect_articles` fails to read or parse the md files
/// - There are underlying I/O issues when walking the directory tree
pub fn build_feed(
    src_dir: &Path,
    title: &str,
    site_url: &str,
    description: &str,
    full_preview: bool,
    max_items: usize,
    paginated: bool,
) -> Result<BuildResult> {
    let articles = collect_articles(src_dir)?;

    let base_url = site_url.trim_end_matches('/');

    let items: Vec<Item> = articles
        .into_iter()
        .map(|article| {
            // Build correct .html path
            let html_path = article
                .path
                .replace('\\', "/")
                .replace(".md", ".html")
                .replace("/README.html", "/index.html");

            let link = format!("{base_url}/{html_path}");

            // Hybrid preview source selection
            let content_trimmed = article.content.trim();

            // Count chars to decide if body is "very short"
            let _body_len = content_trimmed.chars().count();

            // 1) Choose base markdown (body vs description)
            let mut source_md: &str;

            if full_preview {
                // Full-content mode: always use the full body markdown
                source_md = article.content.as_str();
            } else {
                // Only consider the first slice of markdown for preview
                const PREVIEW_MD_SLICE_CHARS: usize = 4000;
                // Preview mode: existing hybrid logic (body vs description, boilerplate strip, slice)
                let content_trimmed = article.content.trim();
                let body_len = content_trimmed.chars().count();

                source_md =
                    if body_len >= MIN_BODY_PREVIEW_CHARS || article.fm.description.is_none() {
                        content_trimmed
                    } else {
                        article.fm.description.as_deref().unwrap_or(content_trimmed)
                    };

                // Strip obvious leading boilerplate so we start near the intro text
                source_md = strip_leading_boilerplate(source_md);

                source_md = utf8_prefix(source_md, PREVIEW_MD_SLICE_CHARS);
            }

            // Convert chosen markdown source → HTML
            let raw_html = markdown_to_html(source_md);

            // Use either full HTML or first few paragraphs as preview
            let preview = if full_preview {
                raw_html
            } else {
                html_first_paragraphs(&raw_html, 3, 800)
            };

            let mut item = ItemBuilder::default();

            item.title(Some(article.fm.title.clone()));
            item.link(Some(link.clone()));
            item.description(Some(preview)); // Stored directly inside CDATA
            item.guid(Some(Guid {
                value: link,
                permalink: true,
            }));

            if let Some(date) = article.fm.date {
                item.pub_date(Some(date.to_rfc2822()));
            }

            if let Some(author) = article.fm.author {
                item.author(Some(author));
            }

            item.build()
        })
        .collect();

    // Helper to construct a single Channel with a slice of items
    let build_channel_for_slice =
        |slice: &[Item], _page_idx: usize, _total_pages: usize| -> Channel {
            ChannelBuilder::default()
                .title(title)
                .link(format!("{base_url}/"))
                .description(description)
                .items(slice.to_vec())
                .generator(Some("mdbook-rss-feed 1.0.0".to_string()))
                .build()
        };

    let mut pages = Vec::new();

    if !paginated || max_items == 0 || items.len() <= max_items {
        // Single feed (no pagination)
        let channel = build_channel_for_slice(&items, 1, 1);
        pages.push(FeedPage {
            filename: "rss.xml".to_string(),
            channel,
        });
    } else {
        // Split into pages of size max_items
        let total_pages = items.len().div_ceil(max_items);

        for page_idx in 0..total_pages {
            let start = page_idx * max_items;
            let end = (start + max_items).min(items.len());
            let slice = &items[start..end];

            let filename = if page_idx == 0 {
                "rss.xml".to_string()
            } else {
                format!("rss{}.xml", page_idx + 1)
            };

            let channel = build_channel_for_slice(slice, page_idx + 1, total_pages);

            pages.push(FeedPage { filename, channel });
        }
    }

    Ok(BuildResult { pages })
}
