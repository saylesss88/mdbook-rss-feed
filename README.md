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
renderers = ["html"]
```

## Frontmatter

```yaml
---
title: My Great Post
date: 2025-11-23
author: Jane Doe
description: Optional short description (otherwise first paragraph is used)
---
```

### Hiding frontmatter in the rendered HTML

I am currently working on a crate to implement this as `mdbook-yml-header`
wouldn't work for me.

**Adding a Description for RSS Preview**

The description in the frontmatter is what will be displayed as your file
preview.

```md
---
title: Debugging NixOS modules
date: 2025-11-22
author: saylesss88
description: This chapter covers debugging NixOS modules, focusing on tracing module options
and evaluating merges.
---
```

The preview should contain the above description.

### License

Apache-2.0
