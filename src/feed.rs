//! Building RSS 2.0 feed pages from collected articles.

use std::path::Path;

use rss::{Channel, ChannelBuilder, Guid, Item, ItemBuilder};

use crate::article::collect_articles;
use crate::error::Result;
use crate::preview::render_preview;

/// One generated RSS feed file.
///
/// `filename` is the relative file name written into `src/` (for example
/// `rss.xml` or `rss2.xml`). `channel` is the corresponding RSS 2.0 channel.
pub struct FeedPage {
    /// e.g. "rss.xml", "rss2.xml"
    pub filename: String,
    pub channel: Channel,
}

/// Result of building feeds for a book.
///
/// In simple setups this will contain a single `rss.xml` page. When
/// pagination is enabled it contains multiple [`FeedPage`]s (e.g. `rss.xml`,
/// `rss2.xml`, `rss3.xml`, …) each with a slice of the overall item list.
pub struct BuildResult {
    pub pages: Vec<FeedPage>,
}

/// Options controlling how a feed is built.
///
/// Grouping these avoids a long positional-argument list at the call site
/// (see `build_feed`) and makes it cheap to add new options later without
/// breaking every caller.
#[derive(Debug, Clone)]
pub struct FeedOptions<'a> {
    pub title: &'a str,
    pub site_url: &'a str,
    pub description: &'a str,
    pub full_preview: bool,
    pub max_items: usize,
    pub paginated: bool,
}

/// Build the absolute `.html` link for an article, given its `src`-relative
/// markdown path.
fn article_link(base_url: &str, article_path: &str) -> String {
    let html_path = article_path
        .replace('\\', "/")
        .replace(".md", ".html")
        .replace("/README.html", "/index.html");
    format!("{base_url}/{html_path}")
}

/// Build a single [`Channel`] from a slice of items.
fn build_channel(title: &str, base_url: &str, description: &str, items: &[Item]) -> Channel {
    ChannelBuilder::default()
        .title(title)
        .link(format!("{base_url}/"))
        .description(description)
        .items(items.to_vec())
        .generator(Some(format!(
            "mdbook-rss-feed {}",
            env!("CARGO_PKG_VERSION")
        )))
        .build()
}

/// Split `items` into one or more [`FeedPage`]s according to `opts`.
fn paginate(items: &[Item], opts: &FeedOptions<'_>, base_url: &str) -> Vec<FeedPage> {
    let mut pages = Vec::new();

    let should_paginate = opts.paginated && opts.max_items > 0 && items.len() > opts.max_items;
    if !should_paginate {
        let channel = build_channel(opts.title, base_url, opts.description, items);
        pages.push(FeedPage {
            filename: "rss.xml".to_string(),
            channel,
        });
        return pages;
    }

    let total_pages = items.len().div_ceil(opts.max_items);
    for page_idx in 0..total_pages {
        let start = page_idx * opts.max_items;
        let end = (start + opts.max_items).min(items.len());
        let slice = &items[start..end];

        let filename = if page_idx == 0 {
            "rss.xml".to_string()
        } else {
            format!("rss{}.xml", page_idx + 1)
        };

        let channel = build_channel(opts.title, base_url, opts.description, slice);
        pages.push(FeedPage { filename, channel });
    }

    pages
}

/// Build one or more RSS 2.0 feeds for an mdBook.
///
/// This scans `src_dir` for chapters, extracts frontmatter, generates HTML
/// previews, and returns a [`BuildResult`] containing one or more
/// [`FeedPage`]s. The first page is always `rss.xml`; when `opts.paginated`
/// is `true` and `opts.max_items > 0`, additional pages `rss2.xml`,
/// `rss3.xml`, … are created.
///
/// # Errors
/// Returns `Err` if:
/// - `src_dir` can't be accessed or doesn't exist
/// - reading or walking the directory tree fails
pub fn build_feed(src_dir: &Path, opts: &FeedOptions<'_>) -> Result<BuildResult> {
    let articles = collect_articles(src_dir)?;
    let base_url = opts.site_url.trim_end_matches('/');

    let items: Vec<Item> = articles
        .into_iter()
        .map(|article| {
            let link = article_link(base_url, &article.path);
            let preview = render_preview(
                &article.content,
                article.fm.description.as_deref(),
                opts.full_preview,
            );

            let mut item = ItemBuilder::default();
            item.title(Some(article.fm.title.clone()));
            item.link(Some(link.clone()));
            item.description(Some(preview));
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

    Ok(BuildResult {
        pages: paginate(&items, opts, base_url),
    })
}
