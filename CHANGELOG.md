# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.7.0] - 2026-08-08

### Added

- strict mode: add `strict = true` to `book.toml` and the book will now fail to
  build on any frontmatter parse error.

- self_url, next_url, and prev_url for json-feed and atom-feed added to
  paginated output


### Fixed

- atom-feeds update time now sets to the most recent entry's update time rather
  than the Unix epoch.

## [1.6.0] - 2026-08-08

### Added

- support for `{{include file.rs}}`: now `mdbook-rss-feed` respects this.

- default-behavior: set to `default-behavior = "exclude all"` for opt-in mode
  where only chapters marked `feed = include` are included in the feed output.

### Fixed

- README now correctly represents behavior
