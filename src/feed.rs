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
    pub author_email: Option<String>,
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
                // chrono's to_rfc2822() doesn't zero-pad single-digit days,
                // violating RFC 2822. Format manually to ensure compliance.
                item.pub_date(Some(date.format("%a, %d %b %Y %T %z").to_string()));
            }
            if let Some(author) = article.fm.author 
                && let Some(email) = &opts.author_email {
                    item.author(Some(format!("{email} ({author})")));
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
/// [`build_feed_from_articles`] with [`crate::articles_from_book_json`] when
/// running as an mdBook preprocessor.
///
/// # Errors
/// Returns `Err` if `src_dir` can't be accessed or walked.
pub fn build_feed(src_dir: &Path, opts: &FeedOptions<'_>) -> Result<BuildResult> {
    let articles = collect_articles(src_dir, opts.strict)?;
    Ok(build_feed_from_articles(articles, opts))
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::article::Article;
    use crate::frontmatter::{FeedVisibility, FrontMatter};

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn make_article(
        title: &str,
        path: &str,
        date: Option<&str>,
        feed: Option<FeedVisibility>,
    ) -> Article {
        let date = date.and_then(|d| {
            chrono::DateTime::parse_from_rfc3339(d)
                .ok()
                .map(|dt| dt.with_timezone(&chrono::Utc))
        });
        Article {
            fm: FrontMatter {
                title: title.to_string(),
                date,
                author: None,
                description: None,
                feed,
            },
            content: format!("# {title}\n\nSome content for {title}."),
            path: path.to_string(),
        }
    }

    fn default_opts(site_url: &str) -> FeedOptions<'_> {
        FeedOptions {
            title: "Test Blog",
            site_url,
            description: "A test blog.",
            full_preview: false,
            max_items: 10,
            paginated: false,
            default_behavior: DefaultBehavior::IncludeAll,
            strict: false,
            author_email: None,
        }
    }

    // ── DefaultBehavior ───────────────────────────────────────────────────────

    #[test]
    fn default_behavior_parses_exclude_all() {
        let b: DefaultBehavior = "exclude-all".parse().unwrap();
        assert_eq!(b, DefaultBehavior::ExcludeAll);
    }

    #[test]
    fn default_behavior_unknown_string_is_include_all() {
        let b: DefaultBehavior = "whatever".parse().unwrap();
        assert_eq!(b, DefaultBehavior::IncludeAll);
    }

    #[test]
    fn default_behavior_empty_string_is_include_all() {
        let b: DefaultBehavior = "".parse().unwrap();
        assert_eq!(b, DefaultBehavior::IncludeAll);
    }

    #[test]
    fn default_behavior_default_is_include_all() {
        assert_eq!(DefaultBehavior::default(), DefaultBehavior::IncludeAll);
    }

    // ── article_link ─────────────────────────────────────────────────────────

    #[test]
    fn article_link_converts_md_to_html() {
        let link = article_link("https://example.com", "posts/hello.md");
        assert_eq!(link, "https://example.com/posts/hello.html");
    }

    #[test]
    fn article_link_readme_becomes_index() {
        let link = article_link("https://example.com", "intro/README.md");
        assert_eq!(link, "https://example.com/intro/index.html");
    }

    #[test]
    fn article_link_normalizes_backslashes() {
        let link = article_link("https://example.com", r"posts\windows.md");
        assert_eq!(link, "https://example.com/posts/windows.html");
    }

    // ── rss_filename ─────────────────────────────────────────────────────────

    #[test]
    fn rss_filename_page_zero_is_rss_xml() {
        assert_eq!(rss_filename(0), "rss.xml");
    }

    #[test]
    fn rss_filename_subsequent_pages_are_numbered() {
        assert_eq!(rss_filename(1), "rss2.xml");
        assert_eq!(rss_filename(2), "rss3.xml");
        assert_eq!(rss_filename(9), "rss10.xml");
    }

    // ── article_is_included ───────────────────────────────────────────────────

    #[test]
    fn include_all_includes_article_with_no_override() {
        let a = make_article("Test", "test.md", None, None);
        assert!(article_is_included(&a, &DefaultBehavior::IncludeAll));
    }

    #[test]
    fn include_all_excludes_article_with_feed_exclude() {
        let a = make_article("Test", "test.md", None, Some(FeedVisibility::Exclude));
        assert!(!article_is_included(&a, &DefaultBehavior::IncludeAll));
    }

    #[test]
    fn exclude_all_excludes_article_with_no_override() {
        let a = make_article("Test", "test.md", None, None);
        assert!(!article_is_included(&a, &DefaultBehavior::ExcludeAll));
    }

    #[test]
    fn exclude_all_includes_article_with_feed_include() {
        let a = make_article("Test", "test.md", None, Some(FeedVisibility::Include));
        assert!(article_is_included(&a, &DefaultBehavior::ExcludeAll));
    }

    #[test]
    fn explicit_include_overrides_include_all() {
        let a = make_article("Test", "test.md", None, Some(FeedVisibility::Include));
        assert!(article_is_included(&a, &DefaultBehavior::IncludeAll));
    }

    // ── build_feed_from_articles ──────────────────────────────────────────────

    #[test]
    fn build_feed_from_articles_basic() {
        let articles = vec![
            make_article("Post A", "a.md", Some("2024-06-01T00:00:00Z"), None),
            make_article("Post B", "b.md", Some("2024-05-01T00:00:00Z"), None),
        ];
        let opts = default_opts("https://example.com");
        let result = build_feed_from_articles(articles, &opts);
        assert_eq!(result.pages.len(), 1);
        let channel = &result.pages[0].channel;
        assert_eq!(channel.items().len(), 2);
        assert_eq!(result.pages[0].filename, "rss.xml");
    }

    #[test]
    fn build_feed_from_articles_channel_metadata() {
        let articles = vec![make_article("Post", "post.md", None, None)];
        let opts = default_opts("https://myblog.com");
        let result = build_feed_from_articles(articles, &opts);
        let channel = &result.pages[0].channel;
        assert_eq!(channel.title(), "Test Blog");
        assert_eq!(channel.description(), "A test blog.");
        assert!(channel.link().contains("myblog.com"));
    }

    #[test]
    fn build_feed_from_articles_item_link_is_html() {
        let articles = vec![make_article("Post", "posts/my-post.md", None, None)];
        let opts = default_opts("https://example.com");
        let result = build_feed_from_articles(articles, &opts);
        let item = &result.pages[0].channel.items()[0];
        let link = item.link().unwrap();

        assert!(
            std::path::Path::new(link)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("html")),
            "link should end in .html: {link}"
        );

        assert!(link.starts_with("https://example.com"));
    }

    #[test]
    fn build_feed_from_articles_pub_date_zero_padded() {
        // Day 5 must be "05", not "5" — RFC 2822 requires zero-padding.
        let articles = vec![make_article(
            "Post",
            "post.md",
            Some("2024-01-05T00:00:00Z"),
            None,
        )];
        let opts = default_opts("https://example.com");
        let result = build_feed_from_articles(articles, &opts);
        let item = &result.pages[0].channel.items()[0];
        let pub_date = item.pub_date().unwrap();
        // The day portion should be "05", never "5".
        assert!(
            pub_date.contains(" 05 "),
            "expected zero-padded day in: {pub_date}"
        );
    }

    #[test]
    fn build_feed_from_articles_filters_excluded_items() {
        let articles = vec![
            make_article("Included", "inc.md", None, None),
            make_article("Excluded", "exc.md", None, Some(FeedVisibility::Exclude)),
        ];
        let opts = default_opts("https://example.com");
        let result = build_feed_from_articles(articles, &opts);
        let items = result.pages[0].channel.items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title().unwrap(), "Included");
    }

    #[test]
    fn build_feed_from_articles_exclude_all_only_shows_explicit_includes() {
        let articles = vec![
            make_article("A", "a.md", None, None),
            make_article("B", "b.md", None, Some(FeedVisibility::Include)),
            make_article("C", "c.md", None, None),
        ];
        let mut opts = default_opts("https://example.com");
        opts.default_behavior = DefaultBehavior::ExcludeAll;
        let result = build_feed_from_articles(articles, &opts);
        let items = result.pages[0].channel.items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title().unwrap(), "B");
    }

    #[test]
    fn build_feed_from_articles_trailing_slash_on_site_url_is_stripped() {
        let articles = vec![make_article("Post", "post.md", None, None)];
        let mut opts = default_opts("https://example.com/");
        opts.max_items = 0; // no pagination
        let result = build_feed_from_articles(articles, &opts);
        let item = &result.pages[0].channel.items()[0];
        let link = item.link().unwrap();
        // Should not have a double slash.
        assert!(
            !link.contains("//post.html"),
            "double slash in link: {link}"
        );
    }

    // ── pagination ────────────────────────────────────────────────────────────

    #[test]
    fn pagination_disabled_all_items_in_one_page() {
        let articles: Vec<Article> = (0..15)
            .map(|i| make_article(&format!("Post {i}"), &format!("{i}.md"), None, None))
            .collect();
        let mut opts = default_opts("https://example.com");
        opts.max_items = 5;
        opts.paginated = false; // disabled
        let result = build_feed_from_articles(articles, &opts);
        assert_eq!(result.pages.len(), 1);
        assert_eq!(result.pages[0].channel.items().len(), 15);
    }

    #[test]
    fn pagination_splits_into_multiple_pages() {
        let articles: Vec<Article> = (0..12)
            .map(|i| make_article(&format!("Post {i}"), &format!("{i}.md"), None, None))
            .collect();
        let mut opts = default_opts("https://example.com");
        opts.max_items = 5;
        opts.paginated = true;
        let result = build_feed_from_articles(articles, &opts);
        // 12 items at 5 per page = 3 pages (5, 5, 2).
        assert_eq!(result.pages.len(), 3);
        assert_eq!(result.pages[0].channel.items().len(), 5);
        assert_eq!(result.pages[1].channel.items().len(), 5);
        assert_eq!(result.pages[2].channel.items().len(), 2);
    }

    #[test]
    fn pagination_page_filenames_are_correct() {
        let articles: Vec<Article> = (0..11)
            .map(|i| make_article(&format!("Post {i}"), &format!("{i}.md"), None, None))
            .collect();
        let mut opts = default_opts("https://example.com");
        opts.max_items = 5;
        opts.paginated = true;
        let result = build_feed_from_articles(articles, &opts);
        assert_eq!(result.pages[0].filename, "rss.xml");
        assert_eq!(result.pages[1].filename, "rss2.xml");
        assert_eq!(result.pages[2].filename, "rss3.xml");
    }

    #[test]
    fn pagination_atom_self_link_matches_filename() {
        let articles: Vec<Article> = (0..11)
            .map(|i| make_article(&format!("Post {i}"), &format!("{i}.md"), None, None))
            .collect();
        let mut opts = default_opts("https://example.com");
        opts.max_items = 5;
        opts.paginated = true;
        let result = build_feed_from_articles(articles, &opts);

        for (idx, page) in result.pages.iter().enumerate() {
            let ext_map = page.channel.extensions();
            let atom_ext = ext_map.get("atom").expect("atom namespace present");
            let links = atom_ext.get("link").expect("atom:link present");
            let self_link = links
                .iter()
                .find(|l| l.attrs().get("rel").map(String::as_str) == Some("self"))
                .expect("rel=self link");
            let href = self_link.attrs().get("href").unwrap();
            let expected_filename = rss_filename(idx);
            assert!(
                href.ends_with(&expected_filename),
                "page {idx} self link '{href}' should end with '{expected_filename}'"
            );
        }
    }

    #[test]
    fn pagination_first_page_has_next_but_no_prev() {
        let articles: Vec<Article> = (0..11)
            .map(|i| make_article(&format!("Post {i}"), &format!("{i}.md"), None, None))
            .collect();
        let mut opts = default_opts("https://example.com");
        opts.max_items = 5;
        opts.paginated = true;
        let result = build_feed_from_articles(articles, &opts);

        let ext_map = result.pages[0].channel.extensions();
        let links = ext_map["atom"]["link"].as_slice();
        let rels: Vec<&str> = links
            .iter()
            .filter_map(|l| l.attrs().get("rel").map(String::as_str))
            .collect();
        assert!(rels.contains(&"next"), "first page should have next");
        assert!(!rels.contains(&"prev"), "first page should not have prev");
    }

    #[test]
    fn pagination_last_page_has_prev_but_no_next() {
        let articles: Vec<Article> = (0..11)
            .map(|i| make_article(&format!("Post {i}"), &format!("{i}.md"), None, None))
            .collect();
        let mut opts = default_opts("https://example.com");
        opts.max_items = 5;
        opts.paginated = true;
        let result = build_feed_from_articles(articles, &opts);

        let last = result.pages.last().unwrap();
        let ext_map = last.channel.extensions();
        let links = ext_map["atom"]["link"].as_slice();
        let rels: Vec<&str> = links
            .iter()
            .filter_map(|l| l.attrs().get("rel").map(String::as_str))
            .collect();
        assert!(rels.contains(&"prev"), "last page should have prev");
        assert!(!rels.contains(&"next"), "last page should not have next");
    }

    #[test]
    fn no_pagination_when_items_fit_in_one_page() {
        let articles: Vec<Article> = (0..3)
            .map(|i| make_article(&format!("Post {i}"), &format!("{i}.md"), None, None))
            .collect();
        let mut opts = default_opts("https://example.com");
        opts.max_items = 10;
        opts.paginated = true;
        let result = build_feed_from_articles(articles, &opts);
        // 3 items fits within max_items=10 — should be a single page.
        assert_eq!(result.pages.len(), 1);
    }

    #[test]
    fn build_feed_from_articles_empty_articles_returns_one_empty_page() {
        let opts = default_opts("https://example.com");
        let result = build_feed_from_articles(vec![], &opts);
        assert_eq!(result.pages.len(), 1);
        assert_eq!(result.pages[0].channel.items().len(), 0);
    }
}
