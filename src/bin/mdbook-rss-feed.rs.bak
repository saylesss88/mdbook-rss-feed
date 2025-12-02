use mdbook_rss_feed::build_feed;
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

    eprintln!("Root: {root}");
    eprintln!("Site URL: {site_url}");
    eprintln!("Title: {feed_title}");
    eprintln!("Full preview: {full_preview}");
    eprintln!("Paginated: {paginated}");
    eprintln!("Max items: {max_items}");

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
    for page in result.pages {
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

    // Flush logs to ensure they appear before stdout
    io::stderr().flush().unwrap();

    eprintln!("RSS feed(s) written to src/");

    // ECHO BACK THE SECOND ELEMENT — THE ACTUAL BOOK
    println!("{}", serde_json::to_string(book).unwrap());
}
