//! Atom 1.0 support.
//!
//! Enabled by the `atom` cargo feature. Converts an RSS [`Channel`] into a
//! best-effort Atom 1.0 feed: titles, links, descriptions (as HTML content),
//! and dates are copied across where available.

use atom_syndication::{
    Content as AtomContent, Entry as AtomEntry, Feed as AtomFeed, Link as AtomLink,
    Person as AtomPerson, Text as AtomText,
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

    // Set per-entry author from RSS `<author>` field if present
    if let Some(author) = item.author() {
        entry.set_authors(vec![AtomPerson {
            name: author.to_string(),
            ..Default::default()
        }]);
    }

    entry
}

/// Convert an RSS 2.0 channel into a minimal Atom 1.0 feed.
///
/// This is a best-effort mapping. It copies titles, links, descriptions (as
/// HTML content), and dates where available.
#[must_use]
pub fn rss_to_atom(
    channel: &Channel,
    self_url: Option<&str>,
    next_url: Option<&str>,
    prev_url: Option<&str>,
    authors: &[String],
) -> AtomFeed {
    let entries: Vec<AtomEntry> = channel.items().iter().map(build_entry).collect();

    // Set feed-level updated to the most recent entry date.
    let latest = entries
        .iter()
        .map(|e| *e.updated())
        .max()
        .unwrap_or_else(|| DateTime::UNIX_EPOCH.into());

    let mut feed = AtomFeed::default();
    feed.set_title(channel.title().to_string());
    feed.set_updated(latest);
    feed.set_entries(entries);

    // Feed-level authors: required by Atom spec when entries lack individual authors
    if !authors.is_empty() {
        feed.set_authors(
            authors
                .iter()
                .map(|name| AtomPerson {
                    name: name.clone(),
                    ..Default::default()
                })
                .collect::<Vec<_>>(),
        );
    }

    let home = channel.link();

    // Build the links vec: self, then optional next/prev, then the home link.
    let mut links: Vec<AtomLink> = Vec::new();

    if let Some(slf) = self_url {
        links.push(AtomLink {
            href: slf.to_string(),
            rel: "self".to_string(),
            ..Default::default()
        });
        feed.set_id(slf.to_string());
    } else if !home.is_empty() {
        feed.set_id(home.to_string());
    } else {
        feed.set_id(channel.title().to_string());
    }

    if let Some(next) = next_url {
        links.push(AtomLink {
            href: next.to_string(),
            rel: "next".to_string(),
            ..Default::default()
        });
    }

    if let Some(prev) = prev_url {
        links.push(AtomLink {
            href: prev.to_string(),
            rel: "prev".to_string(),
            ..Default::default()
        });
    }

    if !home.is_empty() {
        links.push(AtomLink {
            href: home.to_string(),
            rel: "alternate".to_string(),
            ..Default::default()
        });
    }

    feed.set_links(links);

    let desc = channel.description();
    if !desc.is_empty() {
        feed.set_subtitle(Some(AtomText {
            value: desc.to_string(),
            ..Default::default()
        }));
    }

    feed
}
