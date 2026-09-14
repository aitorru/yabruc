use std::path::{Path, PathBuf};

use indicatif::{MultiProgress, ProgressBar};
use tokio::task::JoinSet;

use crate::model::Request;

pub mod bru;
pub mod bru2struct;
pub mod yml2struct;

/// Parses all the files, returning the parsed requests and the errors of the files that failed.
pub async fn parse_pathbuf(
    collection: Vec<PathBuf>,
    multi_bar: &MultiProgress,
) -> (Vec<Request>, Vec<String>) {
    let mut set = JoinSet::new();
    for path in collection {
        // Create a worker that will read the file separated.
        let multi_bar = multi_bar.clone();
        set.spawn(async move { parse_file(path, multi_bar).await });
    }
    // Wait for all the workers to finish and return the requests
    let mut requests = vec![];
    let mut errors = vec![];
    while let Some(res) = set.join_next().await {
        match res.expect("Could not parse file") {
            Ok(request) => requests.push(request),
            Err(error) => errors.push(error),
        }
    }
    (requests, errors)
}

async fn parse_file(path: PathBuf, multi_bar: MultiProgress) -> Result<Request, String> {
    let file_name = path.display().to_string();
    let bar = multi_bar.add(ProgressBar::new_spinner());
    bar.set_message(format!("🔍 Parsing {} file.", file_name));
    let start = std::time::Instant::now();

    let parsed = std::fs::read_to_string(&path)
        .map_err(|error| error.to_string())
        .and_then(|source| parse_request(&path, &source));

    match parsed {
        Ok(request) => {
            bar.finish_with_message(format!("✅ Parsed {} in {:?}", file_name, start.elapsed()));
            Ok(request)
        }
        Err(error) => {
            let error = format!("❌ Could not parse {}: {}", file_name, error);
            bar.finish_with_message(error.clone());
            Err(error)
        }
    }
}

/// Parses a request using the format given by the extension of the file.
fn parse_request(path: &Path, source: &str) -> Result<Request, String> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("yml" | "yaml") => yml2struct::parse(source),
        _ => bru::parse(source)
            .map_err(|error| error.to_string())
            .and_then(|document| bru2struct::document_to_request(&document)),
    }
}
