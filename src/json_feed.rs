//! JSON Feed 1.1 support.
//!
//! Enabled by the `json-feed` cargo feature. Converts an RSS [`Channel`]
//! into a minimal JSON Feed 1.1 document.

use chrono::DateTime;
use rss::Channel;
use serde::Serialize;
use serde_json::Value as JsonValue;

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
) -> JsonFeed {
    let items: Vec<JsonFeedItem> = channel
        .items()
        .iter()
        .map(|item| JsonFeedItem {
            id: item_id(item),
            url: item.link().map(str::to_string),
            title: item.title().map(str::to_string),
            content_html: item.description().map(str::to_string),
            date_published: item
                .pub_date()
                .and_then(|d| DateTime::parse_from_rfc2822(d).ok())
                .map(|dt| dt.to_rfc3339()),
            author: item.author().map(|a| serde_json::json!({ "name": a })),
        })
        .collect();

    JsonFeed {
        version: "https://jsonfeed.org/version/1.1".to_string(),
        title: channel.title().to_string(),
        home_page_url: Some(channel.link().to_string()),
        feed_url: feed_url.map(str::to_string),
        description: Some(channel.description().to_string()),
        next_url: next_url.map(str::to_string),
        items,
    }
}
