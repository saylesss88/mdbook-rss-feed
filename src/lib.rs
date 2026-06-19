//! mdbook-rss-feed core library.
//!
//! This module scans an mdBook src directory for chapters, extracts frontmatter
//! and content, and turns them into one or more RSS 2.0 channels suitable for
//! static hosting.

mod article;
mod error;
mod feed;
mod frontmatter;
mod preview;

#[cfg(feature = "atom")]
mod atom_feed;
#[cfg(feature = "json-feed")]
mod json_feed;

// Re-exports
pub use article::{collect_articles, parse_markdown_file, Article};
#[cfg(feature = "atom")]
pub use atom_feed::rss_to_atom;
pub use error::{FeedError, Result};
pub use feed::{build_feed, BuildResult, FeedOptions, FeedPage};
pub use frontmatter::FrontMatter;
#[cfg(feature = "json-feed")]
pub use json_feed::{rss_to_json_feed, JsonFeed, JsonFeedItem};
