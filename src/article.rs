//! Scanning an mdBook `src` directory into [`Article`]s.

use std::{fs, path::Path, time::SystemTime};

use chrono::{DateTime, Utc};
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
    pub path: String,
}

fn file_stem_or_err(path: &Path) -> Result<String> {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .ok_or_else(|| FeedError::MissingFileStem(path.to_path_buf()))
}

fn fallback_frontmatter(
    path: &Path,
    content: &str,
    fallback_date: Option<DateTime<Utc>>,
) -> Result<FrontMatter> {
    Ok(FrontMatter {
        title: file_stem_or_err(path)?,
        date: fallback_date,
        author: None,
        description: Some(content.to_string()),
    })
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

    let mut lines = text.lines();
    let mut yaml = String::new();
    let mut in_yaml = false;

    // Extract YAML front matter delimited by `---` lines.
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

    // Markdown content after front matter.
    let content = lines.collect::<Vec<_>>().join("\n") + "\n";

    let fallback_date = path
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .map(systemtime_to_utc);

    let fm = if yaml.trim().is_empty() {
        fallback_frontmatter(path, &content, fallback_date)?
    } else {
        match yaml_serde::from_str(&yaml) {
            Ok(fm) => fm,
            Err(_) => fallback_frontmatter(path, &content, fallback_date)?,
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
