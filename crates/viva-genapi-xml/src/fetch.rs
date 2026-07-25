//! URL parsing and XML document retrieval utilities.

use std::future::Future;

use crate::XmlError;
use crate::util::parse_u64;

/// Address of the first URL register in the GigE Vision bootstrap register map.
/// GigE Vision spec: GevFirstURL at 0x0200 (512 bytes max).
const FIRST_URL_ADDRESS: u64 = 0x0200;
/// Address of the second URL register (GevSecondURL at 0x0400, 512 bytes max).
/// Used as a fallback when the first URL is empty or cannot be retrieved.
const SECOND_URL_ADDRESS: u64 = 0x0400;
/// Maximum length of a URL register string.
const URL_MAX_LEN: usize = 512;

/// ZIP local file header magic: `PK\x03\x04`.
const ZIP_MAGIC: &[u8; 4] = b"PK\x03\x04";

/// If `data` starts with a ZIP signature, extract the first file's contents.
/// Otherwise return `data` unchanged.
///
/// GenICam devices commonly serve their register-description XML as a ZIP
/// archive (the URL then ends in `.zip`), which the standard requires
/// consumers to handle transparently.
pub fn decompress_if_zip(data: Vec<u8>) -> Result<Vec<u8>, XmlError> {
    use std::io::Read;

    if data.len() < 4 || &data[..4] != ZIP_MAGIC {
        return Ok(data);
    }
    let cursor = std::io::Cursor::new(&data);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| XmlError::Invalid(format!("bad XML ZIP archive: {e}")))?;
    if archive.is_empty() {
        return Err(XmlError::Invalid("XML ZIP archive is empty".into()));
    }
    let mut file = archive
        .by_index(0)
        .map_err(|e| XmlError::Invalid(format!("cannot read XML ZIP entry: {e}")))?;
    let mut xml = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut xml)
        .map_err(|e| XmlError::Invalid(format!("XML ZIP decompression failed: {e}")))?;
    Ok(xml)
}

/// Fetch the GenICam XML document using the provided memory reader closure.
///
/// The closure must return the requested number of bytes starting at the
/// provided address. It can internally perform chunked transfers.
///
/// Reads `GevFirstURL`, falling back to `GevSecondURL` when the first URL is
/// empty or its document cannot be retrieved. ZIP-compressed XML documents
/// are decompressed transparently.
pub async fn fetch_and_load_xml<F, Fut>(mut read_mem: F) -> Result<String, XmlError>
where
    F: FnMut(u64, usize) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, XmlError>>,
{
    match fetch_from_url_register(&mut read_mem, FIRST_URL_ADDRESS).await {
        Ok(xml) => Ok(xml),
        Err(first_err) => {
            tracing::debug!(error = %first_err, "first URL failed, trying GevSecondURL");
            match fetch_from_url_register(&mut read_mem, SECOND_URL_ADDRESS).await {
                Ok(xml) => Ok(xml),
                // The first URL's error is the more useful diagnostic: the
                // second URL is typically absent on devices where the first
                // one was already the problem.
                Err(second_err) => {
                    tracing::debug!(error = %second_err, "second URL failed as well");
                    Err(first_err)
                }
            }
        }
    }
}

/// Fetch and decode the XML document referenced by the URL register at `url_addr`.
async fn fetch_from_url_register<F, Fut>(
    read_mem: &mut F,
    url_addr: u64,
) -> Result<String, XmlError>
where
    F: FnMut(u64, usize) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, XmlError>>,
{
    let url_bytes = read_mem(url_addr, URL_MAX_LEN).await?;
    let url = first_cstring(&url_bytes)
        .ok_or_else(|| XmlError::Invalid("URL register is empty".into()))?;
    let location = UrlLocation::parse(&url)?;
    match location {
        UrlLocation::Local { address, length } => {
            let xml_bytes = read_mem(address, length).await?;
            let xml_bytes = decompress_if_zip(xml_bytes)?;
            String::from_utf8(xml_bytes)
                .map_err(|err| XmlError::Xml(format!("invalid UTF-8: {err}")))
        }
        UrlLocation::LocalNamed(name) => Err(XmlError::Unsupported(format!(
            "named local URL '{name}' is not supported"
        ))),
        UrlLocation::Http(url) => Err(XmlError::Unsupported(format!(
            "HTTP retrieval is not implemented ({url})"
        ))),
        UrlLocation::File(path) => Err(XmlError::Unsupported(format!(
            "file URL '{path}' is not supported"
        ))),
    }
}

/// Extract the first null-terminated C string from a byte buffer.
fn first_cstring(bytes: &[u8]) -> Option<String> {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let slice = &bytes[..end];
    let value = String::from_utf8_lossy(slice).trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

/// Parsed URL location variants.
#[derive(Debug)]
enum UrlLocation {
    /// Memory-mapped local XML at a fixed address.
    Local { address: u64, length: usize },
    /// Named local XML resource (unsupported).
    #[allow(dead_code)]
    LocalNamed(String),
    /// HTTP(S) remote URL (unsupported).
    Http(String),
    /// File system URL (unsupported).
    File(String),
}

impl UrlLocation {
    fn parse(url: &str) -> Result<Self, XmlError> {
        let lower = url.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("local:") {
            // Use original URL (case-preserved) for the rest, at same offset.
            let rest_original = &url[url.len() - rest.len()..];
            parse_local_url(rest_original)
        } else if lower.starts_with("http://") || lower.starts_with("https://") {
            Ok(UrlLocation::Http(url.to_string()))
        } else if lower.starts_with("file://") {
            Ok(UrlLocation::File(url.to_string()))
        } else {
            Err(XmlError::Unsupported(format!("unknown URL scheme: {url}")))
        }
    }
}

/// Parse a `local:` URL into its components.
///
/// Supports two formats:
///
/// 1. **Key-value**: `local:address=0x10;length=0x3`
/// 2. **GenICam standard**: `Local:///filename;hex_address;hex_length`
///
/// In format 2, the filename is optional (can be just `///;addr;len`).
fn parse_local_url(rest: &str) -> Result<UrlLocation, XmlError> {
    // Strip the `///` prefix used by the standard GenICam URL format.
    let trimmed = rest.strip_prefix("///").unwrap_or(rest).trim();
    if trimmed.is_empty() {
        return Err(XmlError::Invalid("empty local URL".into()));
    }

    let parts: Vec<&str> = trimmed.split(';').collect();

    // GenICam standard format: filename;hex_address;hex_length (3 semicolon-separated parts).
    if parts.len() >= 3 {
        let addr_str = parts[parts.len() - 2].trim();
        let len_str = parts[parts.len() - 1].trim();
        if let (Ok(address), Ok(length)) = (
            u64::from_str_radix(addr_str, 16),
            u64::from_str_radix(len_str, 16),
        ) {
            return Ok(UrlLocation::Local {
                address,
                length: length as usize,
            });
        }
    }

    // Fall back to key-value parsing.
    let mut address = None;
    let mut length = None;
    for part in parts {
        let token = part.trim();
        if token.is_empty() {
            continue;
        }
        if let Some((key, value)) = token.split_once('=') {
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();
            match key.as_str() {
                "address" | "addr" | "offset" => {
                    address = Some(parse_u64(value)?);
                }
                "length" | "size" => {
                    let len = parse_u64(value)?;
                    length = Some(
                        len.try_into()
                            .map_err(|_| XmlError::Invalid("length does not fit usize".into()))?,
                    );
                }
                _ => {}
            }
        } else if token.starts_with("0x") {
            address = Some(parse_u64(token)?);
        }
        // Ignore unrecognized tokens (like the filename).
    }
    match (address, length) {
        (Some(address), Some(length)) => Ok(UrlLocation::Local { address, length }),
        _ => Err(XmlError::Invalid(format!("unsupported local URL: {rest}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compress `data` into a single-entry ZIP archive (deflate).
    fn zip_bytes(name: &str, data: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        writer.start_file(name, options).expect("start zip entry");
        writer.write_all(data).expect("write zip entry");
        writer.finish().expect("finish zip").into_inner()
    }

    #[tokio::test]
    async fn fetch_local_zipped_xml() {
        let url = b"Local:camera.zip;10;9999\0".to_vec();
        let zipped = zip_bytes("camera.xml", b"<a/>");
        let mut url_reg = url.clone();
        url_reg.resize(URL_MAX_LEN, 0);
        let expected_len = zipped.len();
        let loaded = fetch_and_load_xml(|addr, len| {
            let url_reg = url_reg.clone();
            let zipped = zipped.clone();
            async move {
                if addr == FIRST_URL_ADDRESS {
                    Ok(url_reg)
                } else if addr == 0x10 && len == 0x9999 {
                    // Devices serve the advertised length; extra bytes past
                    // the archive end are padding.
                    let mut data = zipped;
                    data.resize(len.max(expected_len), 0);
                    Ok(data)
                } else {
                    Err(XmlError::Transport("unexpected read".into()))
                }
            }
        })
        .await
        .expect("load zipped xml");
        assert_eq!(loaded, "<a/>");
    }

    #[tokio::test]
    async fn falls_back_to_second_url() {
        let url = b"local:address=0x20;length=0x4\0".to_vec();
        let loaded = fetch_and_load_xml(|addr, len| {
            let url = url.clone();
            async move {
                if addr == FIRST_URL_ADDRESS {
                    // Empty first URL register.
                    Ok(vec![0u8; URL_MAX_LEN])
                } else if addr == SECOND_URL_ADDRESS {
                    Ok(url)
                } else if addr == 0x20 && len == 0x4 {
                    Ok(b"<b/>".to_vec())
                } else {
                    Err(XmlError::Transport("unexpected read".into()))
                }
            }
        })
        .await
        .expect("load xml via second URL");
        assert_eq!(loaded, "<b/>");
    }

    #[tokio::test]
    async fn first_url_error_is_reported_when_both_fail() {
        let err = fetch_and_load_xml(|_, _| async { Ok(vec![0u8; URL_MAX_LEN]) })
            .await
            .expect_err("both URLs empty");
        assert!(matches!(err, XmlError::Invalid(_)));
    }

    #[test]
    fn decompress_passes_through_plain_xml() {
        let data = b"<plain/>".to_vec();
        assert_eq!(decompress_if_zip(data.clone()).unwrap(), data);
    }

    #[tokio::test]
    async fn fetch_local_xml() {
        let data = b"local:address=0x10;length=0x3\0".to_vec();
        let xml_payload = b"<a/>".to_vec();
        let loaded = fetch_and_load_xml(|addr, len| {
            let data = data.clone();
            let xml_payload = xml_payload.clone();
            async move {
                if addr == FIRST_URL_ADDRESS {
                    Ok(data)
                } else if addr == 0x10 && len == 0x3 {
                    Ok(xml_payload)
                } else {
                    Err(XmlError::Transport("unexpected read".into()))
                }
            }
        })
        .await
        .expect("load xml");
        assert_eq!(loaded, "<a/>");
    }
}
