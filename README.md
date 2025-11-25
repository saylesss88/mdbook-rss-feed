# mdbook-rss-feed

An mdBook preprocessor that generates a beautiful, full-content RSS 2.0 feed
(and optional Atom) for your book.

Perfect for blogs, documentation sites, or any mdBook that you want to publish.

## Features

- Full HTML content in `<description>` (not just excerpts)

- Proper XML escaping

- Falls back to file modification time if no date in frontmatter

- Supports `date:` in YAML frontmatter (RFC3339 or `YYYY-MM-DD`)

- Respects `config.book.title`, `config.book.description`, and
  `output.html.site-url`

- Zero-config, just drop it in `book.toml`

## Installation

```bash
cargo install mdbook-rss-feed
```

Version Check:

```bash
mdbook-rss-feed --version
```

## Usage

After Installing Globally:

```toml
[preprocessor.rss-feed]
renderers = ["html"]
```

The `renderers = ["html"]` configuration in the `book.toml` explicitly binds the
preprocessor to run only when mdBook uses the HTML renderer, preventing it from
executing unnecessarily for other output formats like Markdown or PDF.

## Frontmatter

```yaml
---
title: Debugging NixOS modules
date: 2025-11-22
author: saylesss88
description: This chapter covers debugging NixOS modules, focusing on tracing module options
and evaluating merges.
---
```

### Hiding frontmatter in the rendered HTML

mdBook does not natively parse or remove YAML frontmatter from Markdown files,
treating it as plain text during rendering, which can result in the raw YAML
block (e.g., ---\ntitle: "My Chapter"\n---) appearing directly in the generated
HTML output. I am currently working on a crate to implement this, I will call it
`mdbook-frontmatter-strip`.

I will add a note to this README once it's complete.

**Adding a Description for RSS Preview**

The description in the frontmatter is what will be displayed as your file
preview.

The preview should contain the above description.

### License

Apache-2.0
