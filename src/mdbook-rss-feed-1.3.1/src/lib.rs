//! mdbook-rss-feed core library.
//!
//! This module scans an mdBook src directory for chapters, extracts frontmatter
//! and content, and turns them into one or more RSS 2.0 channels suitable for
//! static hosting.

use anyhow::Result;
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use pulldown_cmark::{html, Options, Parser};
use rss::{Channel, ChannelBuilder, Guid, Item, ItemBuilder};
use serde::{Deserialize, Deserializer};
use serde_json::Value as JsonValue;
use std::{fs, path::Path, time::SystemTime};
use walkdir::WalkDir;

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
    pub next_url: Option<String>, // <-- add this
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
    pub author: Option<JsonValue>, // allow simple or richer authors later
}

// Optional Atom support
use atom_syndication::{
    Content as AtomContent, Entry as AtomEntry, Feed as AtomFeed, Link as AtomLink,
    Text as AtomText,
};

// Minimum body length (in chars) before we prefer it over description
const MIN_BODY_PREVIEW_CHARS: usize = 80;

// Convert file modification time → UTC
fn systemtime_to_utc(st: SystemTime) -> DateTime<Utc> {
    DateTime::<Utc>::from(st)
}

// Parse front-matter date formats
fn deserialize_date<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;

    if let Some(date_str) = s {
        if let Ok(dt) = DateTime::parse_from_rfc3339(&date_str) {
            return Ok(Some(dt.with_timezone(&Utc)));
        }

        if let Ok(nd) = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
            return Ok(Some(
                Utc.from_utc_datetime(&nd.and_hms_opt(0, 0, 0).unwrap()),
            ));
        }
    }
    Ok(None)
}

/// Parsed YAML frontmatter for a single chapter.
///
/// Fields are used for feed metadata:
/// - `title`: item title shown in the feed.
/// - `date`: publish date for sorting and `pubDate` (RFC3339 or `YYYY-MM-DD`).
/// - `author`: optional item author.
/// - `description`: optional summary/preview override.
#[derive(Debug, Deserialize, Clone)]
pub struct FrontMatter {
    pub title: String,

    #[serde(deserialize_with = "deserialize_date")]
    pub date: Option<DateTime<Utc>>,

    pub author: Option<String>,
    pub description: Option<String>, // User-supplied summary (optional)
}

/// A chapter plus its parsed metadata.
///
/// `Article` holds the frontmatter, full Markdown body, and the path relative
/// to the mdBook `src` root. It is the internal representation used before
/// converting to RSS items.
#[derive(Debug)]
pub struct Article {
    pub fm: FrontMatter,
    pub content: String,
    pub path: String,
}

/// Parse a single Markdown file into `Article`.
///
/// This looks for a leading `---`…`---` YAML frontmatter block, parses it into
/// `FrontMatter`, and treats the rest of the file as the chapter body. When no
/// frontmatter is found or parsing fails, reasonable defaults are used and the
/// file's modification time becomes the fallback date.
pub fn parse_markdown_file(root: &Path, path: &Path) -> Result<Article> {
    let text = fs::read_to_string(path)?;

    let mut lines = text.lines();
    let mut yaml = String::new();
    let mut in_yaml = false;

    // Extract YAML front matter
    for line in lines.by_ref() {
        let trimmed = line.trim();
        if trimmed == "---" {
            if !in_yaml {
                in_yaml = true;
                continue;
            } else {
                break;
            }
        }
        if in_yaml {
            yaml.push_str(line);
            yaml.push('\n');
        }
    }

    // Markdown content after front matter
    let content = lines.collect::<Vec<_>>().join("\n") + "\n";

    let fallback_date = path
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .map(systemtime_to_utc);

    // Parse front matter
    let fm = if !yaml.trim().is_empty() {
        serde_yaml::from_str(&yaml).unwrap_or_else(|_| FrontMatter {
            title: path.file_stem().unwrap().to_string_lossy().into_owned(),
            date: fallback_date,
            author: None,
            description: Some(content.clone()),
        })
    } else {
        FrontMatter {
            title: path.file_stem().unwrap().to_string_lossy().into_owned(),
            date: fallback_date,
            author: None,
            description: Some(content.clone()),
        }
    };

    let rel_path = path.strip_prefix(root).unwrap_or(path);

    Ok(Article {
        fm,
        content,
        path: rel_path.to_string_lossy().into_owned(),
    })
}

/// Collect all Markdown chapters under `src_dir`.
///
/// Walks the directory tree, skipping `SUMMARY.md` and non-Markdown files,
/// parses each chapter into an `Article`, then sorts the list newest → oldest
/// based on frontmatter `date` (falling back to file modification time).
pub fn collect_articles(src_dir: &Path) -> Result<Vec<Article>> {
    let mut articles = Vec::new();

    for entry in WalkDir::new(src_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase());

        if !matches!(ext.as_deref(), Some("md" | "markdown")) {
            continue;
        }

        if path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .eq_ignore_ascii_case("SUMMARY.md")
        {
            continue;
        }

        if let Ok(article) = parse_markdown_file(src_dir, path) {
            articles.push(article);
        }
    }

    // Sort newest → oldest
    articles.sort_by_key(|a| a.fm.date);
    articles.reverse();

    Ok(articles)
}

/// Render Markdown to HTML using `pulldown_cmark`.
///
/// This is used both for full-content feeds and for generating HTML previews
/// from chapter bodies or frontmatter descriptions.
fn markdown_to_html(md: &str) -> String {
    let mut html = String::new();
    let parser = Parser::new_ext(md, Options::all());
    html::push_html(&mut html, parser);
    html
}

/// Strip obvious leading boilerplate (TOCs, details, long definition blocks)
/// so previews tend to start at the main intro text instead of metadata or
/// navigation.
fn strip_leading_boilerplate(md: &str) -> &str {
    let mut seen_heading = false;
    let mut byte_idx = 0;
    let mut acc_bytes = 0;

    for (i, line) in md.lines().enumerate() {
        let line_len_with_nl = line.len() + 1; // assume '\n' separated

        // Skip initial blank lines entirely
        if i == 0 && line.trim().is_empty() {
            acc_bytes += line_len_with_nl;
            continue;
        }

        if line.trim_start().starts_with('#') {
            seen_heading = true;
        }

        if seen_heading && line.trim().is_empty() {
            // First blank line after heading: start preview after this
            acc_bytes += line_len_with_nl;
            byte_idx = acc_bytes;
            break;
        }

        acc_bytes += line_len_with_nl;
    }

    if byte_idx == 0 {
        md
    } else {
        &md[byte_idx.min(md.len())..]
    }
}

/// Take at most `max_chars` worth of UTF‑8 text from `s`.
fn utf8_prefix(s: &str, max_chars: usize) -> &str {
    if max_chars == 0 {
        return "";
    }

    let mut last_byte = 0;

    for (ch_idx, (byte_idx, _)) in s.char_indices().enumerate() {
        if ch_idx == max_chars {
            last_byte = byte_idx;
            break;
        }
        last_byte = byte_idx + 1;
    }

    if last_byte == 0 || last_byte >= s.len() {
        s
    } else {
        &s[..last_byte]
    }
}

/// Return the first few `<p>` blocks from an HTML fragment.
///
/// This is used to build the `<description>` preview for each item. At most
/// `max_paragraphs` paragraphs are included, and the result is truncated to
/// `max_chars` characters (UTF‑8 safe). If no `<p>` is found, the original
/// HTML is returned unchanged.
fn html_first_paragraphs(html: &str, max_paragraphs: usize, max_chars: usize) -> String {
    let mut out = String::new();
    let mut start = 0;
    let mut count = 0;

    while count < max_paragraphs {
        // Find next <p ...>
        let rel = match html[start..].find("<p") {
            Some(i) => i,
            None => break,
        };
        let p_start = start + rel;

        // Find the end of this paragraph
        let rel_close = match html[p_start..].find("</p>") {
            Some(i) => i,
            None => break,
        };
        let close = p_start + rel_close + "</p>".len();

        let para = &html[p_start..close];
        out.push_str(para);
        count += 1;
        start = close;
    }

    // If no <p> found, fall back to original HTML
    if out.is_empty() {
        out = html.to_string();
    }

    // UTF‑8 safe trim by character count
    if out.chars().count() > max_chars {
        out.chars().take(max_chars).collect()
    } else {
        out
    }
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
                .or_else(|| item.link().map(|l| l.to_string()))
                .unwrap_or_else(|| item.title().unwrap_or("").to_string());

            let url = item.link().map(|l| l.to_string());
            let title = item.title().map(|t| t.to_string());
            let content_html = item.description().map(|d| d.to_string());
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
        feed_url: feed_url.map(|u| u.to_string()),
        description: Some(channel.description().to_string()),
        next_url: next_url.map(|u| u.to_string()),
        items,
    }
}
/// Convert an RSS 2.0 channel into a minimal Atom 1.0 feed.
///
/// This is a best-effort mapping used when `atom = true` in the configuration.
/// It copies titles, links, descriptions (as HTML content), and dates where
/// available.
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
                .or_else(|| item.link().map(|l| l.to_string()))
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
    if !link.is_empty() {
        feed.set_links(vec![AtomLink {
            href: link.to_string(),
            ..Default::default()
        }]);
        // Use the public feed URL as a stable Atom feed id
        feed.set_id(link.to_string());
    } else {
        // Fallback id if link is somehow empty
        feed.set_id(channel.title().to_string());
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
///
/// On success, the caller is responsible for writing each `FeedPage`'s channel
/// to disk at `pages[i].filename`.
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

                // Only consider the first slice of markdown for preview
                const PREVIEW_MD_SLICE_CHARS: usize = 4000;
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
                value: link.clone(),
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
