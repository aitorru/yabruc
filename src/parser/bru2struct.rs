use std::path::PathBuf;

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

pub fn parse_pathbuf(collection: Vec<PathBuf>) -> Vec<Dog> {
    todo!()
}