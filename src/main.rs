use std::{path::PathBuf, vec};

use clap::{arg, Command};

struct Meta {
    name: String,
    type_: String,
}

struct Method {
    type_: reqwest::Method,
    url: String,
    // TODO: Expand this, the body might be more difficult.
    body: Option<String>,
    // TODO: Expand this, the auth might need some calculations. Using more types will help.
    auth: Option<String>,
}

fn main() {
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
                collection = scan_folder(&path);
            } else {
                collection = vec![std::path::PathBuf::from(&path)];
            }
            println!(
                "Running on {}",
                path
            );
            execute_collection(collection);
        }
        _ => unreachable!(),
        
    }
}

fn execute_collection(collection: Vec<PathBuf>) {
    for file in collection {
        if file.extension().unwrap() != "bru" {
            continue;
        }
        let content = std::fs::read_to_string(&file).unwrap();
        let lines = content.lines();
        for line in lines {
            println!("{}", line);
        }
    }
}


fn scan_folder(path: &str) -> Vec<PathBuf> {
    let mut folders = vec![];
    let mut files = vec![];

    folders.push(std::path::PathBuf::from(path));

    loop {
        if folders.len() > 0 {
            let folder = folders.pop().unwrap();
            let paths = std::fs::read_dir(&folder).unwrap();
            for path in paths {
                let path = path.unwrap().path();
                if path.is_dir() {
                    folders.push(path);
                } else {
                    files.push(path);
                }
            }
        } else {
            return files;
        }
    }
}

fn cli() -> Command {
    Command::new("bru-rs")
        .about("Bruno's bru cli app written in Rust")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("run")
                .about("Run the bruno collection")
                .arg(arg!(<ROUTE> "The path of the bruno collection"))
                .arg_required_else_help(true),
        )
}