//! Scanning an mdBook `src` directory into [`Article`]s.
//!
//! Two collection strategies are provided:
//!
//! - [`articles_from_book_json`]: reads the already-processed book object
//!   that mdBook passes to preprocessors on stdin. Chapter content has
//!   `{{#include}}` directives already expanded, and only chapters listed in
//!   `SUMMARY.md` are present. **Prefer this path.**
//!
//! - [`collect_articles`]: walks the `src/` directory on disk. Kept for
//!   standalone/testing use. Does **not** expand `{{#include}}` directives
//!   and does **not** filter to `SUMMARY.md` entries.

use std::{fs, path::Path, time::SystemTime};

use chrono::{DateTime, Utc};
use serde_json::Value;
use walkdir::WalkDir;

use crate::error::{FeedError, Result};
use crate::frontmatter::{FrontMatter, parse_frontmatter};

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

// ── Book JSON path ────────────────────────────────────────────────────────────

/// Recursively walk a `BookItem` JSON array and collect chapters.
fn walk_book_items(items: &Value, out: &mut Vec<Article>, strict: bool) {
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
            if let Some(sub) = chapter.get("sub_items") {
                walk_book_items(sub, out, strict);
            }
            continue;
        }

        let (fm, body) = parse_frontmatter(&content, &name, None, strict);

        out.push(Article {
            fm,
            content: body,
            path,
        });

        // Recurse into nested chapters.
        if let Some(sub) = chapter.get("sub_items") {
            walk_book_items(sub, out, strict);
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
pub fn articles_from_book_json(book_json: &Value, strict: bool) -> Vec<Article> {
    let mut articles = Vec::new();

    // mdBook's Book serialises its chapters under "items".
    if let Some(items) = book_json.get("items") {
        walk_book_items(items, &mut articles, strict);
    }

    // Sort newest → oldest; None dates fall last.
    articles.sort_by_key(|b| std::cmp::Reverse(b.fm.date));

    articles
}

// ── Filesystem path (legacy / standalone) ────────────────────────────────────

/// Parses a markdown file and returns an [`Article`].
///
/// # Errors
/// Returns `Err` if `path` can't be read, or if it has no usable file stem
/// (e.g. it's a directory or has no filename).
pub fn parse_markdown_file(root: &Path, path: &Path, strict: bool) -> Result<Article> {
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

    let (fm, content) = parse_frontmatter(&text, &title_hint, fallback_date, strict);

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
pub fn collect_articles(src_dir: &Path, strict: bool) -> Result<Vec<Article>> {
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

        if let Ok(article) = parse_markdown_file(src_dir, path, strict) {
            articles.push(article);
        }
    }

    // Sort newest → oldest.
    articles.sort_by_key(|a| a.fm.date);
    articles.reverse();

    Ok(articles)
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;

    // ── articles_from_book_json ───────────────────────────────────────────────

    fn chapter_item(name: &str, content: &str, path: &str) -> serde_json::Value {
        json!({
            "Chapter": {
                "name": name,
                "content": content,
                "source_path": path,
                "sub_items": []
            }
        })
    }

    #[test]
    fn articles_from_book_json_parses_basic_chapter() {
        let book = json!({
            "items": [
                chapter_item("My Post", "---\ntitle: My Post\ndate: 2024-01-15\n---\n\nHello world.", "posts/hello.md")
            ]
        });
        let articles = articles_from_book_json(&book, false);
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].fm.title, "My Post");
        assert_eq!(articles[0].path, "posts/hello.md");
        assert!(articles[0].content.contains("Hello world."));
    }

    #[test]
    fn articles_from_book_json_skips_separators_and_part_titles() {
        let book = json!({
            "items": [
                { "Separator": {} },
                { "PartTitle": "Part One" },
                chapter_item("Real Chapter", "Content.", "chapter.md")
            ]
        });
        let articles = articles_from_book_json(&book, false);
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].path, "chapter.md");
    }

    #[test]
    fn articles_from_book_json_skips_draft_chapters_with_empty_path() {
        let book = json!({
            "items": [
                {
                    "Chapter": {
                        "name": "Draft",
                        "content": "WIP.",
                        "source_path": null,
                        "path": null,
                        "sub_items": []
                    }
                },
                chapter_item("Published", "Content.", "published.md")
            ]
        });
        let articles = articles_from_book_json(&book, false);
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].path, "published.md");
    }

    #[test]
    fn articles_from_book_json_recurses_into_sub_items() {
        let book = json!({
            "items": [
                {
                    "Chapter": {
                        "name": "Parent",
                        "content": "Parent content.",
                        "source_path": "parent.md",
                        "sub_items": [
                            chapter_item("Child", "Child content.", "child.md")
                        ]
                    }
                }
            ]
        });
        let articles = articles_from_book_json(&book, false);
        assert_eq!(articles.len(), 2);
        let paths: Vec<&str> = articles.iter().map(|a| a.path.as_str()).collect();
        assert!(paths.contains(&"parent.md"));
        assert!(paths.contains(&"child.md"));
    }

    #[test]
    fn articles_from_book_json_sorted_newest_first() {
        let book = json!({
            "items": [
                chapter_item("Old", "---\ndate: 2022-01-01\n---\nOld.", "old.md"),
                chapter_item("New", "---\ndate: 2024-06-01\n---\nNew.", "new.md"),
                chapter_item("Mid", "---\ndate: 2023-03-15\n---\nMid.", "mid.md"),
            ]
        });
        let articles = articles_from_book_json(&book, false);
        assert_eq!(articles.len(), 3);
        assert_eq!(articles[0].path, "new.md");
        assert_eq!(articles[1].path, "mid.md");
        assert_eq!(articles[2].path, "old.md");
    }

    #[test]
    fn articles_from_book_json_undated_articles_sort_last() {
        let book = json!({
            "items": [
                chapter_item("Undated", "No date.", "undated.md"),
                chapter_item("Dated", "---\ndate: 2024-01-01\n---\nDated.", "dated.md"),
            ]
        });
        let articles = articles_from_book_json(&book, false);
        assert_eq!(articles.len(), 2);
        assert_eq!(articles[0].path, "dated.md");
        assert_eq!(articles[1].path, "undated.md");
    }

    #[test]
    fn articles_from_book_json_empty_book_returns_empty_vec() {
        let book = json!({ "items": [] });
        let articles = articles_from_book_json(&book, false);
        assert!(articles.is_empty());
    }

    #[test]
    fn articles_from_book_json_missing_items_key_returns_empty() {
        let book = json!({});
        let articles = articles_from_book_json(&book, false);
        assert!(articles.is_empty());
    }

    #[test]
    fn articles_from_book_json_prefers_source_path_over_path() {
        let book = json!({
            "items": [{
                "Chapter": {
                    "name": "Test",
                    "content": "Content.",
                    "source_path": "actual/source.md",
                    "path": "generated/output.html",
                    "sub_items": []
                }
            }]
        });
        let articles = articles_from_book_json(&book, false);
        assert_eq!(articles[0].path, "actual/source.md");
    }

    // ── parse_markdown_file ───────────────────────────────────────────────────

    fn write_temp_file(dir: &std::path::Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn parse_markdown_file_reads_content_and_strips_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp_file(
            dir.path(),
            "post.md",
            "---\ntitle: Hello\ndate: 2024-01-01\n---\n\nBody text here.",
        );
        let article = parse_markdown_file(dir.path(), &path, false).unwrap();
        assert_eq!(article.fm.title, "Hello");
        assert!(article.content.contains("Body text here."));
        assert_eq!(article.path, "post.md");
    }

    #[test]
    fn parse_markdown_file_uses_stem_as_title_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp_file(dir.path(), "my-chapter.md", "No frontmatter here.");
        let article = parse_markdown_file(dir.path(), &path, false).unwrap();
        // Title falls back to file stem when there's no frontmatter or h1.
        assert_eq!(article.fm.title, "my-chapter");
    }

    #[test]
    fn parse_markdown_file_relative_path_strips_root() {
        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();
        let path = write_temp_file(&subdir, "nested.md", "Content.");
        let article = parse_markdown_file(dir.path(), &path, false).unwrap();
        // Path should be relative: "subdir/nested.md"
        assert!(!article.path.starts_with('/'));
        assert!(article.path.contains("nested.md"));
    }

    #[test]
    fn parse_markdown_file_nonexistent_returns_err() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("ghost.md");
        let result = parse_markdown_file(dir.path(), &missing, false);
        assert!(result.is_err());
    }

    // ── collect_articles ─────────────────────────────────────────────────────

    #[test]
    fn collect_articles_walks_directory() {
        let dir = tempfile::tempdir().unwrap();
        write_temp_file(
            dir.path(),
            "a.md",
            "---\ntitle: A\ndate: 2024-02-01\n---\nA content.",
        );
        write_temp_file(
            dir.path(),
            "b.md",
            "---\ntitle: B\ndate: 2024-01-01\n---\nB content.",
        );
        let articles = collect_articles(dir.path(), false).unwrap();
        assert_eq!(articles.len(), 2);
        // Sorted newest first.
        assert_eq!(articles[0].fm.title, "A");
        assert_eq!(articles[1].fm.title, "B");
    }

    #[test]
    fn collect_articles_skips_summary_md() {
        let dir = tempfile::tempdir().unwrap();
        write_temp_file(
            dir.path(),
            "SUMMARY.md",
            "# Summary\n\n- [Chapter](chapter.md)",
        );
        write_temp_file(dir.path(), "chapter.md", "# Chapter\n\nContent.");
        let articles = collect_articles(dir.path(), false).unwrap();
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].fm.title, "Chapter");
    }

    #[test]
    fn collect_articles_skips_non_markdown_files() {
        let dir = tempfile::tempdir().unwrap();
        write_temp_file(dir.path(), "image.png", "fake png bytes");
        write_temp_file(dir.path(), "style.css", "body { color: red; }");
        write_temp_file(dir.path(), "real.md", "# Real\n\nContent.");
        let articles = collect_articles(dir.path(), false).unwrap();
        assert_eq!(articles.len(), 1);
    }

    #[test]
    fn collect_articles_accepts_markdown_extension() {
        let dir = tempfile::tempdir().unwrap();
        write_temp_file(dir.path(), "post.markdown", "# Long Ext\n\nContent.");
        let articles = collect_articles(dir.path(), false).unwrap();
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].fm.title, "Long Ext");
    }

    #[test]
    fn collect_articles_nonexistent_dir_returns_err() {
        let path = PathBuf::from("/tmp/surely_does_not_exist_mdbook_rss_feed_test");
        let result = collect_articles(&path, false);
        assert!(result.is_err());
    }

    /// Regression test: a section-header chapter (`[English]()` in SUMMARY.md)
    /// has no source path.  Its children must still be collected.
    #[test]
    fn articles_from_book_json_draft_section_header_does_not_drop_children() {
        let book = json!({
            "items": [
                chapter_item("Blog", "# Blog", "README.md"),
                {
                    "Chapter": {
                        "name": "English",
                        "content": "",
                        "source_path": null,
                        "path": null,
                        "sub_items": [
                            chapter_item(
                                "Nix : Devbox to Multiverse",
                                "---\ntitle: Nix : Devbox to Multiverse\ndate: 2024-03-01\n---\nContent.",
                                "en/devbox_to_multiverse.md"
                            )
                        ]
                    }
                }
            ]
        });
        let articles = articles_from_book_json(&book, false);
        assert_eq!(
            articles.len(),
            2,
            "child of draft section header must be collected"
        );
        let paths: Vec<&str> = articles.iter().map(|a| a.path.as_str()).collect();
        assert!(paths.contains(&"README.md"));
        assert!(paths.contains(&"en/devbox_to_multiverse.md"));
    }
}
