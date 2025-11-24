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
- Zero-config — just drop it in `book.toml`

## Installation

```bash
cargo install mdbook-rss-feed
```

## Usage

After Installing Globally:

```toml
[preprocessor.rss-feed]
command = "mdbook-rss-feed"
```

**Example Frontmatter**

```yaml
---
title: My Great Post
date: 2025-11-23
author: Jane Doe
description: Optional short description (otherwise first paragraph is used)
---
```

### License

Apache-2.0
