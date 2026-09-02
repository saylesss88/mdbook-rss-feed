# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **`icon` and `favicon` for JSON Feed**: set feed-level image URLs in
  `book.toml` for display in feed reader timelines and source lists:

```toml
  [preprocessor.rss-feed]
  icon    = "https://example.com/icon-512.png"
  favicon = "https://example.com/favicon-64.png"
```

- **Author fallback chain**: when no `author:` is set in chapter frontmatter,
  the first entry in `[book] authors` is used as a fallback. Per-chapter
  `author_email:` frontmatter field added to override the book-level
  `author-email` for individual chapters.

- Warning for people who set `author` in frontmatter without setting
  `author-email` in `book.toml`.
- **Language support**: add `lang` to chapter frontmatter to declare the chapter
  language as a BCP-47 tag (e.g. `en`, `fr`, `en-US`). Appears as `"language"`
  per item in JSON Feed 1.1 and as `xml:lang` on the Atom feed element (set from
  the first chapter that declares a language). Has no effect on RSS, which only
  supports language at the feed level.

- **Tag/category support** add `tags` to chapter frontmatter to populate
  `<category>` elements in RSS and Atom, and the `tags` array in JSON Feed 1.1:

```yaml
  ---
  title: My Post
  date: 2026-08-01
  tags:
    - nix
    - nixos
  ---
```

- Atom & RSS both use `<category>`, while JSON feed uses `"tags"` in the output.

### Fixed

- Reverted back to `"url": "mailto:sayls8@proton.me"` for `feed.json` to make
  the feed pass validation. `author` & `author-email` are mainly used for RSS.

- When setting both `author` and `author-email` in frontmatter, `feed.json` had
  this:

```json
      "author": {
        "name": "sayls8",
        "url": "mailto:sayls8@proton.me"
      },
```

Changed the `author` variable in `json_feed.rs` and now it produces what most
feed readers would expect:

```json

     "author": {
        "email": "sayls8@proton.me",
        "name": "sayls8"
      },
```

- `RawFrontmatter` field is `author_email`, when YAML uses hyphens. Added a
  `#[serde(rename = "author-email")]` to the field.
- `strict = true` now fails the build when a chapter has `author:` in
  frontmatter but no `author-email` is configured. Without strict mode a warning
  is printed instead.
- The author warning no longer fires for every chapter when `[book] authors` is
  set but `author-email` is not. The warning only triggers when `author:` is
  explicitly declared in a chapter's own frontmatter.

- Failing tests, `yaml_serde` didn't respect the default attribute used in
  `RawFrontmatter`

- `flake.nix` in crate root. Now has the correct license and `buildFeatures`
  override support.

## [1.10.2] - 2026-08-24

### Fixed

- [Issue #15](https://github.com/saylesss88/mdbook-rss-feed/issues/15), -
  `date:` values using a space separator instead of `T` (e.g.
  `2026-08-25 12:56:03+02:00`, the format output by `date --rfc-3339=second`)
  are now parsed correctly. Previously these fell back to date-only parsing,
  losing the time component and producing incorrect feed ordering. This could
  cause federation tools (e.g. Bridgy Fed) to repost old content or miss new
  posts entirely.

- [Issue #14](https://github.com/saylesss88/mdbook-rss-feed/issues/14), children
  were silently dropped on empty draft chapters. Now, when `path.is_empty()`, it
  will still recurse into `sub_items` _before_ the `continue`.

### Added

- Regression test for `Issue #14`.

### Changed

- Update dependencies

## [1.10.1] - 2026-08-17

### Changed

- Update deps
- `parse_frontmatter` is now split into `split_frontmatter` (delimiter
  detection) and `parse_frontmatter` (semantic interpretation), making each
  function independently testable and the logic easier to follow.
- Empty YAML blocks (opening and closing `---` with nothing between) are now
  normalized to `None` before matching, removing a redundant arm from the match.

### Fixed

- [Issue atom or json-feed without feature and with strict should fail #12](https://github.com/saylesss88/mdbook-rss-feed/issues/12).
  - Now when you set `json-feed = true`/`atom = true` without adding the
    features it fails the build with this output:

```sh
 INFO Book building has started
mdbook-rss-feed: collected 8 chapter(s) from book (default-behavior: IncludeAll)
Writing RSS page /home/jr/privacy-book/src/rss.xml (259709 bytes)
Writing RSS page /home/jr/privacy-book/src/rss2.xml (22852 bytes)
error: mdbook-rss-feed: `json-feed = true` is set but this binary was compiled without the `json-feed` feature. Reinstall with: cargo install mdbook-rss-feed --features json-feed
ERROR The "rss-feed" preprocessor exited unsuccessfully with exit status: 1 status
```

- [Issue Outputs RSS but not Atom or JSON #11](https://github.com/saylesss88/mdbook-rss-feed/issues/11).
  Paginated output now displays Writing JSON/Atom page in the output.

- `utf8_prefix` in `preview.rs` panicked on multibyte UTF-8 characters (e.g.
  `ç`, emoji, CJK) when slicing preview content. The byte index calculation used
  `byte_idx + 1` which assumed single-byte characters. Replaced with
  `char_indices().nth(max_chars)` which always lands on a valid char boundary.
  Found by `cargo fuzz`.

### Added

- Merged
  [pineage404 PR Add example usage #13](https://github.com/saylesss88/mdbook-rss-feed/pull/13)

- Fuzzing with `cargo-fuzz` and `arbitrary`

Run a target:

```bash
cargo fuzz run fuzz_parse_frontmatter
cargo fuzz run fuzz_make_urls_absolute
cargo fuzz run fuzz_build_feed
```

Each runs until it finds a panic or you stop it with `Ctrl-C`. Corpus inputs
that find new coverage are saved in `fuzz/corpus/<target>/`. If a crash is
found, it's saved in `fuzz/artifacts/<target>/`.

## [1.10.0] - 2026-08-11

### Fixed

- Atom entries with no body content no longer emit an empty
  `<content type="html"></content>` element, which was causing validation
  warnings.
- Atom feed-level `<updated>` no longer falls back to the Unix epoch
  (`1970-01-01`) when no entries have dates, which was flagged as an implausible
  date by the W3C validator. Falls back to `2000-01-01` instead.

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
