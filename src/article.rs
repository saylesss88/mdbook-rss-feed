//! Scanning an mdBook `src` directory into [`Article`]s.

use std::{fs, path::Path, time::SystemTime};

use chrono::{DateTime, Utc};
use serde_json::Value;
use walkdir::WalkDir;

use crate::error::{FeedError, Result};
use crate::frontmatter::FrontMatter;

/// Convert file modification time to UTC.
fn systemtime_to_utc(st: SystemTime) -> DateTime<Utc> {
    DateTime::<Utc>::from(st)
}

/// A chapter plus its parsed metadata.
///
/// `Article` holds the frontmatter, full Markdown body, and the path
/// relative to the mdBook `src` root. It is the internal representation
/// used before converting to RSS items.
#[derive(Debug)]
pub struct Article {
    pub fm: FrontMatter,
    pub content: String,
    /// Path relative to the `src/` root (e.g. `"changelog.md"`)
    pub path: String,
}

/// Parse frontmatter from raw Markdown text.
///
/// Returns `(frontmatter, body)` where `body` is the content after the
/// closing `---` delimiter. If no valid frontmatter block is found, a
/// minimal `FrontMatter` is synthesised from `title_hint` and
/// `fallback_date`.
///
/// A warning is printed to stderr when YAML is present but fails to parse,
/// so users can diagnose broken frontmatter instead of silently getting
/// wrong metadata.
pub fn parse_frontmatter(
    raw: &str,
    title_hint: &str,
    fallback_date: Option<DateTime<Utc>>,
) -> (FrontMatter, String) {
    let mut lines = raw.lines();
    let mut yaml = String::new();
    let mut in_yaml = false;

    for line in lines.by_ref() {
        let trimmed = line.trim();
        if trimmed == "---" {
            if !in_yaml {
                in_yaml = true;
                continue;
            }
            break;
        }
        if in_yaml {
            yaml.push_str(line);
            yaml.push('\n');
        }
    }

    let content = lines.collect::<Vec<_>>().join("\n") + "\n";

    let fm = if yaml.trim().is_empty() {
        FrontMatter {
            title: title_hint.to_string(),
            date: fallback_date,
            author: None,
            description: None,
            feed: None,
        }
    } else {
        match yaml_serde::from_str(&yaml) {
            Ok(fm) => fm,
            Err(e) => {
                eprintln!(
                    "mdbook-rss-feed: warning: failed to parse frontmatter for \
                     '{title_hint}', falling back to defaults: {e}"
                );
                FrontMatter {
                    title: title_hint.to_string(),
                    date: fallback_date,
                    author: None,
                    description: None,
                    feed: None,
                }
            }
        }
    };

    (fm, content)
}

fn walk_book_items(items: &Value, out: &mut Vec<Article>) {
    let Some(arr) = items.as_array() else { return };

    for item in arr {
        // Only Chapter variants carry content; Separator and PartTitle are skipped.
        let Some(chapter) = item.get("Chapter") else {
            continue;
        };

        let name = chapter
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("untitled")
            .to_string();

        let content = chapter
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        // `path` is the output HTML path; `source_path` is the .md source.
        // We prefer source_path (e.g. "changelog.md") for URL generation
        // because `path` may sometimes be None for draft chapters.
        let path = chapter
            .get("source_path")
            .and_then(Value::as_str)
            .or_else(|| chapter.get("path").and_then(Value::as_str))
            .unwrap_or("")
            .to_string();

        if path.is_empty() {
            // Draft chapter with no source (skip).
            continue;
        }

        let (fm, body) = parse_frontmatter(&content, &name, None);

        out.push(Article {
            fm,
            content: body,
            path,
        });

        // Recurse into nested chapters.
        if let Some(sub) = chapter.get("sub_items") {
            walk_book_items(sub, out);
        }
    }
}

/// Collect articles from the book JSON object mdBook passes to preprocessors.
///
/// This is the **preferred** collection strategy. The book JSON contains only
/// chapters listed in `SUMMARY.md`, and all `{{#include}}` directives in
/// chapter content have already been expanded by mdBook before this
/// preprocessor is called.
#[must_use]
pub fn articles_from_book_json(book_json: &Value) -> Vec<Article> {
    let mut articles = Vec::new();

    // mdBook's Book serialises its chapters under "items".
    if let Some(items) = book_json.get("items") {
        walk_book_items(items, &mut articles);
    }

    // Sort newest → oldest; None dates fall last.
    articles.sort_by_key(|b| std::cmp::Reverse(b.fm.date));

    articles
}

/// Parses a markdown file and returns an [`Article`].
///
/// # Errors
/// Returns `Err` if `path` can't be read, or if it has no usable file stem
/// (e.g. it's a directory or has no filename).
pub fn parse_markdown_file(root: &Path, path: &Path) -> Result<Article> {
    let text = fs::read_to_string(path).map_err(|source| FeedError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    let fallback_date = path
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .map(systemtime_to_utc);

    let title_hint = path.file_stem().map_or_else(
        || "untitled".to_string(),
        |s| s.to_string_lossy().into_owned(),
    );

    let (fm, content) = parse_frontmatter(&text, &title_hint, fallback_date);

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
/// parses each chapter into an [`Article`], then sorts the list newest →
/// oldest based on frontmatter `date` (falling back to file modification
/// time). Files that fail to parse are skipped rather than aborting the
/// whole scan.
///
/// # Errors
/// Returns `Err` if `src_dir` doesn't exist or can't be walked.
pub fn collect_articles(src_dir: &Path) -> Result<Vec<Article>> {
    let mut articles = Vec::new();

    for entry in WalkDir::new(src_dir) {
        let entry = entry.map_err(|source| FeedError::WalkDir {
            path: src_dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        if !matches!(ext.as_deref(), Some("md" | "markdown")) {
            continue;
        }

        let is_summary = path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("SUMMARY.md"));
        if is_summary {
            continue;
        }

        if let Ok(article) = parse_markdown_file(src_dir, path) {
            articles.push(article);
        }
    }

    // Sort newest → oldest.
    articles.sort_by_key(|a| a.fm.date);
    articles.reverse();

    Ok(articles)
}
