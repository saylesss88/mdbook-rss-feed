//! Fuzz target for `make_urls_absolute`.
//!
//! Feeds arbitrary HTML strings and base URLs. The function must never panic
//! and must always return a string at least as long as the input (since it
//! only ever adds content, never removes it).
//!
//! Run with:
//!   cargo fuzz run fuzz_make_urls_absolute

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use mdbook_rss_feed::make_urls_absolute;

#[derive(Arbitrary, Debug)]
struct Input {
    html: String,
    base_url: String,
    page_url: Option<String>,
}

fuzz_target!(|input: Input| {
    let out = make_urls_absolute(&input.html, &input.base_url, input.page_url.as_deref());

    // Output must never be shorter than input: we only expand URLs, never
    // remove content.
    assert!(
        out.len() >= input.html.len(),
        "output should not be shorter than input html"
    );
});
