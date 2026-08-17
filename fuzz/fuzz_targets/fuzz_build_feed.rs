//! Fuzz target for the full article → feed pipeline.
//!
//! Constructs an `Article` from arbitrary input and runs it through
//! `build_feed_from_articles`. The goal is to confirm the entire pipeline
//! never panics on arbitrary content.
//!
//! Run with:
//!   cargo fuzz run fuzz_build_feed

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use mdbook_rss_feed::{
    Article, DefaultBehavior, FeedOptions, build_feed_from_articles, parse_frontmatter,
};

#[derive(Arbitrary, Debug)]
struct Input {
    /// Raw chapter content fed through the full parse + build pipeline
    raw: String,
    /// Simulated src-relative path (e.g. "chapter/page.md")
    path: String,
    full_preview: bool,
}

fuzz_target!(|input: Input| {
    let (fm, content) = parse_frontmatter(&input.raw, "fuzz-chapter", None, false);

    let articles = vec![Article {
        fm,
        content,
        path: input.path,
    }];

    let opts = FeedOptions {
        title: "Fuzz Book",
        site_url: "https://example.com",
        description: "Fuzz feed",
        full_preview: input.full_preview,
        max_items: 0,
        paginated: false,
        default_behavior: DefaultBehavior::IncludeAll,
        strict: false,
        author_email: None,
    };

    let result = build_feed_from_articles(articles, &opts);

    // Must always produce at least one page.
    assert!(
        !result.pages.is_empty(),
        "build must always produce at least one page"
    );
});
