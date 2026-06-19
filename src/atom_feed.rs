//! Atom 1.0 support.
//!
//! Enabled by the `atom` cargo feature. Converts an RSS [`Channel`] into a
//! best-effort Atom 1.0 feed: titles, links, descriptions (as HTML content),
//! and dates are copied across where available.

use atom_syndication::{
    Content as AtomContent, Entry as AtomEntry, Feed as AtomFeed, Link as AtomLink,
    Text as AtomText,
};
use chrono::DateTime;
use rss::Channel;

/// Stable per-entry id: prefer guid, then link, then title.
fn entry_id(item: &rss::Item) -> String {
    item.guid()
        .map(|g| g.value().to_string())
        .or_else(|| item.link().map(str::to_string))
        .unwrap_or_else(|| item.title().unwrap_or_default().to_string())
}

fn build_entry(item: &rss::Item) -> AtomEntry {
    let mut entry = AtomEntry::default();
    entry.set_id(entry_id(item));

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
    if let Some(dt) = item
        .pub_date()
        .and_then(|d| DateTime::parse_from_rfc2822(d).ok())
    {
        entry.set_updated(dt);
    }

    entry
}

/// Convert an RSS 2.0 channel into a minimal Atom 1.0 feed.
///
/// This is a best-effort mapping. It copies titles, links, descriptions (as
/// HTML content), and dates where available.
#[must_use]
pub fn rss_to_atom(channel: &Channel) -> AtomFeed {
    let entries: Vec<AtomEntry> = channel.items().iter().map(build_entry).collect();

    let mut feed = AtomFeed::default();
    feed.set_title(channel.title().to_string());
    feed.set_entries(entries);

    let link = channel.link();
    if link.is_empty() {
        // Fallback id if link is somehow empty.
        feed.set_id(channel.title().to_string());
    } else {
        feed.set_links(vec![AtomLink {
            href: link.to_string(),
            ..Default::default()
        }]);
        // Use the public feed URL as a stable Atom feed id.
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
