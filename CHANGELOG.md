# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [1.10.0] - 2026-08-11

## [Unreleased]

### Fixed


- Atom entries with no body content no longer emit an empty
  `<content type="html"></content>` element, which was causing validation
  warnings.
- Atom feed-level `<updated>` no longer falls back to the Unix epoch
  (`1970-01-01`) when no entries have dates, which was flagged as an
  implausible date by the W3C validator. Falls back to `2000-01-01` instead.

- Atom chapters with no date in the frontmatter now fallback to a plausible date

- Relative image and link URLs in feed content are now rewritten to absolute
  URLs using `site-url` as the base, so feed readers can display images and
  follow links without visiting the original page.

- Fragment-only links (`#anchor`) are prefixed with the chapter's own URL (e.g.
  `https://example.com/page.html#anchor`) so they resolve correctly when read
  outside the context of the original page.

- RSS `<author>` elements now use the RFC-compliant `email (Name)` format
  required by the RSS 2.0 spec. A plain author name without an email address is
  no longer emitted, which was causing feed validation failures. Set
  `author-email` in `[preprocessor.rss-feed]` to enable author output:

```toml
  [preprocessor.rss-feed]
  author-email = "you@example.com"
```

### Added

- Section in README about feed validation, mentioning that `mdbook-rss-feed`
  generates valid RSS 2.0, Atom 1.0, and JSON Feed feeds.

## [1.9.0] - 2026-08-10

### Fixed

- Atom feeds now include feed-level `<author>` elements sourced from the
  `authors` field in `[book]` in `book.toml`, satisfying the Atom spec
  requirement. Per-entry `<author>` elements are also set when a chapter's
  frontmatter includes an `author:` field.

### Added

- **Test suite**: `frontmatter`, `article`, and `feed` modules now have inline
  `#[cfg(test)]` unit tests (78 tests total, complementing the 6 existing tests
  in `preview`). Coverage includes:
  - frontmatter parsing edge cases (`first_h1`, `resolve_title`,
    `parse_frontmatter`),
  - book JSON collection (`articles_from_book_json`, sub-item recursion,
    draft-chapter skipping, newest-first sort),
  - filesystem collection (`collect_articles`, `parse_markdown_file`),
  - feed filtering (`article_is_included`, `DefaultBehavior`),
  - link generation (`article_link`, `rss_filename`),
  - RSS channel metadata, RFC 2822 zero-padding, and full pagination logic (page
    splits, filenames, `atom:link` `rel` values).

- A `tempfile` dev-dependency is added for filesystem-based tests.

## [1.8.1] - 2026-08-09

### Fixed

- [Bug](https://github.com/saylesss88/mdbook-rss-feed/issues/8):Horizontal rules
  (`---`) in the middle of a document body no longer trigger frontmatter
  parsing. Frontmatter is now only detected when `---` appears on the very first
  line of the file, matching standard frontmatter convention.

- [Bug](https://github.com/saylesss88/mdbook-rss-feed/issues/7):`date:` values
  that are present but unparseable (e.g. `date: invalid`) now produce a proper
  parse error rather than silently falling back to `None`. This means
  `strict = true` now correctly catches malformed dates.

## [1.8.0] - 2026-08-09

### Added

- **RSS pagination links**: paginated RSS feeds now include `atom:link` elements
  with `rel="self"`, `rel="next"`, and `rel="prev"` using the Atom namespace,
  allowing feed readers to discover adjacent pages. The `xmlns:atom` namespace
  is declared on the `<rss>` element per the convention. Single-page feeds get a
  `rel="self"` link only.

### Fixed

- `pubDate` in RSS items now zero-pads single-digit days (e.g. `02 Aug` instead
  of `2 Aug`) to comply with RFC 2822.

- Remove unused `anyhow` dep.

## [1.7.0] - 2026-08-08

### Added

- **Strict mode**: via `strict = true` in `[preprocessor.rss-feed]`, causing the
  book build to fail on any frontmatter parse error, instead of warning and
  continuing. Useful for CI pipelines where silent fallbacks would produce a
  wrong feed without any visible failure.

- **Atom pagination links**: paginated Atom feeds now include `rel="self"`,
  `rel="next"`, and `rel="prev"` link elements per the Atom spec.

- **JSON Feed: `next_url`**: paginated JSON feeds now include `next_url`
  pointing to the next (older) page, per JSON Feed 1.1 spec.

- **Title fallback to `#Heading`**: `title` is now optional frontmatter. The
  resolution order is:
  1. `title:` in YAML frontmatter
  2. First `# Heading` in chapter body
  3. Chapter name from `SUMMARY.md`

### Fixed

- Atom feed `updated` timestamps to use the most recent entry's update time
  instead of the Unix epoch.

- Frontmatter parse errors now print a warning to stderr instead of silently
  falling back to file modification time, making date ordering issues visible.

## [1.6.0] - 2026-08-08

### Changed

- Switched from filesystem scanning to book JSON. The preprocessor now reads
  chapter content from the processed book object mdBook passes on stdin, rather
  than walking `src/` on disk directly. This means `{{#include}}` directives are
  expanded in feed content, and only chapters listed in `SUMMARY.md` appear in
  the feed.

### Added

- Support for `{{include file.rs}}` when processing book content.

- **Feed visibility filtering**: control which chapters appear in the feed with
  `feed: include` / `feed: exclude` in frontmatter, and a book-level
  `default-behavior = "exclude-all"` config option for opt-in mode.

### Fixed

- `supports` handler now exits with code 0 instead of printing `"true"`, fixing
  mdBook preprocessor handshake.

- README now correctly represents behavior

- README now suggests adding `before = ["frontmatter-strip"]` to your
  `book.toml` if you use `mdbook-frontmatter-strip`.

- `cargo-audit` warnings

## [1.5.0 - 1.5.1] - 2026-06-19

### Changed

- **Refactor(lib)**: break down `lib.rs` into sub-modules

### Added

- **Feature gates**: for `atom` and `json-feed`, install with:
  - `cargo install mdbook-rss-feed --features atom,json-feed`

### Fixed

- Clippy lints

- Remove unused/unnecessary features

- Remove `anyhow` from the lib

- Replace depreciated `serde_yaml` crate with the maintained `yaml_serde` crate
