//! Every `{{#include}}` in the book must resolve.
//!
//! The book's Rust snippets are pulled from the examples in this crate, so that
//! `cargo clippy --workspace --all-targets` compiles every line the book shows
//! and a snippet cannot drift from the API it documents. That only holds while
//! the includes themselves resolve, and mdBook is unhelpful here: a missing
//! *file* logs `[ERROR]` but still exits 0, and a missing *anchor* renders as an
//! empty code block with no diagnostic at all. Either way the book builds, the
//! page publishes, and the snippet is simply gone.
//!
//! So this test checks what mdBook will not. It lives in `viva-genicam` because
//! that is where the anchored examples live, and it runs under the ordinary
//! `cargo test --workspace` rather than in a docs-only CI job.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("resolve repository root")
}

fn markdown_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let path = entry.expect("read directory entry").path();
        if path.is_dir() {
            markdown_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            out.push(path);
        }
    }
}

/// The `{{#include <path>[:<anchor>]}}` directives in one document.
fn includes(markdown: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = markdown;
    while let Some(start) = rest.find("{{#include ") {
        rest = &rest[start + "{{#include ".len()..];
        let Some(end) = rest.find("}}") else { break };
        found.push(rest[..end].trim().to_string());
        rest = &rest[end + 2..];
    }
    found
}

/// Split a directive into its path and, when it names one, its anchor.
///
/// mdBook also accepts line ranges (`file.rs:10:20`, `file.rs::20`). Those are
/// resolved against the file's length rather than a name, so we check only that
/// the file exists.
fn split_spec(spec: &str) -> (&str, Option<&str>) {
    match spec.split_once(':') {
        None => (spec, None),
        Some((path, suffix)) => {
            let looks_like_line_range = suffix
                .split(':')
                .all(|part| part.is_empty() || part.parse::<usize>().is_ok());
            if looks_like_line_range {
                (path, None)
            } else {
                (path, Some(suffix))
            }
        }
    }
}

#[test]
fn book_includes_resolve() {
    let root = repo_root();
    let book_src = root.join("book").join("src");
    assert!(book_src.is_dir(), "{} is missing", book_src.display());

    let mut pages = Vec::new();
    markdown_files(&book_src, &mut pages);
    assert!(!pages.is_empty(), "no markdown found under book/src");

    let mut problems = Vec::new();
    let mut checked = 0usize;

    for page in &pages {
        let markdown = fs::read_to_string(page).expect("read book page");
        let page_dir = page.parent().expect("book page has a parent directory");

        for spec in includes(&markdown) {
            checked += 1;
            let (relative, anchor) = split_spec(&spec);
            let target = page_dir.join(relative);

            let Ok(source) = fs::read_to_string(&target) else {
                problems.push(format!(
                    "{}: include target does not exist: {}",
                    page.strip_prefix(&root).unwrap_or(page).display(),
                    relative
                ));
                continue;
            };

            let Some(anchor) = anchor else { continue };

            // mdBook matches `ANCHOR: name` and `ANCHOR_END: name` anywhere on a
            // line, in a comment of whatever shape the language uses.
            for marker in ["ANCHOR", "ANCHOR_END"] {
                let needle = format!("{marker}: {anchor}");
                if !source.lines().any(|line| line.contains(&needle)) {
                    problems.push(format!(
                        "{}: {} has no `{}`",
                        page.strip_prefix(&root).unwrap_or(page).display(),
                        relative,
                        needle
                    ));
                }
            }
        }
    }

    assert!(
        problems.is_empty(),
        "{} broken book include(s) out of {checked} checked:\n  {}",
        problems.len(),
        problems.join("\n  ")
    );
    assert!(
        checked > 0,
        "no {{{{#include}}}} directives found — the book snippets are no longer \
         sourced from compiled examples, which is the thing this test exists to protect"
    );
}
