# mdbook-rss-feed

An mdBook preprocessor that generates RSS, Atom, and JSON feeds with rich HTML
previews, optional full-content entries, and pagination support. Drop it in
`book.toml` and it just works, perfect for blogs, docs sites, or any mdBook
you want to publish a feed for.

## Features

- Hybrid HTML preview built from the first paragraphs of each chapter, with
  fallback to a frontmatter `description` when the body is short or missing
- Optional full-content entries instead of previews
- Optional pagination (`rss2.xml`, `rss3.xml`, …) to keep the main feed small
- Optional Atom (`atom.xml`) and JSON Feed (`feed.json`) output alongside RSS
- Reads `date:` from frontmatter (RFC3339 or `YYYY-MM-DD`), falling back to
  file modification time
- Works with or without frontmatter; zero-config by default

## Installation

Atom and JSON Feed output are gated behind cargo features, since they pull in
extra dependencies most users won't need. Pick what you want at install time:

```bash
# RSS only (default)
cargo install mdbook-rss-feed

# RSS + Atom + JSON Feed
cargo install mdbook-rss-feed --features atom,json-feed
```

| Feature | Enables |
|---|---|
| `atom` | `atom.xml` output |
| `json-feed` | `feed.json` output |

If you set `atom = true` or `json-feed = true` in `book.toml` without
installing the matching feature, the preprocessor prints a warning to stderr
and skips that output rather than failing the build.

Version check:

```bash
mdbook-rss-feed --version
```

## Usage

```toml
[book]
title = "your-title"
author = "your-author"
src = "src"

[preprocessor.rss-feed]
renderers = ["html"]
# full-preview = true   # use the whole chapter as the preview, not an excerpt
# atom = true           # also write atom.xml (needs the `atom` feature)
# json-feed = true      # also write feed.json (needs the `json-feed` feature)
# paginated = true       # split into rss.xml, rss2.xml, ... once max-items is exceeded
# max-items = 4

[output.html]
site-url = "https://your-user.github.io/"
```

`renderers = ["html"]` ensures the preprocessor only runs for HTML builds.

- Omitting `title` falls back to `My mdbook`; omitting `site-url` falls back
  to `https://example.com`. Set `site-url` to your site's real public base URL.
- With the config above, the feed is published at
  `https://your-user.github.io/rss.xml`.
- `full-preview = true` lets readers read the whole entry in their feed
  reader without visiting the site — better privacy, fewer tracked page
  views.

### Pagination

<details>
<summary>Click to expand</summary>

Enable with `paginated = true` and `max-items = N` in `[preprocessor.rss-feed]`.

- Chapters are sorted by frontmatter `date` (newest first), falling back to
  file modification time.
- `rss.xml` holds the newest `N` items; older items spill into `rss2.xml`,
  `rss3.xml`, etc.
- If `atom`/`json-feed` are enabled, their paginated pages mirror the RSS
  pages (`atom2.xml`, `feed2.json`, …). Atom pages include `rel="next"`/
  `rel="prev"` links; JSON Feed pages include `next_url`, per the JSON Feed
  1.1 spec.

Pagination relies on accurate `date:` frontmatter — without it, ordering
falls back to file timestamps, which may not match publish order.

To turn pagination back off: set `paginated = false` and `max-items = 0`,
delete any `rss2.xml`, `atom2.xml`, `feed2.json` (etc.) files from `src/`,
and run `mdbook clean` before rebuilding.

</details>

## Frontmatter

Frontmatter is optional. Without it, entries fall back to the chapter title
and a date derived from the file's mtime. With it, the same block drives all
three feed formats:

```yaml
title: Debugging NixOS modules
date: 2025-11-22
author: saylesss88
description: This chapter covers debugging NixOS modules, focusing on tracing
  module options and evaluating merges.
```

- Dates must be RFC3339 or `YYYY-MM-DD` to sort correctly; add them to every
  chapter for reliable chronological order.
- A loader like
  [mdbook-content-loader](https://crates.io/crates/mdbook-content-loader) can
  enforce typed, validated frontmatter so dates are always present — this
  makes pagination ordering more reliable, but isn't required.

### How the preview is built

By default, the preview comes from the first 2–3 `<p>` blocks of the
rendered chapter (up to ~800 characters), skipping any leading
non-paragraph content like lists or details blocks. Set `description` in
frontmatter to override this, that text is used whenever the chapter body
is empty or very short. Omit `description` if you always want the preview
pulled from the body.

## Syndication formats

- **RSS 2.0** (`rss.xml`): widest reader support; good default.
- **Atom 1.0** (`atom.xml`, needs the `atom` feature): stricter spec, less
  ambiguity than RSS.
- **JSON Feed 1.1** (`feed.json`, needs the `json-feed` feature): plain
  JSON, easy to consume from custom tooling without an XML parser.

<details>
<summary>RSS example</summary>

```xml
<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0">
  <channel>
    <title>privacy-book</title>
    <link>https://mako088.github.io/</link>
    <description>An mdBook-generated site</description>
    <generator>mdbook-rss-feed 1.4.1</generator>
    <item>
      <title>Encrypted DNS on Arch</title>
      <link>https://mako088.github.io/arch/enc_dns.html</link>
      <description><![CDATA[<p>NOTE: There are many other ways for someone
      monitoring your traffic to see what domain you looked up via DNS...</p>]]></description>
      <guid>https://mako088.github.io/arch/enc_dns.html</guid>
      <pubDate>Fri, 28 Nov 2025 00:00:00 +0000</pubDate>
    </item>
  </channel>
</rss>
```
_Truncated for brevity._
</details>

<details>
<summary>Atom example</summary>

```xml
<?xml version="1.0"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>privacy-book</title>
  <id>https://mako088.github.io/atom.xml</id>
  <link href="https://mako088.github.io/atom.xml" rel="self"/>
  <subtitle>An mdBook-generated site</subtitle>
  <entry>
    <title>Encrypted DNS on Arch</title>
    <id>https://mako088.github.io/arch/enc_dns.html</id>
    <updated>2025-11-28T00:00:00+00:00</updated>
    <link href="https://mako088.github.io/arch/enc_dns.html" rel="alternate"/>
    <content type="html">&lt;p&gt;NOTE: There are many other ways...&lt;/p&gt;</content>
  </entry>
</feed>
```
_Truncated for brevity._
</details>

<details>
<summary>JSON Feed example</summary>

```json
{
  "version": "https://jsonfeed.org/version/1.1",
  "title": "privacy-book",
  "home_page_url": "https://mako088.github.io/",
  "feed_url": "https://mako088.github.io/feed.json",
  "description": "An mdBook-generated site",
  "items": [
    {
      "id": "https://mako088.github.io/arch/enc_dns.html",
      "url": "https://mako088.github.io/arch/enc_dns.html",
      "title": "Encrypted DNS on Arch",
      "content_html": "<p>NOTE: There are many other ways...</p>",
      "date_published": "2025-11-28T00:00:00+00:00",
      "author": { "name": "saylesss88" }
    }
  ]
}
```
_Truncated for brevity._
</details>

### Using injected snippets as previews

<details>
<summary>Click to expand</summary>

If you use preprocessors like
[mdbook-content-loader](https://crates.io/crates/mdbook-content-loader) or
[mdbook-content-collections](https://crates.io/crates/mdbook-content-collections)
to inject intro snippets into chapters, those snippets are ordinary Markdown
by the time `mdbook-rss-feed` sees them. Since the preview is built from the
first real `<p>` blocks in the rendered chapter, an injected intro paragraph
becomes the feed preview automatically — no extra config needed.

Combined with typed frontmatter from a content loader, you get consistent
ordering between your book's index and the RSS feed, plus cleaner previews
when snippets are well-structured.

</details>

### Hiding frontmatter in rendered HTML

mdBook doesn't strip YAML frontmatter on its own, so the raw block can leak
into the rendered HTML. Use
[mdbook-frontmatter-strip](https://crates.io/crates/mdbook-frontmatter-strip)
to remove it.

## RSS button for the mdBook header

<details>
<summary>Click to expand</summary>

Add to `book.toml`:

```toml
additional-css = ["theme/rss-button.css"]
additional-js = ["theme/rss-button.js"]
```

`theme/rss-button.css`:

```css
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
.rss-btn:hover svg {
  stroke: #f26522;
}
```

`theme/rss-button.js`:

> [!NOTE]
> Make sure to change the line `rssLink.href = "https://your-user.github.io/rss.xml"; // set to your feed URL` to your page.

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
    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"
         viewBox="0 0 24 24" fill="none" stroke="currentColor"
         stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
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

This pins a small RSS icon to the top right of the book, linking to your
feed.

</details>

## License

[Apache License 2.0](https://github.com/saylesss88/mdbook-rss-feed/blob/main/LICENSE)
