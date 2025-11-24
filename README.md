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

mdBook itself does not understand YAML frontmatter and will render it as plain
text at the top of each chapter. If you use frontmatter, you will usually want
to pair this preprocessor with a “frontmatter stripper” so the YAML block does
not appear on your HTML pages.

One option is [`mdbook-yml-header`], which removes `--- … ---` headers before
rendering:

```bash
cargo install mdbook-yml-header
```

`book.toml`:

```toml
[preprocessor.yml-header]
```

**Adding a Description for RSS Preview**

The first header will be picked up for the preview, for example:

```md
---
title: Debugging NixOS modules
date: 2025-11-22
author: saylesss88
description: Chapter 9
---

# Chapter 9

This chapter covers debugging NixOS modules, focusing on tracing module options
and evaluating merges.
```

The preview should contain the above description.

### License

Apache-2.0
