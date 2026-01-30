use mdbook_rss_feed::{build_feed, rss_to_atom, rss_to_json_feed};
use serde_json::Value;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

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
        println!("true");
        return true;
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
    json_enabled: bool,
    atom_enabled: bool,
}

impl FeedConfig {
    fn from_json(context: &Value) -> Self {
        let root = context
            .pointer("/root")
            .and_then(|v| v.as_str())
            .unwrap_or(".");
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
            json_enabled: context
                .pointer("/config/preprocessor/rss-feed/json-feed")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            atom_enabled: context
                .pointer("/config/preprocessor/rss-feed/atom")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if handle_mdbook_hooks(&args) {
        return;
    }

    // 1. READ STDIN
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("Failed to read stdin");

    // 2. PARSE JSON
    let input_array: Vec<Value> = serde_json::from_str(&input).expect("Invalid JSON");
    if input_array.len() < 2 {
        std::process::exit(1);
    }

    // 3. EXTRACT CONFIG & BOOK
    let config = FeedConfig::from_json(&input_array[0]);
    let book = &input_array[1];

    // 4. BUILD FEED
    let result = build_feed(
        &config.src_dir,
        &config.title,
        &config.site_url,
        &config.description,
        config.full_preview,
        config.max_items,
        config.paginated,
    )
    .expect("Failed to generate RSS feed");

    // 5. WRITE RSS PAGES
    for page in &result.pages {
        let rss_path = config.src_dir.join(&page.filename);
        let rss_content = page.channel.to_string();

        eprintln!(
            "Writing RSS page {} ({} bytes)",
            rss_path.display(),
            rss_content.len()
        );

        fs::write(&rss_path, &rss_content).expect("Failed to write RSS file");
    }

    // 6. WRITE JSON FEED (Optional)
    if config.json_enabled {
        for (page_idx, page) in result.pages.iter().enumerate() {
            let suffix = if page_idx == 0 {
                String::new()
            } else {
                (page_idx + 1).to_string()
            };
            let self_url = format!("{}/feed{}.json", config.site_url, suffix);

            let json_feed = rss_to_json_feed(&page.channel, Some(&self_url), None);
            let json_path = config.src_dir.join(if page_idx == 0 {
                "feed.json".into()
            } else {
                format!("feed{}.json", page_idx + 1)
            });

            fs::write(&json_path, serde_json::to_vec_pretty(&json_feed).unwrap())
                .expect("JSON write failed");
        }
    }

    // 7. WRITE ATOM FEED (Optional)
    if config.atom_enabled {
        for (page_idx, page) in result.pages.iter().enumerate() {
            let atom_feed = rss_to_atom(&page.channel);
            let atom_path = config.src_dir.join(if page_idx == 0 {
                "atom.xml".into()
            } else {
                format!("atom{}.xml", page_idx + 1)
            });

            fs::write(&atom_path, atom_feed.to_string()).expect("Atom write failed");
        }
    }

    // 8. FINAL ECHO TO MDBOOK
    let _ = io::stderr().flush();
    println!("{}", serde_json::to_string(book).unwrap());
}
