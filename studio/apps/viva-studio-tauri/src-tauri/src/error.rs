/// Map `Err` strings through [`humanize_error`] at the IPC boundary.
pub trait HumanizeExt<T> {
    fn humanize(self) -> Result<T, String>;
}

impl<T> HumanizeExt<T> for Result<T, String> {
    fn humanize(self) -> Result<T, String> {
        self.map_err(|e| humanize_error(&e))
    }
}

/// Convert raw internal error strings into user-friendly messages.
///
/// Errors returned from Tauri commands are shown directly in the UI.
/// This function rewrites common Zenoh/service/validation patterns
/// into messages a machine-vision engineer can act on.
pub fn humanize_error(raw: &str) -> String {
    // Timeout / no reply from service
    if (raw.contains("No reply") || raw.contains("no reply")) && raw.contains("timeout") {
        return "Camera service did not respond. Is it running?".to_string();
    }

    // Zenoh transport-level errors
    if raw.contains("Zenoh GET error") || raw.contains("Zenoh error") {
        return "Communication error with camera service".to_string();
    }

    // Node not in cache
    if raw.contains("not in cache") {
        if let Some(name) = extract_quoted(raw) {
            return format!("Feature '{name}' is not available on this camera");
        }
        return "Feature is not available on this camera".to_string();
    }

    // Read-only access
    if raw.contains("not writable") && (raw.contains("RO") || raw.contains("NA")) {
        if let Some(name) = extract_quoted(raw) {
            return format!("Feature '{name}' is read-only");
        }
        return "Feature is read-only".to_string();
    }

    // Not connected
    if raw.contains("Not connected") || raw.contains("not connected") {
        return "No camera connected".to_string();
    }

    // No Zenoh session
    if raw.contains("Zenoh session not initialized") {
        return "Camera service connection not established. Try restarting the app.".to_string();
    }

    // Pass through — already user-friendly (validation messages, service error responses)
    raw.to_string()
}

/// Extract the first single-quoted substring from an error message.
fn extract_quoted(s: &str) -> Option<&str> {
    let start = s.find('\'')?;
    let rest = &s[start + 1..];
    let end = rest.find('\'')?;
    Some(&rest[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_no_reply() {
        let msg = humanize_error("No reply for write_node 'Width' (timeout)");
        assert_eq!(msg, "Camera service did not respond. Is it running?");
    }

    #[test]
    fn zenoh_get_error() {
        let msg = humanize_error("Zenoh GET error: session closed");
        assert_eq!(msg, "Communication error with camera service");
    }

    #[test]
    fn zenoh_error() {
        let msg = humanize_error("Zenoh error: transport failure");
        assert_eq!(msg, "Communication error with camera service");
    }

    #[test]
    fn node_not_in_cache() {
        let msg = humanize_error("Node 'Gain' not in cache");
        assert_eq!(msg, "Feature 'Gain' is not available on this camera");
    }

    #[test]
    fn read_only_node() {
        let msg = humanize_error("Node 'SensorWidth' is not writable (access_mode=RO)");
        assert_eq!(msg, "Feature 'SensorWidth' is read-only");
    }

    #[test]
    fn not_connected() {
        let msg = humanize_error("Not connected to a device");
        assert_eq!(msg, "No camera connected");
    }

    #[test]
    fn no_zenoh_session() {
        let msg = humanize_error("Zenoh session not initialized");
        assert_eq!(
            msg,
            "Camera service connection not established. Try restarting the app."
        );
    }

    #[test]
    fn passthrough_validation_error() {
        let raw = "Value 5000 exceeds maximum 4096 for node 'Width'";
        assert_eq!(humanize_error(raw), raw);
    }

    #[test]
    fn passthrough_enum_error() {
        let raw = "'JPEG' is not a valid entry for enumeration node 'PixelFormat'";
        assert_eq!(humanize_error(raw), raw);
    }

    #[test]
    fn extract_quoted_basic() {
        assert_eq!(extract_quoted("Node 'Width' not found"), Some("Width"));
    }

    #[test]
    fn extract_quoted_none() {
        assert_eq!(extract_quoted("no quotes here"), None);
    }
}
