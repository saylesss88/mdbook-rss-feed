//! Fuzz target for `parse_frontmatter`
#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use mdbook_rss_feed::parse_frontmatter;

#[derive(Arbitrary, Debug)]
struct Input {
    /// Raw chapter content (may or may not contain a frontmatter block)
    raw: String,
    /// The chapter name hint used as a title fallback
    title_hint: String,
}
fuzz_target!(|input: Input| {
    // `strict = false` so we never call `process::exit` during fuzzing.
    let (fm, body) = parse_frontmatter(&input.raw, &input.title_hint, None, false);

    // Basic invariants that must always hold:
    // - title is never empty (falls back to title_hint or h1)
    assert!(
        !fm.title.is_empty() || input.title_hint.is_empty(),
        "title should not be empty when title_hint is non-empty"
    );
    // - body + frontmatter block together should not be longer than raw input
    assert!(
        body.len() <= input.raw.len() + 1,
        "body should not be longer than raw input"
    );
});
