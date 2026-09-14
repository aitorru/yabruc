use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
    vec,
};

use clap::{Command, arg};
use indicatif::{MultiProgress, ProgressBar};
use tokio::task::JoinSet;

mod hermes;
mod model;
mod parser;
use colored::Colorize;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let multi_bar = MultiProgress::new();
    let matches = cli().get_matches();
    match matches.subcommand() {
        Some(("run", run_matches)) => {
            let path = run_matches.get_one::<String>("ROUTE").expect("required");
            let collection;
            // Check if the path is a folder or a file and that it exists
            if !std::path::Path::new(&path).exists() {
                eprintln!("The path {} does not exist", path);
                std::process::exit(1);
            }
            if std::path::Path::new(&path).is_dir() {
                // Scan the folder for .bru files
                let bar = multi_bar.add(ProgressBar::new_spinner());
                bar.enable_steady_tick(Duration::from_millis(100));
                bar.set_message("🔍 Scanning folder");
                // Store the current time
                let start = std::time::Instant::now();
                collection = scan_folder(path);
                // Print the time it took to scan the folder
                bar.set_message(format!("✅ Scanned folder in {:?}", start.elapsed()));
                // Stop the spinner
                bar.finish();
            } else {
                let bar = multi_bar.add(ProgressBar::new_spinner());
                bar.enable_steady_tick(Duration::from_millis(100));
                let start = std::time::Instant::now();
                bar.set_message("🔍 Scanning file");
                collection = vec![std::path::PathBuf::from(&path)];
                // Print the time it took to scan the folder
                bar.set_message(format!("✅ Scanned file in {:?}", start.elapsed()));
                // Stop the spinner
                bar.finish();
            }
            if collection.is_empty() {
                println!("No .bru files found.\nExiting 😉...");
                std::process::exit(0);
            }
            let (queries, errors) = parser::parse_pathbuf(collection, &multi_bar).await;
            // Progress bars are hidden when there is no terminal, so always print the errors
            for error in &errors {
                eprintln!("  {}", error);
            }
            println!("\n  🚀 Starting requests");
            execute_collection(queries, errors.is_empty(), &multi_bar).await;
        }
        _ => unreachable!(),
    }
}

async fn execute_collection(
    queries: Vec<model::Request>,
    parsed_all: bool,
    multi_bar: &MultiProgress,
) {
    let state = Arc::new(Mutex::new(multi_bar.clone()));
    let mut set = JoinSet::new();
    for query in queries {
        let state_clone = state.clone();
        set.spawn(async move { hermes::requester::send_request(query, state_clone).await });
    }

    let mut results_bools = vec![];
    let mut results_requests = vec![];
    while let Some(result) = set.join_next().await {
        let (status, request) = result.expect("Request panicked");
        results_bools.push(status);
        results_requests.push(request);
    }

    // Check if all requests were successful
    if parsed_all && results_bools.iter().all(|&x| x) {
        println!("\nAll requests were successful! 🎉🎉🎉");
        // exit 0
        std::process::exit(0);
    } else {
        if !parsed_all {
            println!("\n  Some files could not be parsed");
        }
        // Search for the failed requests
        for (i, status) in results_bools.iter().enumerate() {
            if !status {
                println!(
                    "\n  Request {}\n  ➡️  Failed for {}",
                    results_requests[i].name.red(),
                    results_requests[i].url.red()
                );
            }
        }
        std::process::exit(1);
    }
}

fn scan_folder(path: &str) -> Vec<PathBuf> {
    let mut folders = vec![];
    let mut files = vec![];

    folders.push(std::path::PathBuf::from(path));

    loop {
        if !folders.is_empty() {
            let folder = folders.pop().unwrap();
            let paths = std::fs::read_dir(&folder).unwrap();
            for path in paths {
                let path = path.unwrap().path();
                if path.is_dir() {
                    folders.push(path);
                } else if path.extension().is_some_and(|ext| ext == "bru") {
                    files.push(path);
                }
            }
        } else {
            return files;
        }
    }
}

fn cli() -> Command {
    Command::new("yabruc")
        .about("Bruno's bru cli app written in Rust. Yet another bru compiler")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("run")
                .about("Run the bruno collection")
                .arg(arg!(<ROUTE> "The path of the bruno collection"))
                .arg_required_else_help(true),
        )
}
