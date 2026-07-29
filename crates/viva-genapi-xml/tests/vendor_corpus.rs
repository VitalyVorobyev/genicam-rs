//! Conformance test against real-camera GenICam XML descriptions.
//!
//! Our fake cameras and hand-written fixtures only exercise the constructs we
//! already thought of. Real vendor documents are where the surprises live —
//! issue #45 (a CDATA-wrapped formula) and issue #35 (a constant-formula
//! SwissKnife) were both found in the field, on cameras we cannot buy, after
//! users had already been blocked by them.
//!
//! The corpus is not committed: the documents are vendor copyright, published
//! for interoperability by third-party projects, so we fetch rather than
//! redistribute. Populate it with:
//!
//! ```sh
//! scripts/fetch-xml-corpus.sh
//! cargo test -p viva-genapi-xml --test vendor_corpus -- --nocapture
//! ```
//!
//! Set `VIVA_GENICAM_XML_CORPUS` to test a different directory — point it at
//! XML dumped from your own hardware to check a camera before you own it in
//! production.
//!
//! When the directory is absent the test passes with a note, so CI and a fresh
//! clone stay green.

use std::path::{Path, PathBuf};

/// Default corpus location relative to the workspace root.
const DEFAULT_CORPUS: &str = "fixtures/vendor-xml";

/// Nodes we knowingly cannot represent yet, as `(document, node name)`.
///
/// Everything else that fails to parse is a regression. Keep this list short
/// and each entry tied to a `docs/backlog.md` task.
const EXPECTED_SKIPS: &[(&str, &str)] = &[
    // XML-01: negative register address (`<Address>-4</Address>`), used for
    // offsets relative to the end of a chunk block. Our addressing model is
    // unsigned.
    ("Baumer_HXG20.xml", "ChunkImageLength"),
];

fn corpus_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("VIVA_GENICAM_XML_CORPUS") {
        return PathBuf::from(dir);
    }
    // CARGO_MANIFEST_DIR is crates/viva-genapi-xml.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(DEFAULT_CORPUS)
}

fn is_expected_skip(document: &str, node: Option<&str>) -> bool {
    EXPECTED_SKIPS
        .iter()
        .any(|(doc, name)| *doc == document && Some(*name) == node)
}

#[test]
fn vendor_xml_corpus_parses() {
    let dir = corpus_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        println!(
            "corpus not present at {}; run scripts/fetch-xml-corpus.sh to enable this test",
            dir.display()
        );
        return;
    };

    let mut documents: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "xml"))
        .collect();
    documents.sort();

    if documents.is_empty() {
        println!(
            "corpus at {} is empty; run scripts/fetch-xml-corpus.sh",
            dir.display()
        );
        return;
    }

    let mut failures = Vec::new();
    let mut total_nodes = 0usize;

    for path in &documents {
        let document = path
            .file_name()
            .expect("directory entry has a file name")
            .to_string_lossy()
            .into_owned();
        let bytes = std::fs::read(path).expect("read corpus document");
        // Encoding handling is tracked separately (XML-02); lossy decoding keeps
        // this test focused on the parser.
        let xml = String::from_utf8_lossy(&bytes);

        match viva_genapi_xml::parse(&xml) {
            Ok(model) => {
                total_nodes += model.nodes.len();
                let unexpected: Vec<_> = model
                    .skipped
                    .iter()
                    .filter(|skipped| !is_expected_skip(&document, skipped.name.as_deref()))
                    .collect();
                if unexpected.is_empty() {
                    println!(
                        "ok    {document}: {} nodes, {} known gaps",
                        model.nodes.len(),
                        model.skipped.len()
                    );
                } else {
                    println!("SKIP  {document}: {} unexpected", unexpected.len());
                    for skipped in &unexpected {
                        println!(
                            "        <{}> {:?}: {}",
                            skipped.tag, skipped.name, skipped.error
                        );
                        failures.push(format!(
                            "{document}: node <{}> {:?} skipped: {}",
                            skipped.tag, skipped.name, skipped.error
                        ));
                    }
                }
            }
            Err(err) => {
                println!("FAIL  {document}: {err}");
                failures.push(format!("{document}: {err}"));
            }
        }
    }

    println!(
        "=== {} documents, {total_nodes} nodes, {} failures ===",
        documents.len(),
        failures.len()
    );

    assert!(
        failures.is_empty(),
        "{} corpus document(s) regressed:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}
