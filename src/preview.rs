//! Markdown → HTML rendering and preview/excerpt extraction.
//!
//! These helpers turn a chapter's full Markdown body (or its frontmatter
//! `description` override) into a short HTML preview suitable for an RSS
//! `<description>`.

use pulldown_cmark::{Options, Parser, html};

/// Minimum body length (in chars) before we prefer it over the frontmatter
/// `description` as the preview source.
pub const MIN_BODY_PREVIEW_CHARS: usize = 80;

/// Maximum amount of markdown (in chars) considered when building a preview,
/// applied before HTML conversion to bound rendering cost on huge chapters.
pub const PREVIEW_MD_SLICE_CHARS: usize = 4000;

/// Render Markdown to HTML using `pulldown_cmark`.
///
/// Used both for full-content feeds and for generating HTML previews from
/// chapter bodies or frontmatter descriptions.
pub fn markdown_to_html(md: &str) -> String {
    let mut html = String::new();
    let parser = Parser::new_ext(md, Options::all());
    html::push_html(&mut html, parser);
    html
}

/// Strip obvious leading boilerplate (TOCs, details, long definition blocks)
/// so previews tend to start at the main intro text instead of metadata or
/// navigation.
///
/// Heuristic: find the first heading, then return everything after the
/// first blank line that follows it. If no heading is found, the input is
/// returned unchanged.
pub fn strip_leading_boilerplate(md: &str) -> &str {
    let mut seen_heading = false;
    let mut byte_idx = 0;
    let mut acc_bytes = 0;

    for (i, line) in md.lines().enumerate() {
        let line_len_with_nl = line.len() + 1; // assume '\n'-separated

        // Skip a single leading blank line entirely.
        if i == 0 && line.trim().is_empty() {
            acc_bytes += line_len_with_nl;
            continue;
        }

        if line.trim_start().starts_with('#') {
            seen_heading = true;
        }

        if seen_heading && line.trim().is_empty() {
            // First blank line after heading: preview starts after this.
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

/// Take at most `max_chars` worth of UTF-8 text from `s`.
pub fn utf8_prefix(s: &str, max_chars: usize) -> &str {
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
/// Used to build the `<description>` preview for each item. At most
/// `max_paragraphs` paragraphs are included, and the result is truncated to
/// `max_chars` characters (UTF-8 safe). If no `<p>` is found, the original
/// HTML is returned unchanged.
pub fn html_first_paragraphs(html: &str, max_paragraphs: usize, max_chars: usize) -> String {
    let mut out = String::new();
    let mut start = 0;
    let mut count = 0;

    while count < max_paragraphs {
        let Some(rel) = html[start..].find("<p") else {
            break;
        };
        let p_start = start + rel;

        let Some(rel_close) = html[p_start..].find("</p>") else {
            break;
        };
        let close = p_start + rel_close + "</p>".len();

        out.push_str(&html[p_start..close]);
        count += 1;
        start = close;
    }

    if out.is_empty() {
        out = html.to_string();
    }

    if out.chars().count() > max_chars {
        out.chars().take(max_chars).collect()
    } else {
        out
    }
}

/// Rewrite relative URLs in HTML to absolute ones using `base_url`.
///
/// Rewrites `src="..."` and `href="..."` attributes. Skips URLs that are
/// already absolute (`http://`, `https://`, `//`) or fragment-only (`#`).
pub fn make_urls_absolute(html: &str, base_url: &str, page_url: Option<&str>) -> String {
    let base = base_url.trim_end_matches('/');

    let is_absolute = |url: &str| -> bool {
        url.starts_with("http://")
            || url.starts_with("https://")
            || url.starts_with("//")
            || url.is_empty()
    };

    let mut result = String::with_capacity(html.len() + 64);
    let mut rest = html;

    while !rest.is_empty() {
        // Find the next src=" or href=" occurrence.
        let next_src = rest.find("src=\"");
        let next_href = rest.find("href=\"");

        let (pos, prefix_len) = match (next_src, next_href) {
            (None, None) => break,
            (Some(s), None) => (s, 5),
            (None, Some(h)) => (h, 6),
            (Some(s), Some(h)) => {
                if s <= h {
                    (s, 5)
                } else {
                    (h, 6)
                }
            }
        };

        // Copy everything up to and including the attribute prefix.
        result.push_str(&rest[..pos + prefix_len]);
        rest = &rest[pos + prefix_len..];

        // Find the closing quote.
        if let Some(end) = rest.find('"') {
            let url = &rest[..end];

            if is_absolute(url) {
                // Already absolute, copy as-is
                result.push_str(url);
            } else if url.starts_with('#') {
                if let Some(page) = page_url {
                    result.push_str(page.trim_end_matches('/'));
                }
                result.push_str(url);
            } else {
                let path = url.trim_start_matches('/');
                result.push_str(base);
                result.push('/');
                result.push_str(path);
            }
            result.push('"');
            rest = &rest[end + 1..];
        }
    }

    result.push_str(rest);
    result
}

/// Choose and render a preview source for an article body.
///
/// When `full_preview` is `true`, the entire body is rendered to HTML.
/// Otherwise a hybrid heuristic picks between the body and the frontmatter
/// `description` (preferring the body once it's long enough), strips
/// leading boilerplate, slices it down before HTML conversion, and finally
/// keeps only the first few rendered paragraphs.
pub fn render_preview(
    content: &str,
    description: Option<&str>,
    full_preview: bool,
    base_url: &str,
    page_url: Option<&str>,
) -> String {
    let html = if full_preview {
        markdown_to_html(content)
    } else {
        let content_trimmed = content.trim();
        let body_len = content_trimmed.chars().count();

        let source_md = if body_len >= MIN_BODY_PREVIEW_CHARS || description.is_none() {
            content_trimmed
        } else {
            description.unwrap_or(content_trimmed)
        };

        let source_md = strip_leading_boilerplate(source_md);
        let source_md = utf8_prefix(source_md, PREVIEW_MD_SLICE_CHARS);

        let raw_html = markdown_to_html(source_md);
        html_first_paragraphs(&raw_html, 3, 800)
    };

    make_urls_absolute(&html, base_url, page_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_leading_boilerplate_skips_to_after_heading() {
        let md = "\n# Title\n\nReal content starts here.";
        assert_eq!(strip_leading_boilerplate(md), "Real content starts here.");
    }

    #[test]
    fn strip_leading_boilerplate_no_heading_is_noop() {
        let md = "Just a paragraph, no heading.";
        assert_eq!(strip_leading_boilerplate(md), md);
    }

    #[test]
    fn utf8_prefix_respects_char_boundaries() {
        let s = "héllo wörld"; // contains multi-byte chars
        let prefix = utf8_prefix(s, 5);
        assert_eq!(prefix.chars().count(), 5);
    }

    #[test]
    fn html_first_paragraphs_extracts_up_to_limit() {
        let html = "<p>one</p><p>two</p><p>three</p>";
        let out = html_first_paragraphs(html, 2, 1000);
        assert_eq!(out, "<p>one</p><p>two</p>");
    }

    #[test]
    fn html_first_paragraphs_falls_back_without_p_tags() {
        let html = "<div>no paragraphs here</div>";
        let out = html_first_paragraphs(html, 2, 1000);
        assert_eq!(out, html);
    }

    #[test]
    fn render_preview_full_uses_whole_body() {
        let body = "# Heading\n\nSome content.";
        let out = render_preview(body, None, true, "https://example.com", None);
        assert!(out.contains("Some content."));
        assert!(out.contains("<h1>"));
    }

    #[test]
    fn make_urls_absolute_rewrites_relative_src() {
        let html = r#"<img src="images/foo.png"><img src="https://example.com/bar.png">"#;
        let out = make_urls_absolute(html, "https://example.com/", None);
        assert!(out.contains(r#"src="https://example.com/images/foo.png""#));
        assert!(out.contains(r#"src="https://example.com/bar.png""#));
    }

    #[test]
    fn make_urls_absolute_rewrites_relative_href() {
        let html = r#"<a href="chapter/page.html">link</a>"#;
        let out = make_urls_absolute(html, "https://example.com", None);
        assert!(out.contains(r#"href="https://example.com/chapter/page.html""#));
    }
}
