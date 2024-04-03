use std::{fs::File, io::{BufRead, BufReader, Lines}, path::PathBuf};

use tokio::task::JoinSet;

pub struct Dog {
    meta: Meta,
    method: Method,
}

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

pub async fn parse_pathbuf(collection: Vec<PathBuf>) -> Vec<Dog> {
    let mut set = JoinSet::new();
    for path in collection {
        let file = File::open(path).expect("Could not open file");
        // Read the file with a buff reader
        let lines = BufReader::new(file).lines();
        // Create a worker that will read the file separated.
        set.spawn(async {
            parse_and_return_dog(lines).await
        });
    }
    // Wait for all the workers to finish and return the vec of dogs
    let mut dogs = vec![];
    while let Some(res) = set.join_next().await {
        let out = res.expect("Could not parse file");
        dogs.push(out);
    }
    dogs
}

async fn parse_and_return_dog(lines: Lines<BufReader<File>>) -> Dog {
    for line in lines.flatten() {
        println!("{}", line);
    }
    todo!()
}