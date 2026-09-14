use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use clap::{Command, arg};
use indicatif::{MultiProgress, ProgressBar};
use tokio::task::JoinSet;

mod collection;
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
            let route = run_matches.get_one::<String>("ROUTE").expect("required");
            let path = Path::new(route);
            // Check if the path is a folder or a file and that it exists
            if !path.exists() {
                eprintln!("The path {} does not exist", route);
                std::process::exit(1);
            }

            let bar = multi_bar.add(ProgressBar::new_spinner());
            bar.enable_steady_tick(Duration::from_millis(100));
            bar.set_message("🔍 Loading collection");
            // Store the current time
            let start = std::time::Instant::now();
            let collection = match collection::load(path) {
                Ok(collection) => collection,
                Err(error) => {
                    bar.finish_and_clear();
                    eprintln!("❌ Could not load the collection: {}", error);
                    std::process::exit(1);
                }
            };
            let files = collection.requests_in(path);
            // Print the time it took to load the collection
            let description = match collection.format {
                Some(collection::Format::Bru) => format!("bru collection {}", collection.name),
                Some(collection::Format::Yml) => format!("yml collection {}", collection.name),
                None => collection.root.display().to_string(),
            };
            bar.finish_with_message(format!(
                "✅ Loaded {} request(s) from {} in {:?}",
                files.len(),
                description,
                start.elapsed()
            ));

            // Progress bars are hidden when there is no terminal, so always print the problems
            for warning in &collection.warnings {
                eprintln!("  ⚠️  {}", warning);
            }
            if files.is_empty() {
                println!("No requests found.\nExiting 😉...");
                std::process::exit(0);
            }

            let mut requests = vec![];
            let mut errors = 0;
            for file in files {
                match &file.request {
                    Ok(request) => requests.push(request.clone()),
                    Err(error) => {
                        errors += 1;
                        let path = file
                            .path
                            .strip_prefix(&collection.root)
                            .unwrap_or(&file.path);
                        eprintln!("  ❌ Could not parse {}: {}", path.display(), error);
                    }
                }
            }
            println!("\n  🚀 Starting requests");
            execute_collection(requests, errors == 0, &multi_bar).await;
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
