# mdbook-rss-feed

An mdBook preprocessor that generates a beautiful RSS 2.0 feed (and optional
Atom) for your book, with HTML previews for each chapter.

Perfect for blogs, documentation sites, or any mdBook that you want to publish.

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

`renderers = ["html"]` ensures the preprocessor only runs for HTML builds.

- If you omit `title`, mdBook will use `My mdbook`.

- If you omit `site-url`, the default is `https://example.com`. Set this to the
  public base URL of your site.

- With the example above, the feed would be at:
  `https://your-user.github.io/rss.xml`.

## Frontmatter

Frontmatter is optional. Without it, entries only include the chapter title,
book name, and date/time.(varies by RSS reader)

With frontmatter, you can customize those fields and add author and description:

```yaml
title: Debugging NixOS modules
date: 2025-11-22
author: saylesss88
description: This chapter covers debugging NixOS modules, focusing on tracing module
options and evaluating merges.
```

### How feed preview is generated

The preview is generated from the rendered HTML of the chapter. The crate finds
`<p>…</p>` blocks and takes the first 2–3 paragraphs, up to 800 characters.

If a chapter starts with non-paragraph content (lists, details blocks, custom
markup), the preview starts at the first real paragraph.

To override this, set `description` in the YAML frontmatter; that text is used
when the body is empty or very short.

- Default: preview comes from the first few body paragraphs.

- Fallback: when the body is empty/very short, preview comes from `description`.

If you never want to use the fallback, just omit `description`; the preview will
always come from the body.

### Hiding frontmatter in the rendered HTML

mdBook does not parse or strip YAML frontmatter, so the raw block (e.g. any YAML
keys like `title:`, `date:`, etc.) appears in the HTML.

To avoid this, you can use:

[mdbook-frontmatter-strip](https://crates.io/crates/mdbook-frontmatter-strip)

## RSS Button for mdbook header

<details>
<summary> ✔️ Click to expand RSS Button Example </summary>

Your `book.toml` can accept additional css and js:

```book.toml
additional-css = [ "theme/rss-button.css" ]
additional-js = [ "theme/rss-buttons.js" ]
```

In your books `theme/` directory, place these two files:

1. `theme/rss-button.css`

```css
/* Simple RSS header button */
.rss-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  margin-left: 8px;
  color: var(--sidebar-fg, #333);
  opacity: 0.7;
  transition: opacity 0.2s;
}

.rss-btn:hover {
  opacity: 1;
  color: var(--sidebar-fg, #333);
}

/* Optional: orange hover like classic RSS */
.rss-btn:hover svg {
  stroke: #f26522;
}
```

2. `theme/rss-button.js`

```js
document.addEventListener("DOMContentLoaded", () => {
  const menuBar =
    document.querySelector(".menu-bar .right-buttons") ||
    document.querySelector(".menu-bar");
  if (!menuBar) return;

  const rssLink = document.createElement("a");
  rssLink.href = "https://your-user.github.io/rss.xml"; // set to your feed URL
  rssLink.target = "_blank";
  rssLink.rel = "noopener";
  rssLink.title = "Subscribe to RSS feed";
  rssLink.className = "rss-btn";

  rssLink.innerHTML = `
    <svg xmlns="http://www.w3.org/2000/svg"
         width="16" height="16" viewBox="0 0 24 24"
         fill="none" stroke="currentColor" stroke-width="2"
         stroke-linecap="round" stroke-linejoin="round"
         style="margin-bottom:-3px">
      <circle cx="6" cy="18" r="3"></circle>
      <path d="M6 6c6.627 0 12 5.373 12 12"></path>
      <path d="M6 12c3.314 0 6 2.686 6 6"></path>
    </svg>
  `;

  const printButton = menuBar.querySelector(".print-btn, #print-button");
  if (printButton && printButton.parentNode === menuBar) {
    printButton.before(rssLink);
  } else {
    menuBar.appendChild(rssLink);
  }
});
```

Now you should have a small logo pinned to the top right of your book that leads
to `https://your-site/rss.xml`

</details>

### License

[Apache License 2.0](https://github.com/saylesss88/mdbook-rss-feed/blob/main/LICENSE)
