use std::{path::PathBuf, vec};

use clap::{arg, Command};

mod parser;

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
            let queries = parser::bru2struct::parse_pathbuf(collection);
            execute_collection(queries);
        }
        _ => unreachable!(),
        
    }
}

fn execute_collection(queries: Vec<parser::bru2struct::Dog>) {
    todo!()
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
    Command::new("yabruc")
        .about("Bruno's bru cli app written in Rust. Yet another bru _compiler_")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("run")
                .about("Run the bruno collection")
                .arg(arg!(<ROUTE> "The path of the bruno collection"))
                .arg_required_else_help(true),
        )
}