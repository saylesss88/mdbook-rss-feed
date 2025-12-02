use anyhow::Result;
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use pulldown_cmark::{html, Options, Parser};
use rss::{Channel, ChannelBuilder, Guid, Item, ItemBuilder};
use serde::{Deserialize, Deserializer};
use std::{fs, path::Path, time::SystemTime};
use walkdir::WalkDir;

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

#[derive(Debug, Deserialize, Clone)]
pub struct FrontMatter {
    pub title: String,

    #[serde(deserialize_with = "deserialize_date")]
    pub date: Option<DateTime<Utc>>,

    pub author: Option<String>,
    pub description: Option<String>, // User-supplied summary (optional)
}

#[derive(Debug)]
pub struct Article {
    pub fm: FrontMatter,
    pub content: String,
    pub path: String,
}

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

fn markdown_to_html(md: &str) -> String {
    let mut html = String::new();
    let parser = Parser::new_ext(md, Options::all());
    html::push_html(&mut html, parser);
    html
}

/// Strip obvious leading boilerplate (TOCs, details, long definition blocks)
/// so previews tend to start at the main intro text instead of metadata.
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

/// Take up to `max_paragraphs` <p> blocks from HTML, and cap at `max_chars` (UTF-8 safe).
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

// One generated feed page
pub struct FeedPage {
    pub filename: String, // e.g. "rss.xml", "rss2.xml"
    pub channel: Channel,
}

// Result for the whole build
pub struct BuildResult {
    pub pages: Vec<FeedPage>,
}

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
                .generator(Some("mdbook-rss-feed 0.1.0".to_string()))
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
