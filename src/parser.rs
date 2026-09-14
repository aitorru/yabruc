use std::path::Path;

use crate::model::Request;

pub mod bru;
pub mod bru2struct;
pub mod yml2struct;

/// Parses a request using the format given by the extension of the file.
pub fn parse_request(path: &Path, source: &str) -> Result<Request, String> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("yml" | "yaml") => yml2struct::parse(source),
        _ => bru::parse(source)
            .map_err(|error| error.to_string())
            .and_then(|document| bru2struct::document_to_request(&document)),
    }
}
