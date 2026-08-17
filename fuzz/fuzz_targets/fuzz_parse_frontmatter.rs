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

    // Title must never be empty: falls back to title_hint, first # heading,
    // or "Untitled" when all three are absent.
    assert!(!fm.title.is_empty(), "title should never be empty");

    // Use both outputs to prevent the optimizer removing the call entirely.
    let _ = body.len();
    let _ = fm.date;
});
