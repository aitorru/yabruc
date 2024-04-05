use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Lines},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use indicatif::{MultiProgress, ProgressBar};
use tokio::task::JoinSet;

use lazy_static::lazy_static;

#[derive(Debug)]
pub struct Dog {
    meta: Meta,
    method: Method,
    variables: Variables,
}

#[derive(Debug)]
struct Meta {
    name: String,
    type_: String,
}

#[derive(Debug)]
struct Method {
    type_: reqwest::Method,
    url: String,
    // TODO: Expand this, the body might be more difficult.
    body: Option<Body>,
    // TODO: Expand this, the auth might need some calculations. Using more types will help.
    auth: Option<String>,
}

#[derive(Debug)]
struct Variables {
    pre: PreVars,
    post: PostVars,
}

#[derive(Debug)]
struct PreVars {
    vars: HashMap<String, String>,
}

#[derive(Debug)]
struct PostVars {
    vars: HashMap<String, String>,
}

#[derive(Debug)]
struct Body {
    type_: Option<BodyType>,
    value: String,
}

#[derive(Debug)]
enum BodyType {
    Json,
    Xml,
    Text,
    Sparql,
    Form,
    FormUrl,
}

pub async fn parse_pathbuf(collection: Vec<PathBuf>, multi_bar: &MultiProgress) -> Vec<Dog> {
    let state = Arc::new(Mutex::new(multi_bar.clone()));
    let mut set = JoinSet::new();
    for path in collection {
        let file = File::open(&path).expect("Could not open file");
        // Read the file with a buff reader
        let lines = BufReader::new(file).lines();
        // Create a worker that will read the file separated.
        let state_clone = state.clone();
        set.spawn(async move {
            parse_and_return_dog(lines, state_clone, path.display().to_string()).await
        });
    }
    // Wait for all the workers to finish and return the vec of dogs
    let mut dogs = vec![];
    while let Some(res) = set.join_next().await {
        let out = res.expect("Could not parse file");
        dogs.push(out);
    }
    // #[cfg(debug_assertions)]
    // let _ = multi_bar.println(format!("Dogs: {:?}\n", dogs));
    dogs
}

lazy_static! {
    static ref METHODS: HashMap<&'static str, reqwest::Method> = {
        let mut m = HashMap::new();
        m.insert("get", reqwest::Method::GET);
        m.insert("post", reqwest::Method::POST);
        m.insert("put", reqwest::Method::PUT);
        m.insert("delete", reqwest::Method::DELETE);
        m.insert("patch", reqwest::Method::PATCH);
        m.insert("options", reqwest::Method::OPTIONS);
        m.insert("head", reqwest::Method::HEAD);
        m.insert("connect", reqwest::Method::CONNECT);
        m.insert("trace", reqwest::Method::TRACE);
        m
    };
}

enum ParseState {
    Unknown,
    Meta,
    Method,
    VariablesPre,
    VariablesPost,
    BodyJson,
}

async fn parse_and_return_dog(
    lines: Lines<BufReader<File>>,
    state: Arc<Mutex<MultiProgress>>,
    file_name: String,
) -> Dog {
    let bar = state.lock().unwrap().add(ProgressBar::new_spinner());
    bar.set_message(format!("🔍 Parsing {} file.", file_name));
    let start = std::time::Instant::now();

    let mut state: ParseState = ParseState::Unknown;
    let mut final_dog: Dog = Dog {
        meta: Meta {
            name: "".to_string(),
            type_: "".to_string(),
        },
        method: Method {
            type_: reqwest::Method::GET,
            url: "".to_string(),
            body: None,
            auth: None,
        },
        variables: Variables {
            pre: PreVars {
                vars: HashMap::new(),
            },
            post: PostVars {
                vars: HashMap::new(),
            },
        },
    };
    for line in lines.flatten() {
        match state {
            ParseState::Unknown => {
                if line == "" {
                    continue;
                }
                if line.starts_with("meta") {
                    state = ParseState::Meta;
                }
                // Method range. It can be the following values
                // get | post | put | delete | patch | options | head | connect | trace
                if let Some(method) = METHODS.get(line.split_whitespace().next().unwrap()) {
                    final_dog.method.type_ = method.clone();
                    state = ParseState::Method;
                }

                if line.starts_with("vars:pre-request") {
                    state = ParseState::VariablesPre;
                }

                if line.starts_with("vars:post-request") {
                    state = ParseState::VariablesPost;
                }

                if line.starts_with("body:json") {
                    state = ParseState::BodyJson;
                }
            }
            ParseState::Meta => {
                if line.starts_with("}") {
                    state = ParseState::Unknown;
                    continue;
                }
                let (mut index, value) = line.split_at(line.find(":").unwrap());
                let value = value.split_at(1).1;
                // Remove stating and ending whitespaces in index
                while index.starts_with(" ") || index.ends_with(" ") {
                    index = index.trim();
                }
                if index == "name" {
                    final_dog.meta.name = value.trim().to_string();
                } else if index == "type" {
                    final_dog.meta.type_ = value.trim().to_string();
                }
            }
            ParseState::Method => {
                if line.starts_with("}") {
                    state = ParseState::Unknown;
                    continue;
                }
                let (mut index, value) = line.split_at(line.find(":").unwrap());
                let value = value.split_at(1).1;
                // Remove stating and ending whitespaces in index
                while index.starts_with(" ") || index.ends_with(" ") {
                    index = index.trim();
                }
                if index == "url" {
                    final_dog.method.url = value.trim().to_string();
                }
                if index == "body" {
                    // Body can be none or a string
                    match value.trim() {
                        "none" => {
                            final_dog.method.body = None;
                        }
                        "json" => {
                            final_dog.method.body = Some(Body {
                                type_: Some(BodyType::Json),
                                value: "".to_string(),
                            });
                        }
                        "xml" => {
                            final_dog.method.body = Some(Body {
                                type_: Some(BodyType::Xml),
                                value: "".to_string(),
                            });
                        }
                        "text" => {
                            final_dog.method.body = Some(Body {
                                type_: Some(BodyType::Text),
                                value: "".to_string(),
                            });
                        }
                        "sparql" => {
                            final_dog.method.body = Some(Body {
                                type_: Some(BodyType::Sparql),
                                value: "".to_string(),
                            });
                        }
                        "multipartForm" => {
                            final_dog.method.body = Some(Body {
                                type_: Some(BodyType::Form),
                                value: "".to_string(),
                            });
                        }
                        "formUrlEncoded" => {
                            final_dog.method.body = Some(Body {
                                type_: Some(BodyType::FormUrl),
                                value: "".to_string(),
                            });
                        }
                        _ => panic!("Unknown body type"),
                    }
                }
            }
            ParseState::VariablesPre => {
                if line.starts_with("}") {
                    state = ParseState::Unknown;
                    continue;
                }
                let (mut index, value) = line.split_at(line.find(":").unwrap());
                let value = value.split_at(1).1;
                // Remove stating and ending whitespaces in index
                while index.starts_with(" ") || index.ends_with(" ") {
                    index = index.trim();
                }

                // Add the index and the value to the hashmap
                final_dog
                    .variables
                    .pre
                    .vars
                    .insert(index.to_string(), value.trim().to_string());
            }
            ParseState::VariablesPost => {
                if line.starts_with("}") {
                    state = ParseState::Unknown;
                    continue;
                }
                let (mut index, value) = line.split_at(line.find(":").unwrap());
                let value = value.split_at(1).1;
                // Remove stating and ending whitespaces in index
                while index.starts_with(" ") || index.ends_with(" ") {
                    index = index.trim();
                }

                // Add the index and the value to the hashmap
                final_dog
                    .variables
                    .pre
                    .vars
                    .insert(index.to_string(), value.trim().to_string());
            }
            ParseState::BodyJson => {
                if line.starts_with("}") {
                    state = ParseState::Unknown;
                    continue;
                }
                // If the body is none, continue, as the option has been disabled
                if final_dog.method.body.is_none() {
                    continue;
                }
                if let Some(bodytype) = &(final_dog.method.body.as_mut().unwrap().type_) {
                    match bodytype {
                        BodyType::Json => {
                            final_dog.method.body.as_mut().unwrap().value += line.trim();
                        }
                        _ => continue,
                    }
                } else {
                    continue;
                }
            }
        }
    }
    bar.set_message(format!("✅ Parsed {} in {:?}", file_name, start.elapsed()));
    bar.finish();
    final_dog
}
