//! Building RSS 2.0 feed pages from collected articles.

use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use rss::extension::{Extension, ExtensionBuilder};
use rss::{Channel, ChannelBuilder, Guid, Item, ItemBuilder};

use crate::article::{collect_articles, Article};
use crate::error::Result;
use crate::frontmatter::FeedVisibility;
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DefaultBehavior {
    /// Include every chapter unless explicitly marked `feed: exclude`.
    /// This is the default when `default-behavior` is not set.
    #[default]
    IncludeAll,
    /// Exclude every chapter unless explicitly marked `feed: include`.
    ExcludeAll,
}

impl FromStr for DefaultBehavior {
    type Err = std::convert::Infallible;
    /// Parse from the string value in `book.toml`.
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.trim() {
            "exclude-all" => Ok(Self::ExcludeAll),
            _ => Ok(Self::IncludeAll),
        }
    }
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
    pub default_behavior: DefaultBehavior,
    pub strict: bool,
}

/// Return `true` if this article should appear in the feed given `default_behavior`.
fn article_is_included(article: &Article, default_behavior: &DefaultBehavior) -> bool {
    match &article.fm.feed {
        // Explicit per-chapter override always wins.
        Some(FeedVisibility::Include) => true,
        Some(FeedVisibility::Exclude) => false,
        // No override: fall back to the book-level default.
        None => *default_behavior == DefaultBehavior::IncludeAll,
    }
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

/// Build an `atom:link` extension element.
///
/// Used to add `rel="self"`, `rel="next"`, and `rel="prev"` links to RSS
/// channels, following the Atom namespace convention for RSS pagination.
fn atom_link(href: &str, rel: &str) -> Extension {
    let mut attrs = BTreeMap::new();
    attrs.insert("href".to_string(), href.to_string());
    attrs.insert("rel".to_string(), rel.to_string());
    ExtensionBuilder::default()
        .name("atom:link".to_string())
        .attrs(attrs)
        .build()
}

/// Compute the RSS filename for a given zero-based page index.
fn rss_filename(page_idx: usize) -> String {
    if page_idx == 0 {
        "rss.xml".to_string()
    } else {
        format!("rss{}.xml", page_idx + 1)
    }
}

/// Build a single [`Channel`] from a slice of items.
///
/// - `rel="self"` — the canonical URL of this page
/// - `rel="prev"` — the newer page, when this is not the first page
/// - `rel="next"` — the older page, when this is not the last page
fn build_channel(
    title: &str,
    base_url: &str,
    description: &str,
    items: &[Item],
    page_idx: usize,
    total_pages: usize,
) -> Channel {
    // Atom namespace links for pagination discovery.
    let self_url = format!("{base_url}/{}", rss_filename(page_idx));
    let mut atom_links = vec![atom_link(&self_url, "self")];

    if page_idx > 0 {
        let prev_url = format!("{base_url}/{}", rss_filename(page_idx - 1));
        atom_links.push(atom_link(&prev_url, "prev"));
    }
    if page_idx + 1 < total_pages {
        let next_url = format!("{base_url}/{}", rss_filename(page_idx + 1));
        atom_links.push(atom_link(&next_url, "next"));
    }

    let mut namespaces = BTreeMap::new();
    namespaces.insert(
        "atom".to_string(),
        "http://www.w3.org/2005/Atom".to_string(),
    );

    let mut inner: BTreeMap<String, Vec<Extension>> = BTreeMap::new();
    inner.insert("link".to_string(), atom_links);
    let mut extensions = BTreeMap::new();
    extensions.insert("atom".to_string(), inner);

    ChannelBuilder::default()
        .title(title)
        .link(format!("{base_url}/"))
        .description(description)
        .items(items.to_vec())
        .generator(Some(format!(
            "mdbook-rss-feed {}",
            env!("CARGO_PKG_VERSION")
        )))
        .namespaces(namespaces)
        .extensions(extensions)
        .build()
}

/// Split `items` into one or more [`FeedPage`]s according to `opts`.
fn paginate(items: &[Item], opts: &FeedOptions<'_>, base_url: &str) -> Vec<FeedPage> {
    let mut pages: Vec<FeedPage> = Vec::new();

    let should_paginate = opts.paginated && opts.max_items > 0 && items.len() > opts.max_items;
    if !should_paginate {
        let channel = build_channel(opts.title, base_url, opts.description, items, 0, 1);
        return vec![FeedPage {
            filename: "rss.xml".to_string(),
            channel,
        }];
    }

    let total_pages = items.len().div_ceil(opts.max_items);
    for page_idx in 0..total_pages {
        let start = page_idx * opts.max_items;
        let end = (start + opts.max_items).min(items.len());
        let channel = build_channel(
            opts.title,
            base_url,
            opts.description,
            &items[start..end],
            page_idx,
            total_pages,
        );
        pages.push(FeedPage {
            filename: rss_filename(page_idx),
            channel,
        });
    }
    pages
}

/// Convert a list of [`Article`]s into RSS [`Item`]s, applying filtering.
fn articles_to_items(articles: Vec<Article>, opts: &FeedOptions<'_>, base_url: &str) -> Vec<Item> {
    articles
        .into_iter()
        .filter(|a| article_is_included(a, &opts.default_behavior))
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
        .collect()
}

#[must_use]
pub fn build_feed_from_articles(articles: Vec<Article>, opts: &FeedOptions<'_>) -> BuildResult {
    let base_url = opts.site_url.trim_end_matches('/');
    let items = articles_to_items(articles, opts, base_url);
    BuildResult {
        pages: paginate(&items, opts, base_url),
    }
}

/// Build one or more RSS 2.0 feeds by scanning `src_dir` on disk.
///
/// **Legacy path.** Does not expand `{{#include}}` directives and includes
/// all `.md` files, not just those listed in `SUMMARY.md`. Prefer
/// [`build_feed_from_articles`] with [`articles_from_book_json`] when
/// running as an mdBook preprocessor.
///
/// # Errors
/// Returns `Err` if `src_dir` can't be accessed or walked.
pub fn build_feed(src_dir: &Path, opts: &FeedOptions<'_>) -> Result<BuildResult> {
    let articles = collect_articles(src_dir, opts.strict)?;
    Ok(build_feed_from_articles(articles, opts))
}
