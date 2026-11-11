use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

use serde_json::Value;

use mdbook_rss_feed::{
    DefaultBehavior, FeedOptions, articles_from_book_json, build_feed_from_articles,
};

const DEFAULT_RSS_FILENAME: &str = "rss.xml";
const DEFAULT_ATOM_FILENAME: &str = "atom.xml";
const DEFAULT_JSON_FEED_FILENAME: &str = "feed.json";

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
    output_filenames: OutputFilenames,
    default_behavior: DefaultBehavior,
    #[cfg_attr(not(feature = "json-feed"), allow(dead_code))]
    json_enabled: bool,
    #[cfg_attr(not(feature = "atom"), allow(dead_code))]
    atom_enabled: bool,
    #[cfg_attr(not(feature = "atom"), allow(dead_code))]
    authors: Vec<String>,
    strict: bool,
    author_email: Option<String>,
    icon: Option<String>,
    favicon: Option<String>,
}

impl FeedConfig {
    /// Build the config from the mdBook context.
    ///
    /// Filename problems are printed as warnings, falling back to default
    /// names, unless `strict = true`, in which case they are printed as errors
    /// and the process exits after all of them are reported.
    #[expect(clippy::too_many_lines, reason = "inline config parsing and defaults")]
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

        let paginated = context
            .pointer("/config/preprocessor/rss-feed/paginated")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let json_enabled = context
            .pointer("/config/preprocessor/rss-feed/json-feed")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let atom_enabled = context
            .pointer("/config/preprocessor/rss-feed/atom")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let strict = context
            .pointer("/config/preprocessor/rss-feed/strict")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let requested_filename = |key: &str, default: &str| {
            context
                .pointer(&format!("/config/preprocessor/rss-feed/{key}"))
                .and_then(Value::as_str)
                .unwrap_or(default)
                .to_string()
        };

        let (output_filenames, filename_problems) = OutputFilenames::resolve(
            &requested_filename("rss-filename", DEFAULT_RSS_FILENAME),
            &requested_filename("atom-filename", DEFAULT_ATOM_FILENAME),
            &requested_filename("json-feed-filename", DEFAULT_JSON_FEED_FILENAME),
            atom_enabled,
            json_enabled,
            paginated,
        );

        for msg in &filename_problems {
            if strict {
                eprintln!("error: {msg}");
            } else {
                eprintln!("warning: {msg} (use strict = true to fail the build)");
            }
        }
        if strict && !filename_problems.is_empty() {
            std::process::exit(1);
        }

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
            paginated,
            max_items: context
                .pointer("/config/preprocessor/rss-feed/max-items")
                .and_then(Value::as_u64)
                .map_or(0, |n| usize::try_from(n).unwrap_or(usize::MAX)),
            output_filenames,
            default_behavior,
            json_enabled,
            atom_enabled,
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
            strict,
            author_email: context
                .pointer("/config/preprocessor/rss-feed/author-email")
                .and_then(Value::as_str)
                .map(str::to_string),
            icon: context
                .pointer("/config/preprocessor/rss-feed/icon")
                .and_then(Value::as_str)
                .map(str::to_string),
            favicon: context
                .pointer("/config/preprocessor/rss-feed/favicon")
                .and_then(Value::as_str)
                .map(str::to_string),
        }
    }
    fn feed_options(&self) -> FeedOptions<'_> {
        let book_author = self.authors.first().cloned();
        FeedOptions {
            title: &self.title,
            site_url: &self.site_url,
            description: &self.description,
            full_preview: self.full_preview,
            max_items: self.max_items,
            paginated: self.paginated,
            rss_filename: &self.output_filenames.rss,
            default_behavior: self.default_behavior.clone(),
            strict: self.strict,
            author_email: self.author_email.clone(),
            book_author,
            icon: self.icon.clone(),
            favicon: self.favicon.clone(),
        }
    }
}

/// Base output filenames for each feed format.
///
/// Guaranteed to be flat, valid, and without possible collisions if constructed
/// through [`Self::resolve`].
struct OutputFilenames {
    rss: String,
    atom: String,
    json_feed: String,
}

impl Default for OutputFilenames {
    fn default() -> Self {
        Self {
            rss: DEFAULT_RSS_FILENAME.to_string(),
            atom: DEFAULT_ATOM_FILENAME.to_string(),
            json_feed: DEFAULT_JSON_FEED_FILENAME.to_string(),
        }
    }
}

impl OutputFilenames {
    /// Resolve the requested filenames into ones that are safe to write,
    /// returning any problems found.
    ///
    /// An individual invalid name falls back to its own default. If the
    /// names of the enabled formats collide, or if pagination is enabled and
    /// the names would collide at later pages, all three fall back to their
    /// defaults.
    fn resolve(
        requested_rss: &str,
        requested_atom: &str,
        requested_json_feed: &str,
        atom_enabled: bool,
        json_enabled: bool,
        paginated: bool,
    ) -> (Self, Vec<String>) {
        let mut problems = Vec::new();

        let mut valid_or_default = |key: &str, name: &str, default: &str| {
            if let Some(reason) = invalid_filename_reason(name) {
                problems.push(format!(
                    "mdbook-rss-feed: invalid `{key}` value {name:?} ({reason})"
                ));

                default
            } else {
                name
            }
            .to_string()
        };

        let names = Self {
            rss: valid_or_default("rss-filename", requested_rss, DEFAULT_RSS_FILENAME),
            atom: valid_or_default("atom-filename", requested_atom, DEFAULT_ATOM_FILENAME),
            json_feed: valid_or_default(
                "json-feed-filename",
                requested_json_feed,
                DEFAULT_JSON_FEED_FILENAME,
            ),
        };

        // Only formats that will actually be written can collide.
        let names_to_write: Vec<&str> = [
            Some(names.rss.as_str()),
            atom_enabled.then_some(names.atom.as_str()),
            json_enabled.then_some(names.json_feed.as_str()),
        ]
        .into_iter()
        .flatten()
        .collect();

        // With pagination, later pages add numbers to each name, so page 2 of a
        // feed starting from `feed.xml` would collide with the first page of a
        // feed starting from `feed2.xml`.
        let names_to_compare: Vec<String> = names_to_write
            .iter()
            .map(|&name| {
                if paginated {
                    strip_page_number(name)
                } else {
                    name.to_string()
                }
            })
            .collect();

        if (1..names_to_compare.len()).any(|i| names_to_compare[..i].contains(&names_to_compare[i]))
        {
            problems.push(format!(
                "mdbook-rss-feed: output filenames collide ({})",
                names_to_write.join(", ")
            ));

            (Self::default(), problems)
        } else {
            (names, problems)
        }
    }
}

/// Return why `name` can't be used as an output filename, or `None` if it's
/// fine. `name` must be the name of a single, flat file with no whitespace.
fn invalid_filename_reason(name: &str) -> Option<&'static str> {
    match name {
        "" => Some("must not be empty"),
        "." | ".." => Some("must not refer to a directory"),
        path if path.contains(char::is_whitespace) => Some("must not contain whitespace"),
        path if path.contains(['/', '\\']) => Some("must not contain path separators"),
        _ => None,
    }
}

/// Remove any page number that `paginated_filename` could add, getting the base
/// name of a feed from any page.
///
/// Trailing digits are stripped from the part before the last extension,
/// splitting the same way `paginated_filename` does (e.g. `feed2.xml` and
/// `feed12.xml` both give `feed.xml`, and `.feed2` gives `.feed`).
fn strip_page_number(name: &str) -> String {
    if let Some((left, ext)) = name.rsplit_once('.')
        && !left.is_empty()
    {
        format!(
            "{}.{ext}",
            left.trim_end_matches(|c: char| c.is_ascii_digit())
        )
    } else {
        name.trim_end_matches(|c: char| c.is_ascii_digit())
            .to_string()
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
    use mdbook_rss_feed::{paginated_filename, rss_to_json_feed};

    if !config.json_enabled {
        return Ok(());
    }
    let base = config.site_url.trim_end_matches('/');
    let total = pages.len();
    for (page_idx, page) in pages.iter().enumerate() {
        let filename = paginated_filename(&config.output_filenames.json_feed, page_idx);
        let self_url = format!("{base}/{filename}");

        let next_url = if page_idx + 1 < total {
            Some(format!(
                "{base}/{}",
                paginated_filename(&config.output_filenames.json_feed, page_idx + 1)
            ))
        } else {
            None
        };
        let json_feed = rss_to_json_feed(
            &page.channel,
            Some(&self_url),
            next_url.as_deref(),
            &page.item_meta,
            config.icon.as_deref(),
            config.favicon.as_deref(),
        );
        let json_path = config.src_dir.join(&filename);
        fs::write(&json_path, serde_json::to_vec_pretty(&json_feed)?)?;
        let json_bytes = serde_json::to_vec_pretty(&json_feed)?;
        eprintln!(
            "Writing JSON page {} ({} bytes)",
            json_path.display(),
            json_bytes.len()
        );
    }
    Ok(())
}

#[cfg(not(feature = "json-feed"))]
fn write_json_pages(
    config: &FeedConfig,
    _pages: &[mdbook_rss_feed::FeedPage],
) -> Result<(), Box<dyn std::error::Error>> {
    if config.json_enabled {
        let msg = "mdbook-rss-feed: `json-feed = true` is set but this binary was \
                   compiled without the `json-feed` feature. Reinstall with: \
                   cargo install mdbook-rss-feed --features json-feed";
        if config.strict {
            eprintln!("error: {msg}");
            std::process::exit(1);
        }
        eprintln!("warning: {msg}");
    }
    Ok(())
}

#[cfg(feature = "atom")]
fn write_atom_pages(config: &FeedConfig, pages: &[mdbook_rss_feed::FeedPage]) -> io::Result<()> {
    use mdbook_rss_feed::{paginated_filename, rss_to_atom};

    if !config.atom_enabled {
        return Ok(());
    }
    let base = config.site_url.trim_end_matches('/');
    let total = pages.len();
    for (page_idx, page) in pages.iter().enumerate() {
        let filename = paginated_filename(&config.output_filenames.atom, page_idx);
        let self_url = format!("{base}/{filename}");
        // next points to older page, prev points to newer page.
        let next_url = if page_idx + 1 < total {
            Some(format!(
                "{base}/{}",
                paginated_filename(&config.output_filenames.atom, page_idx + 1)
            ))
        } else {
            None
        };
        let prev_url = if page_idx > 0 {
            Some(format!(
                "{base}/{}",
                paginated_filename(&config.output_filenames.atom, page_idx - 1)
            ))
        } else {
            None
        };
        let atom_feed = rss_to_atom(
            &page.channel,
            Some(&self_url),
            next_url.as_deref(),
            prev_url.as_deref(),
            &config.authors,
            &page.item_meta,
        );
        let atom_path = config.src_dir.join(&filename);
        fs::write(&atom_path, atom_feed.to_string())?;
        eprintln!(
            "Writing Atom page {} ({} bytes)",
            atom_path.display(),
            atom_feed.to_string().len()
        );
    }
    Ok(())
}

#[cfg(not(feature = "atom"))]
fn write_atom_pages(_config: &FeedConfig, _pages: &[mdbook_rss_feed::FeedPage]) -> io::Result<()> {
    if _config.atom_enabled {
        let msg = "mdbook-rss-feed: `atom = true` is set but this binary was \
                   compiled without the `atom` feature. Reinstall with: \
                   cargo install mdbook-rss-feed --features atom";
        if _config.strict {
            eprintln!("error: {msg}");
            std::process::exit(1);
        }
        eprintln!("warning: {msg}");
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if handle_mdbook_hooks(&args) {
        return Ok(());
    }
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    let input_array: Vec<Value> = serde_json::from_str(&input)?;
    let [context, book] = input_array.as_slice() else {
        return Err("expected mdBook to send a [context, book] pair on stdin".into());
    };

    let config = FeedConfig::from_json(context);

    let articles = articles_from_book_json(book, config.strict);

    eprintln!(
        "mdbook-rss-feed: collected {} chapter(s) from book (default-behavior: {:?})",
        articles.len(),
        config.default_behavior,
    );

    let result = build_feed_from_articles(articles, &config.feed_options());

    write_rss_pages(&config, &result.pages)?;
    write_json_pages(&config, &result.pages)?;
    write_atom_pages(&config, &result.pages)?;

    // FINAL ECHO TO MDBOOK
    io::stderr().flush()?;
    println!("{}", serde_json::to_string(book)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Build a config from `[preprocessor.rss-feed]` keys, as mdBook would
    /// pass them.
    fn config_with(rss_feed: &Value) -> FeedConfig {
        FeedConfig::from_json(&json!({
            "root": "/book",
            "config": { "preprocessor": { "rss-feed": rss_feed } },
        }))
    }

    /// Arguments for [`OutputFilenames::resolve`], defaulting to the same
    /// values [`FeedConfig::from_json`] uses when keys are missing.
    struct ResolveArgs<'a> {
        rss: &'a str,
        atom: &'a str,
        json_feed: &'a str,
        atom_enabled: bool,
        json_enabled: bool,
        paginated: bool,
    }

    impl Default for ResolveArgs<'_> {
        fn default() -> Self {
            Self {
                rss: DEFAULT_RSS_FILENAME,
                atom: DEFAULT_ATOM_FILENAME,
                json_feed: DEFAULT_JSON_FEED_FILENAME,
                atom_enabled: false,
                json_enabled: false,
                paginated: false,
            }
        }
    }

    impl ResolveArgs<'_> {
        fn resolve(self) -> (OutputFilenames, Vec<String>) {
            OutputFilenames::resolve(
                self.rss,
                self.atom,
                self.json_feed,
                self.atom_enabled,
                self.json_enabled,
                self.paginated,
            )
        }
    }

    /// Get the three output filenames (in the order RSS, Atom, JSON Feed).
    fn filenames_from(filenames: &OutputFilenames) -> [&str; 3] {
        [&filenames.rss, &filenames.atom, &filenames.json_feed]
    }

    #[test]
    fn invalid_filename_reason_accepts_flat_names() {
        for name in ["rss.xml", "feed.xml", "feed", ".feed", "my.feed.xml"] {
            assert_eq!(invalid_filename_reason(name), None, "{name:?}");
        }
    }

    #[test]
    fn invalid_filename_reason_rejects_unusable_names() {
        for name in [
            "",
            ".",
            "..",
            " ",
            "\t..",
            "rss.xml\n",
            "feeds/rss.xml",
            "../rss.xml",
            "/etc/rss.xml",
            r"feeds\rss.xml",
        ] {
            assert!(invalid_filename_reason(name).is_some(), "{name:?}");
        }
    }

    #[test]
    fn missing_keys_use_defaults() {
        let config = config_with(&json!({ "atom": true, "json-feed": true }));
        assert_eq!(
            filenames_from(&config.output_filenames),
            ["rss.xml", "atom.xml", "feed.json"]
        );
    }

    #[test]
    fn valid_custom_names_are_kept() {
        let config = config_with(&json!({
            "atom": true,
            "json-feed": true,
            "rss-filename": "feed.xml",
            "atom-filename": "index.atom",
            "json-feed-filename": "posts.json",
        }));
        assert_eq!(
            filenames_from(&config.output_filenames),
            ["feed.xml", "index.atom", "posts.json"]
        );
    }

    #[test]
    fn invalid_name_falls_back_to_its_individual_default() {
        let (filenames, problems) = ResolveArgs {
            rss: "../escape.xml",
            atom: "index.atom",
            atom_enabled: true,
            ..Default::default()
        }
        .resolve();

        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("rss-filename"), "{}", problems[0]);
        assert_eq!(
            filenames_from(&filenames),
            ["rss.xml", "index.atom", "feed.json"]
        );
    }

    #[test]
    fn collision_resets_all_names_to_defaults() {
        let (filenames, problems) = ResolveArgs {
            rss: "feed.xml",
            atom: "feed.xml",
            json_feed: "posts.json",
            atom_enabled: true,
            ..Default::default()
        }
        .resolve();

        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("collide"), "{}", problems[0]);
        assert_eq!(
            filenames_from(&filenames),
            ["rss.xml", "atom.xml", "feed.json"]
        );
    }

    #[test]
    fn custom_name_matching_another_default_collides() {
        // RSS takes the Atom default while Atom keeps it.
        let (filenames, problems) = ResolveArgs {
            rss: "atom.xml",
            atom_enabled: true,
            ..Default::default()
        }
        .resolve();

        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("collide"), "{}", problems[0]);
        assert_eq!(
            filenames_from(&filenames),
            ["rss.xml", "atom.xml", "feed.json"]
        );
    }

    #[test]
    fn strip_page_number_removes_trailing_stem_digits() {
        for (name, expected) in [
            ("feed.xml", "feed.xml"),
            ("feed2.xml", "feed.xml"),
            ("feed12.xml", "feed.xml"),
            ("my.feed2.xml", "my.feed.xml"),
            ("feed2", "feed"),
            (".feed2", ".feed"),
            ("v2-feed.xml", "v2-feed.xml"),
        ] {
            assert_eq!(strip_page_number(name), expected, "{name:?}");
        }
    }

    #[test]
    fn paginated_page_collision_is_detected() {
        // RSS page 2 would be `feed2.xml`, the same as Atom page 1.
        let (filenames, problems) = ResolveArgs {
            rss: "feed.xml",
            atom: "feed2.xml",
            atom_enabled: true,
            paginated: true,
            ..Default::default()
        }
        .resolve();

        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("collide"), "{}", problems[0]);
        assert_eq!(
            filenames_from(&filenames),
            ["rss.xml", "atom.xml", "feed.json"]
        );
    }

    #[test]
    fn numbered_names_do_not_collide_without_pagination() {
        // Only first pages are written, so `feed.xml` and `feed2.xml` coexist.
        let (filenames, problems) = ResolveArgs {
            rss: "feed.xml",
            atom: "feed2.xml",
            atom_enabled: true,
            ..Default::default()
        }
        .resolve();

        assert_eq!(problems, [] as [String; 0]);
        assert_eq!(
            filenames_from(&filenames),
            ["feed.xml", "feed2.xml", "feed.json"]
        );
    }

    #[test]
    fn paginated_names_with_different_extensions_do_not_collide() {
        let (filenames, problems) = ResolveArgs {
            rss: "feed.xml",
            json_feed: "feed2.json",
            json_enabled: true,
            paginated: true,
            ..Default::default()
        }
        .resolve();

        assert_eq!(problems, [] as [String; 0]);
        assert_eq!(
            filenames_from(&filenames),
            ["feed.xml", "atom.xml", "feed2.json"]
        );
    }

    #[test]
    fn disabled_format_cannot_collide() {
        // Atom is off, so its name is never written, even though its custom
        // name collides with the default filename for RSS.
        let (filenames, problems) = ResolveArgs {
            atom: "rss.xml",
            ..Default::default()
        }
        .resolve();

        assert_eq!(problems, [] as [String; 0]);
        assert_eq!(
            filenames_from(&filenames),
            ["rss.xml", "rss.xml", "feed.json"]
        );
    }
}
