use atom_syndication::Link as AtomLink;
use mdbook_rss_feed::{build_feed, rss_to_atom, rss_to_json_feed};
use serde_json::Value;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

fn main() {
    eprintln!(
        "Running mdbook-rss-feed binary at: {}",
        std::env::current_exe().unwrap().display()
    );

    let args: Vec<String> = std::env::args().collect();
    // Handle version
    if args.get(1).map(|s| s.as_str()) == Some("--version")
        || args.get(1).map(|s| s.as_str()) == Some("-V")
    {
        println!("mdbook-rss-feed {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    // Required by mdBook
    if args.get(1).map(|s| s.as_str()) == Some("supports") {
        println!("true");
        return;
    }

    let mut input = String::new();
    match std::io::stdin().read_to_string(&mut input) {
        Ok(0) => {
            eprintln!("ERROR: Read 0 bytes from stdin – empty input?");
            eprintln!(
                "Current dir: {}",
                std::env::current_dir().unwrap_or_default().display()
            );
            eprintln!("Args: {:?}", std::env::args().collect::<Vec<_>>());
            std::process::exit(1);
        }
        Ok(len) => eprintln!("Read {} bytes from stdin", len),
        Err(e) => {
            eprintln!("ERROR: Failed to read stdin: {}", e);
            std::process::exit(1);
        }
    }

    if input.trim().is_empty() {
        eprintln!("ERROR: Stdin empty after read");
        std::process::exit(1);
    }

    eprintln!(
        "Stdin preview (first 100 chars): {}",
        &input[..input.len().min(100)]
    );

    let input_array: Vec<Value> = serde_json::from_str(&input).expect("Invalid JSON from mdBook");

    if input_array.len() < 2 {
        eprintln!("ERROR: mdBook sent less than 2 JSON objects");
        std::process::exit(1);
    }

    let context = &input_array[0];
    let book = &input_array[1]; // This is the real book

    let root = context
        .pointer("/root")
        .and_then(|v| v.as_str())
        .unwrap_or(".");

    let src_dir = PathBuf::from(root).join("src");

    // Robust Site URL Extraction
    let site_url = context
        .pointer("/config/output/html/site-url")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("https://example.com/")
        .trim_end_matches('/')
        .to_string();

    let feed_title = context
        .pointer("/config/book/title")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("My mdBook");

    let feed_description = context
        .pointer("/config/book/description")
        .and_then(|v| v.as_str())
        .unwrap_or("An mdBook-generated site");

    // Preview mode flag
    let full_preview = context
        .pointer("/config/preprocessor/rss-feed/full-preview")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Pagination flags
    let paginated = context
        .pointer("/config/preprocessor/rss-feed/paginated")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let max_items = context
        .pointer("/config/preprocessor/rss-feed/max-items")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(0); // 0 = unlimited (single feed)

    // JSON Feed flag
    let json_feed_enabled = context
        .pointer("/config/preprocessor/rss-feed/json-feed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Atom flag
    let atom_enabled = context
        .pointer("/config/preprocessor/rss-feed/atom")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    eprintln!("Root: {root}");
    eprintln!("Site URL: {site_url}");
    eprintln!("Title: {feed_title}");
    eprintln!("Full preview: {full_preview}");
    eprintln!("Paginated: {paginated}");
    eprintln!("Max items: {max_items}");
    eprintln!("Atom enabled: {atom_enabled}");
    eprintln!("JSON Feed enabled: {json_feed_enabled}");

    let result = build_feed(
        &src_dir,
        feed_title,
        &site_url,
        feed_description,
        full_preview,
        max_items,
        paginated,
    )
    .expect("Failed to generate RSS feed");

    // Write all pages
    for page in &result.pages {
        let rss_path = src_dir.join(&page.filename);

        let rss_content = page.channel.to_string();
        let rss_bytes = rss_content.as_bytes();

        eprintln!(
            "Writing RSS page {} ({} bytes)",
            rss_path.display(),
            rss_bytes.len()
        );
        eprintln!("Src dir exists: {}", src_dir.exists());
        eprintln!(
            "Src dir writable: {}",
            src_dir
                .metadata()
                .map(|m| !m.permissions().readonly())
                .unwrap_or(false)
        );

        match fs::write(&rss_path, rss_bytes) {
            Ok(_) => {
                let written_metadata = rss_path.metadata();
                match written_metadata {
                    Ok(m) => {
                        eprintln!(
                            "Write succeeded for {}! Written file size: {} bytes",
                            rss_path.display(),
                            m.len()
                        );
                        if m.len() == 0 {
                            eprintln!("ERROR: Wrote 0 bytes—possible I/O truncation");
                        }
                    }
                    Err(e) => eprintln!(
                        "ERROR: Failed to get metadata after write for {}: {}",
                        rss_path.display(),
                        e
                    ),
                }
            }
            Err(e) => {
                eprintln!("ERROR: fs::write failed for {}: {}", rss_path.display(), e);
                std::process::exit(1);
            }
        }
    }

    // Optionally write JSON Feed from the first RSS page

    // Optionally write JSON Feed for all pages
    if json_feed_enabled {
        let total_pages = result.pages.len();

        for (page_idx, page) in result.pages.iter().enumerate() {
            // suffix: "" for page 0, "2", "3", ...
            let suffix = if page_idx == 0 {
                String::new()
            } else {
                (page_idx + 1).to_string()
            };

            let self_url = format!("{}/feed{}.json", site_url, suffix);

            let next_url = if page_idx + 1 < total_pages {
                let next_suffix = (page_idx + 2).to_string(); // page 2, 3, ...
                Some(format!("{}/feed{}.json", site_url, next_suffix))
            } else {
                None
            };

            let json_feed = rss_to_json_feed(&page.channel, Some(&self_url), next_url.as_deref());

            let json_path = src_dir.join(if page_idx == 0 {
                "feed.json".to_string()
            } else {
                format!("feed{}.json", page_idx + 1)
            });

            let json_bytes =
                serde_json::to_vec_pretty(&json_feed).expect("Failed to serialize JSON Feed");

            eprintln!(
                "Writing JSON Feed {} ({} bytes)",
                json_path.display(),
                json_bytes.len()
            );

            if let Err(e) = fs::write(&json_path, &json_bytes) {
                eprintln!(
                    "ERROR: fs::write failed for JSON Feed {}: {}",
                    json_path.display(),
                    e
                );
                std::process::exit(1);
            }
        }
    }
    // Optionally write Atom feed(s) from all RSS pages
    if atom_enabled {
        let total_pages = result.pages.len();

        for (page_idx, page) in result.pages.iter().enumerate() {
            let mut atom_feed = rss_to_atom(&page.channel);

            let suffix = if page_idx == 0 {
                String::new()
            } else {
                (page_idx + 1).to_string()
            };

            let self_url = format!("{}/atom{}.xml", site_url, suffix);

            let next_url = if page_idx + 1 < total_pages {
                let next_suffix = (page_idx + 2).to_string();
                Some(format!("{}/atom{}.xml", site_url, next_suffix))
            } else {
                None
            };

            let prev_url = if page_idx > 0 {
                let prev_suffix = if page_idx - 1 == 0 {
                    String::new()
                } else {
                    page_idx.to_string()
                };
                Some(format!("{}/atom{}.xml", site_url, prev_suffix))
            } else {
                None
            };

            let mut links = Vec::new();

            // rel="self"
            links.push(AtomLink {
                href: self_url.clone(),
                rel: "self".to_string(),
                ..Default::default()
            });

            // rel="next"
            if let Some(href) = next_url {
                links.push(AtomLink {
                    href,
                    rel: "next".to_string(),
                    ..Default::default()
                });
            }

            // rel="prev"
            if let Some(href) = prev_url {
                links.push(AtomLink {
                    href,
                    rel: "prev".to_string(),
                    ..Default::default()
                });
            }

            atom_feed.set_links(links);
            atom_feed.set_id(self_url.clone());

            let atom_xml = atom_feed.to_string();
            let atom_path = src_dir.join(if page_idx == 0 {
                "atom.xml".to_string()
            } else {
                format!("atom{}.xml", page_idx + 1)
            });
            let atom_bytes = atom_xml.as_bytes();

            eprintln!(
                "Writing Atom feed {} ({} bytes)",
                atom_path.display(),
                atom_bytes.len()
            );

            if let Err(e) = fs::write(&atom_path, atom_bytes) {
                eprintln!(
                    "ERROR: fs::write failed for Atom feed {}: {}",
                    atom_path.display(),
                    e
                );
                std::process::exit(1);
            }
        }
    }
    // Flush logs to ensure they appear before stdout
    io::stderr().flush().unwrap();

    eprintln!("RSS feed(s) written to src/");
    if atom_enabled {
        eprintln!("Atom feed written to src/atom.xml");
    }

    // ECHO BACK THE SECOND ELEMENT — THE ACTUAL BOOK
    println!("{}", serde_json::to_string(book).unwrap());
}
