use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

use serde_json::Value;

use mdbook_rss_feed::{
    articles_from_book_json, build_feed_from_articles, DefaultBehavior, FeedOptions,
};

fn handle_mdbook_hooks(args: &[String]) -> bool {
    // Check for version
    if args.get(1).map(String::as_str) == Some("--version")
        || args.get(1).map(String::as_str) == Some("-V")
    {
        println!("mdbook-rss-feed {}", env!("CARGO_PKG_VERSION"));
        return true;
    }

    // Check for mdBook supports hook
    if args.get(1).map(String::as_str) == Some("supports") {
        std::process::exit(0);
    }

    false
}

#[allow(clippy::struct_excessive_bools)]
struct FeedConfig {
    src_dir: PathBuf,
    site_url: String,
    title: String,
    description: String,
    full_preview: bool,
    paginated: bool,
    max_items: usize,
    default_behavior: DefaultBehavior,
    #[cfg_attr(not(feature = "json-feed"), allow(dead_code))]
    json_enabled: bool,
    #[cfg_attr(not(feature = "atom"), allow(dead_code))]
    atom_enabled: bool,
    #[cfg_attr(not(feature = "atom"), allow(dead_code))]
    authors: Vec<String>,
    strict: bool,
    author_email: Option<String>,
}

impl FeedConfig {
    fn from_json(context: &Value) -> Self {
        let root = context
            .pointer("/root")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let default_behavior = context
            .pointer("/config/preprocessor/rss-feed/default-behavior")
            .and_then(Value::as_str)
            .and_then(|s| s.parse::<DefaultBehavior>().ok())
            .unwrap_or_default();

        Self {
            src_dir: PathBuf::from(root).join("src"),
            site_url: context
                .pointer("/config/output/html/site-url")
                .and_then(|v| v.as_str())
                .unwrap_or("https://example.com/")
                .to_string(),
            title: context
                .pointer("/config/book/title")
                .and_then(|v| v.as_str())
                .unwrap_or("My mdBook")
                .to_string(),
            description: context
                .pointer("/config/book/description")
                .and_then(|v| v.as_str())
                .unwrap_or("Description")
                .to_string(),
            full_preview: context
                .pointer("/config/preprocessor/rss-feed/full-preview")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            paginated: context
                .pointer("/config/preprocessor/rss-feed/paginated")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            max_items: context
                .pointer("/config/preprocessor/rss-feed/max-items")
                .and_then(Value::as_u64)
                .map_or(0, |n| usize::try_from(n).unwrap_or(usize::MAX)),
            default_behavior,
            json_enabled: context
                .pointer("/config/preprocessor/rss-feed/json-feed")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            atom_enabled: context
                .pointer("/config/preprocessor/rss-feed/atom")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            authors: context
                .pointer("/config/book/authors")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            strict: context
                .pointer("/config/preprocessor/rss-feed/strict")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            author_email: context
                .pointer("/config/preprocessor/rss-feed/author-email")
                .and_then(Value::as_str)
                .map(str::to_string),
        }
    }
    fn feed_options(&self) -> FeedOptions<'_> {
        FeedOptions {
            title: &self.title,
            site_url: &self.site_url,
            description: &self.description,
            full_preview: self.full_preview,
            max_items: self.max_items,
            paginated: self.paginated,
            default_behavior: self.default_behavior.clone(),
            strict: self.strict,
            author_email: self.author_email.clone(),
        }
    }
}

fn write_rss_pages(config: &FeedConfig, pages: &[mdbook_rss_feed::FeedPage]) -> io::Result<()> {
    for page in pages {
        let rss_path = config.src_dir.join(&page.filename);
        let rss_content = page.channel.to_string();
        eprintln!(
            "Writing RSS page {} ({} bytes)",
            rss_path.display(),
            rss_content.len()
        );
        fs::write(&rss_path, &rss_content)?;
    }
    Ok(())
}

#[cfg(feature = "json-feed")]
fn write_json_pages(
    config: &FeedConfig,
    pages: &[mdbook_rss_feed::FeedPage],
) -> Result<(), Box<dyn std::error::Error>> {
    use mdbook_rss_feed::rss_to_json_feed;

    if !config.json_enabled {
        return Ok(());
    }
    let base = config.site_url.trim_end_matches('/');
    let total = pages.len();
    for (page_idx, page) in pages.iter().enumerate() {
        let suffix = if page_idx == 0 {
            String::new()
        } else {
            (page_idx + 1).to_string()
        };
        let self_url = format!("{base}/feed{suffix}.json");

        let next_url = if page_idx + 1 < total {
            Some(format!("{base}/feed{}.json", page_idx + 2))
        } else {
            None
        };
        let json_feed = rss_to_json_feed(&page.channel, Some(&self_url), next_url.as_deref());
        let json_path = config.src_dir.join(if page_idx == 0 {
            "feed.json".to_string()
        } else {
            format!("feed{}.json", page_idx + 1)
        });
        fs::write(&json_path, serde_json::to_vec_pretty(&json_feed)?)?;
    }
    Ok(())
}

#[cfg(not(feature = "json-feed"))]
fn write_json_pages(
    _config: &FeedConfig,
    _pages: &[mdbook_rss_feed::FeedPage],
) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

#[cfg(feature = "atom")]
fn write_atom_pages(config: &FeedConfig, pages: &[mdbook_rss_feed::FeedPage]) -> io::Result<()> {
    use mdbook_rss_feed::rss_to_atom;

    if !config.atom_enabled {
        return Ok(());
    }
    let base = config.site_url.trim_end_matches('/');
    let total = pages.len();
    for (page_idx, page) in pages.iter().enumerate() {
        let suffix = if page_idx == 0 {
            String::new()
        } else {
            (page_idx + 1).to_string()
        };
        let self_url = format!("{base}/atom{suffix}.xml");
        // next points to older page, prev points to newer page.
        let next_url = if page_idx + 1 < total {
            Some(format!("{base}/atom{}.xml", page_idx + 2))
        } else {
            None
        };
        let prev_url = if page_idx > 0 {
            let prev_suffix = if page_idx == 1 {
                String::new()
            } else {
                page_idx.to_string()
            };
            Some(format!("{base}/atom{prev_suffix}.xml"))
        } else {
            None
        };
        let atom_feed = rss_to_atom(
            &page.channel,
            Some(&self_url),
            next_url.as_deref(),
            prev_url.as_deref(),
            &config.authors,
        );
        let atom_path = config.src_dir.join(if page_idx == 0 {
            "atom.xml".to_string()
        } else {
            format!("atom{}.xml", page_idx + 1)
        });
        fs::write(&atom_path, atom_feed.to_string())?;
    }
    Ok(())
}

#[cfg(not(feature = "atom"))]
fn write_atom_pages(_config: &FeedConfig, _pages: &[mdbook_rss_feed::FeedPage]) -> io::Result<()> {
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if handle_mdbook_hooks(&args) {
        return Ok(());
    }

    // 1. READ STDIN
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    // 2. PARSE JSON
    let input_array: Vec<Value> = serde_json::from_str(&input)?;
    let [context, book] = input_array.as_slice() else {
        return Err("expected mdBook to send a [context, book] pair on stdin".into());
    };

    // 3. EXTRACT CONFIG & BOOK
    let config = FeedConfig::from_json(context);

    // 4. COLLECT ARTICLES FROM THE BOOK JSON
    // This uses the already-processed book rather than walking the fs
    let articles = articles_from_book_json(book, config.strict);

    eprintln!(
        "mdbook-rss-feed: collected {} chapter(s) from book (default-behavior: {:?})",
        articles.len(),
        config.default_behavior,
    );

    // 5. BUILD FEED
    let result = build_feed_from_articles(articles, &config.feed_options());

    write_rss_pages(&config, &result.pages)?;
    write_json_pages(&config, &result.pages)?;
    write_atom_pages(&config, &result.pages)?;

    // 6. FINAL ECHO TO MDBOOK
    io::stderr().flush()?;
    println!("{}", serde_json::to_string(book)?);
    Ok(())
}
