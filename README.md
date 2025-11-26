# mdbook-rss-feed

An mdBook preprocessor that generates a beautiful RSS 2.0 feed (and optional
Atom) for your book, with HTML previews for each chapter.

Perfect for blogs, documentation sites, or any mdBook that you want to publish.

Sorry for the frequent updates, this should be close to stable now.

## Features

- HTML preview in `<description>` built from the first paragraphs of each
  chapter

- Hybrid preview source:
  - Prefer chapter body content for the preview
  - Fall back to `description` in frontmatter when the body is empty or very
    short

- Proper XML escaping via the `rss` crate

- Falls back to file modification time if no date in frontmatter

- Supports `date:` in YAML frontmatter (RFC3339 or `YYYY-MM-DD`)

- Respects `config.book.title`, `config.book.description`, and
  `output.html.site-url`

- Zero-config, just drop it in `book.toml`

- Works with or without YAML frontmatter

## Installation

```bash
cargo install mdbook-rss-feed
```

Version Check:

```bash
mdbook-rss-feed --version
```

Tested against:

- mdBook v0.4.40 & v0.5.1
- Rust editions 2020 & 2024

## Usage

After installing globally, add the following to your mdbook's `book.toml`:

```toml
[book]
title = "your-title"
author = "your-author"
language = "your-lang"
src = "src"

[preprocessor.rss-feed]
renderers = ["html"]

[output.html]
site-url = "https://your-user.github.io/"
```

The `renderers = ["html"]` configuration in `book.toml` explicitly binds the
preprocessor to run only when mdBook uses the HTML renderer, preventing it from
executing unnecessarily for other output formats like Markdown or PDF.

- If you don't give your book a `title`, it will be displayed as `My mdbook`.

- If you don't give a `site-url`, the default is `example.com`. Use the public
  base URL of your deployed site, `site-url = "https://your-user.github.io/"` is
  just an example of what a gh-pages site could be.

- In the above example, the RSS feed would be located at:
  `https://your-user.github.io/rss.xml`

## Frontmatter

Frontmatter is optional, without it only the chapter title, book name, date, and
time are listed.

Adding frontmatter is useful to customize any of those settings as well as add
the author and short description.

```yaml
title: Debugging NixOS modules
date: 2025-11-22
author: saylesss88
description: This chapter covers debugging NixOS modules, focusing on tracing module
options and evaluating merges.
```

### How Feed Preview is Generated

The feed preview is generated from the rendered HTML of the chapter. The crate
looks for `<p>…</p>` blocks (HTML paragraphs) and takes the first 2–3
paragraphs, up to 800 characters total.​

If your chapter starts with non-paragraph content (e.g., lists, details blocks,
or other custom markup), the preview will begin at the first real HTML paragraph
that appears after that content.​

To override this behavior, set `description` in the YAML frontmatter; that text
will be used as the preview when the chapter body is short.​

- **Default behavior:** The RSS preview is generated from the first few
  paragraphs of the chapter body.

- **Fallback behavior:** If the chapter body is empty or extremely short, the
  preview is generated from the `description` field instead.

- This makes `description` a good place for a short, human‑written summary,
  while still keeping the preview in sync with the chapter content in normal
  cases.

If you prefer not to rely on this fallback at all, you can simply omit
`description` in your frontmatter; the preview will always come from the chapter
body.

### Hiding frontmatter in the rendered HTML

mdBook does not natively parse or remove YAML frontmatter from Markdown files,
treating it as plain text during rendering, which can result in the raw YAML
block (e.g., `---\ntitle: "My Chapter"\n---`) appearing directly in the
generated HTML output.

To avoid this, you can use:

[mdbook-frontmatter-strip](https://crates.io/crates/mdbook-frontmatter-strip)

### License

Apache-2.0
