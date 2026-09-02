//! JSON Feed 1.1 support.
//!
//! Enabled by the `json-feed` cargo feature. Converts an RSS [`Channel`]
//! into a minimal JSON Feed 1.1 document.

use chrono::DateTime;
use rss::Channel;
use serde::Serialize;
use serde_json::Value as JsonValue;

use crate::feed::ItemMeta;

/// Minimal JSON Feed 1.1 document.
#[derive(Serialize)]
pub struct JsonFeed {
    pub version: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home_page_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// URL of a large square image (512×512+) for use in timelines.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// URL of a small square image (64×64+) for use in source lists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favicon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_url: Option<String>,
    pub items: Vec<JsonFeedItem>,
}

#[derive(Serialize)]
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
    /// Allows a simple string or a richer author object later.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<JsonValue>,
    /// Tags/categories for this item, per JSON Feed 1.1 spec.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// BCP-47 language tag, per JSON Feed 1.1 spec.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// Stable per-item id: prefer guid, then link, then title.
fn item_id(item: &rss::Item) -> String {
    item.guid()
        .map(|g| g.value().to_string())
        .or_else(|| item.link().map(str::to_string))
        .unwrap_or_else(|| item.title().unwrap_or_default().to_string())
}

/// Convert an RSS 2.0 channel into a JSON Feed 1.1 structure.
#[must_use]
pub fn rss_to_json_feed(
    channel: &Channel,
    feed_url: Option<&str>,
    next_url: Option<&str>,
    item_meta: &[ItemMeta],
    icon: Option<&str>,
    favicon: Option<&str>,
) -> JsonFeed {
    let items: Vec<JsonFeedItem> = channel
        .items()
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let meta = item_meta.get(i).cloned().unwrap_or_default();

            let author = meta.author.as_deref().map(|name| {
                if let Some(email) = meta.author_email.as_deref() {
                    serde_json::json!({ "name": name, "email": email })
                } else {
                    serde_json::json!({ "name": name })
                }
            });
            JsonFeedItem {
                id: item_id(item),
                url: item.link().map(str::to_string),
                title: item.title().map(str::to_string),
                content_html: item.description().map(str::to_string),
                date_published: item
                    .pub_date()
                    .and_then(|d| DateTime::parse_from_rfc2822(d).ok())
                    .map(|dt| dt.to_rfc3339()),
                author,
                tags: meta.tags,
                language: meta.lang,
            }
        })
        .collect();

    JsonFeed {
        version: "https://jsonfeed.org/version/1.1".to_string(),
        title: channel.title().to_string(),
        home_page_url: Some(channel.link().to_string()),
        feed_url: feed_url.map(str::to_string),
        description: Some(channel.description().to_string()),
        icon: icon.map(str::to_string),
        favicon: favicon.map(str::to_string),
        next_url: next_url.map(str::to_string),
        items,
    }
}
