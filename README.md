# mdbook-rss-feed

An mdBook preprocessor that generates RSS, Atom, and JSON feeds with rich HTML
previews, optional full-content entries, and pagination support.

Perfect for blogs, documentation sites, or any mdBook that you want to publish.

---

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

- Optional flag to generate a full-preview

- Optional paginated feeds: keep `rss.xml` small and fast for readers while
  still exposing older entries via `rss2.xml`, `rss3.xml`, etc., so archives
  stay accessible without bloating the main feed.

- Optionally output an Atom 1.0 feed is written to `atom.xml` alongside
  `rss.xml` so Atom-capable readers can subscribe to either format.

- Optionally output a json feed file (`feed.json`)

---

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

---

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
# Generate the full chapter as the preview
# full-preview = true
# Also generate an atom.xml
# atom = true
# Also generate a feed.json
# json-feed = true


# Enable pagination (optionally for RSS, Atom, and JSON feeds)
# full-preview = false
# paginated = true # enable pagination
# max-items = 4    # max items per page (0 = unlimited / single feed)

[output.html]
site-url = "https://your-user.github.io/"
```

`renderers = ["html"]` ensures the preprocessor only runs for HTML builds.

- If you omit `title`, mdBook will use `My mdbook`.

- If you omit `site-url`, the default is `https://example.com`. Set this to the
  public base URL of your site.

- With the example above, the feed would be at:
  `https://your-user.github.io/rss.xml`.
  - With pagination, if the `max-items` is less than the total items, the feeds
    will be split into the necessary number of feeds to meet that `max-items`
    requirement.
  - The paginated feeds in the above example would be located at
    `https://your-user.github.io/rss.xml`,
    `https://your-user.github.io/rss2.xml`,
    `https://your-user.github.io/rss3.xml`, etc.

- Adding `full-preview = true` lets readers view the entire content directly in
  their feed reader, which improves privacy and reduces tracking by avoiding
  visits to the website itself.

- In the above example, the atom feed would be at
  `https://your-user.github.io/atom.xml`.

---

### Pagination

<details>
<summary> ✔️ Click to Expand Pagination Overview </summary>

Enable with `paginated = true` and `max-items = N` (e.g., 20) in
`[preprocessor.rss-feed]`.

- Collects all .md chapters, sorts by `date` in frontmatter (newest first; falls
  back to file modification time).

- RSS:
  - `rss.xml` gets the N newest items only.
  - Older items go to `rss2.xml`, `rss3.xml`, etc. (e.g., with `max-items = 4`,
    `rss.xml` has top 4 by date).
  - Keeps the main feed small/fast for readers; full history available in extra
    files.

- Atom (when `atom = true`):
  - `atom.xml`, `atom2.xml`, `atom3.xml`, mirror the RSS pages.

  - Each Atom page includes `rel="self"` plus `rel="next"`/`rel="prev"` links so
    clients can follow older or newer entries.​

- JSON Feed (when `json-feed = true`):
  - `feed.json`, `feed2.json`, `feed3.json`, mirror the RSS pages.

  - Each JSON feed page includes a `next_url` pointing to the next page of older
    items, as defined in JSON Feed 1.1.

To paginate correctly, ensure most chapters have `date:` in frontmatter (RFC3339
like `2025-12-02T12:00:00Z` or simple `2025-12-02`). Without dates, sorting uses
file timestamps, which may not reflect publish order.

When switching back to an un-paginated feed, use `paginated = false`, with
`max-items = 0`, delete any `rss2.xml`, `rss3.xml`, `atom2.xml`, `atom3.xml`,
`feed2.json`, etc. files in your `src/` directory, and run `mdbook clean` before
rebuilding.

</details>

---

## Frontmatter

Frontmatter is optional. Without it, entries only include the chapter title,
book name, and date/time. (varies by RSS reader)

With frontmatter, you can customize those fields and add author and description,
the same frontmatter works for all syndications:

```yaml
title: Debugging NixOS modules
date: 2025-11-22
author: saylesss88
description: This chapter covers debugging NixOS modules, focusing on tracing module
options and evaluating merges.
```

- Dates must be parsable (YYYY-MM-DD or RFC3339) to sort accurately.

- Add dates to all chapters for chronological order; without them, recent file
  saves win.

- Using a loader like `mdbook-content-loader` to validate and normalize
  frontmatter ensures dates are always present and correctly formatted, which
  makes pagination and chronological ordering in the RSS feed more reliable.
  (Optional)

---

### How feed preview is generated (default)

The preview is generated from the rendered HTML of the chapter. The crate finds
`<p>…</p>` blocks and takes the first 2–3 paragraphs, up to 800 characters.

If a chapter starts with non-paragraph content (lists, details blocks, custom
markup), the preview starts at the first real paragraph.

To override this, set `description` in the YAML frontmatter; that text is used
when the body is empty or very short.

- **Default**: preview comes from the first few body paragraphs.

- **Fallback**: when the body is empty/very short, preview comes from
  `description`.

If you never want to use the fallback, just omit `description`; the preview will
always come from the body.

---

## Syndication

This crate exposes three feed formats so you can use whatever works best for
your reader or tooling:

- **RSS 2.0** (`rss.xml`): The most widely supported format. Good default choice
  for maximum compatibility with older and newer feed readers alike.

<details>
<summary> ✔️ RSS example (rss.xml)</summary>

```xml
<?xml version="1.0" encoding="utf-8"?><rss version="2.0"><channel><title>privacy-book</title><link>https://mako088.github.io/</link><description>An mdBook-generated site</description><generator>mdbook-rss-feed 1.0.0</generator><item><title>index</title><link>https://mako088.github.io/index.html</link><description><![CDATA[]]></description><guid>https://mako088.github.io/index.html</guid><pubDate>Wed, 3 Dec 2025 00:05:27 +0000</pubDate></item><item><title>Encrypted DNS on Arch</title><link>https://mako088.github.io/arch/enc_dns.html</link><description><![CDATA[<p>❗ NOTE: There are many other ways for someone monitoring your traffic to see
what domain you looked up via DNS that it’s effectiveness is questionable
without also using Tor or a VPN. Encrypted DNS will not help you hide any of
your browsing activity.</p><pre><code class="language-bash">sudo pacman -S dnscrypt-proxy
</code></pre>
<blockquote>
```

_Truncated example for brevity_

</details>

- **Atom 1.0** (`atom.xml`): A better-specified XML format with stricter
  semantics and less ambiguity than RSS. Nice choice if you care about standards
  correctness and richer metadata but still want XML.

<details>
<summary> ✔️ Atom example (atom.xml)</summary>

```xml
<?xml version="1.0"?>
<feed xmlns="http://www.w3.org/2005/Atom"><title>privacy-book</title><id>https://mako088.github.io/atom.xml</id><updated>1970-01-01T00:00:00+00:00</updated><link href="https://mako088.github.io/atom.xml" rel="self"/><subtitle>An mdBook-generated site</subtitle><entry><title>index</title><id>https://mako088.github.io/index.html</id><updated>2025-12-03T00:05:27+00:00</updated><link href="https://mako088.github.io/index.html" rel="alternate"/><content type="html"></content></entry><entry><title>Encrypted DNS on Arch</title><id>https://mako088.github.io/arch/enc_dns.html</id><updated>2025-11-28T00:00:00+00:00</updated><link href="https://mako088.github.io/arch/enc_dns.html" rel="alternate"/><content type="html">&lt;p&gt;❗ NOTE: There are many other ways for someone monitoring your traffic to see
what domain you looked up via DNS that it’s effectiveness is questionable
without also using Tor or a VPN. Encrypted DNS will not help you hide any of
your browsing activity.&lt;/p&gt;&lt;pre&gt;&lt;code class=&quot;language-bash&quot;&gt;sudo pacman -S dnscrypt-proxy
&lt;/code&gt;&lt;/pre&gt;
&lt;blockquote&gt;
&lt;p&gt;NOTE: udp is required for dnscrypt protocol, keep this in mind when
```

_Truncated example for brevity_

</details>

- **JSON Feed 1.1** (`feed.json`): A feed format based on JSON instead of XML,
  designed to be easy to consume from modern applications, thus being noticeably
  faster. This is often the easiest to parse if you’re writing custom tools,
  because you can treat it as ordinary JSON rather than dealing with XML
  parsing.

<details>
<summary>✔️ JSON Feed example (feed.json)</summary>

```json
{
  "version": "https://jsonfeed.org/version/1.1",
  "title": "privacy-book",
  "home_page_url": "https://mako088.github.io/",
  "feed_url": "https://mako088.github.io/feed.json",
  "description": "An mdBook-generated site",
  "items": [
    {
      "id": "https://mako088.github.io/index.html",
      "url": "https://mako088.github.io/index.html",
      "title": "index",
      "content_html": "",
      "date_published": "2025-12-03T00:05:27+00:00"
    },
    {
      "id": "https://mako088.github.io/arch/enc_dns.html",
      "url": "https://mako088.github.io/arch/enc_dns.html",
      "title": "Encrypted DNS on Arch",
      "content_html": "<p>❗ NOTE: There are many other ways for someone monitoring your traffic to see\nwhat domain you looked up via DNS that it’s effectiveness is questionable\nwithout also using Tor or a VPN. Encrypted DNS will not help you hide any of\nyour browsing activity.</p><pre><code class=\"language-bash\">sudo pacman -S dnscrypt-proxy\n</code></pre>\n<blockquote>\n<p>NOTE: udp is required for dnscrypt protocol, keep this in mind when\nconfiguring your servers if your output chain is a default drop.</p><p><a href=\"https://wiki.archlinux.org/title/Dnscrypt-proxy\">Arch Wiki dnscrypt-proxy</a></p>",
      "date_published": "2025-11-28T00:00:00+00:00",
      "author": {
        "name": "saylesss88"
      }
    },
```

_Truncated example shown for brevity_

</details>

---

### Using injected snippets as previews

<details>
<summary> ✔️ Click to Expand snippet preview Overview </summary>

- If you use preprocessors like
  [mdbook-content-loader](https://crates.io/crates/mdbook-content-loader), and
  [mdbook-content-collections](https://crates.io/crates/mdbook-content-collections)
  to inject intro snippets into your chapters, those snippets are treated just
  like normal Markdown.

- Because `mdbook-rss-feed` renders the final chapter Markdown and then finds
  the first real `<p>…</p>` blocks, injected paragraphs at the top of the
  chapter will become the preview text automatically.

- This allows you to maintain small, reusable intro snippets (or per-section
  summaries) and have them appear in both the book and the RSS feed without any
  extra configuration.

- `mdbook-content-loader` can enforce typed frontmatter and custom sorting (for
  example, by a typed date field or other metadata).​

`mdbook-rss-feed` reads the resulting Markdown files and sorts feed items by the
date in frontmatter (falling back to file modification time when missing).

- Combined, you get:
  - Strongly-typed frontmatter (less chance of bad dates or missing fields).

  - Consistent ordering between your book’s index pages and the RSS feed.

  - Cleaner previews when your loader injects well-structured intro snippets at
    the top of each chapter.

</details>

---

### Hiding frontmatter in the rendered HTML

mdBook does not parse or strip YAML frontmatter, so the raw block (e.g. any YAML
keys like `title:`, `date:`, etc.) appears in the HTML.

To avoid this, you can use:

[mdbook-frontmatter-strip](https://crates.io/crates/mdbook-frontmatter-strip)

---

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

---

### License

[Apache License 2.0](https://github.com/saylesss88/mdbook-rss-feed/blob/main/LICENSE)
