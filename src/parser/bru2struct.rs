use std::{collections::HashMap, path::PathBuf, sync::LazyLock};

use indicatif::{MultiProgress, ProgressBar};
use tokio::task::JoinSet;

use super::bru::{self, Document};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Dog {
    pub meta: Meta,
    pub method: Method,
    pub variables: Variables,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Meta {
    pub name: String,
    pub type_: String,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Method {
    pub type_: reqwest::Method,
    pub url: String,
    // TODO: Expand this, the body might be more difficult.
    pub body: Option<Body>,
    // TODO: Expand this, the auth might need some calculations. Using more types will help.
    pub auth: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Variables {
    pub pre: PreVars,
    pub post: PostVars,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PreVars {
    pub vars: HashMap<String, String>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PostVars {
    pub vars: HashMap<String, String>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Body {
    pub type_: Option<BodyType>,
    pub value: String,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BodyType {
    Json,
    Xml,
    Text,
    Sparql,
    Graphql,
    Form,
    FormUrl,
    File,
}

/// Parses all the files, returning the parsed requests and the errors of the files that failed.
pub async fn parse_pathbuf(
    collection: Vec<PathBuf>,
    multi_bar: &MultiProgress,
) -> (Vec<Dog>, Vec<String>) {
    let mut set = JoinSet::new();
    for path in collection {
        // Create a worker that will read the file separated.
        let multi_bar = multi_bar.clone();
        set.spawn(async move { parse_and_return_dog(path, multi_bar).await });
    }
    // Wait for all the workers to finish and return the vec of dogs
    let mut dogs = vec![];
    let mut errors = vec![];
    while let Some(res) = set.join_next().await {
        match res.expect("Could not parse file") {
            Ok(dog) => dogs.push(dog),
            Err(error) => errors.push(error),
        }
    }
    (dogs, errors)
}

static METHODS: LazyLock<HashMap<&'static str, reqwest::Method>> = LazyLock::new(|| {
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
});

async fn parse_and_return_dog(path: PathBuf, multi_bar: MultiProgress) -> Result<Dog, String> {
    let file_name = path.display().to_string();
    let bar = multi_bar.add(ProgressBar::new_spinner());
    bar.set_message(format!("🔍 Parsing {} file.", file_name));
    let start = std::time::Instant::now();

    let parsed = std::fs::read_to_string(&path)
        .map_err(|error| error.to_string())
        .and_then(|source| bru::parse(&source).map_err(|error| error.to_string()))
        .and_then(|document| document_to_dog(&document));

    match parsed {
        Ok(dog) => {
            bar.finish_with_message(format!("✅ Parsed {} in {:?}", file_name, start.elapsed()));
            Ok(dog)
        }
        Err(error) => {
            let error = format!("❌ Could not parse {}: {}", file_name, error);
            bar.finish_with_message(error.clone());
            Err(error)
        }
    }
}

fn document_to_dog(document: &Document) -> Result<Dog, String> {
    let meta = document.block("meta");
    let meta = Meta {
        name: meta
            .and_then(|m| m.get("name"))
            .unwrap_or_default()
            .to_string(),
        type_: meta
            .and_then(|m| m.get("type"))
            .unwrap_or("http")
            .to_string(),
    };

    let (type_, request) = document
        .blocks
        .iter()
        .find_map(|block| {
            if block.name == "http" {
                let method =
                    reqwest::Method::from_bytes(block.get("method")?.to_uppercase().as_bytes())
                        .ok()?;
                return Some((method, block));
            }
            METHODS
                .get(block.name.as_str())
                .map(|method| (method.clone(), block))
        })
        .ok_or("no request block (get, post, put...) found")?;

    let body = match request.get("body").unwrap_or("none") {
        "none" => None,
        mode => {
            let (type_, block) = match mode {
                "json" => (BodyType::Json, "body:json"),
                "xml" => (BodyType::Xml, "body:xml"),
                "text" => (BodyType::Text, "body:text"),
                "sparql" => (BodyType::Sparql, "body:sparql"),
                "graphql" => (BodyType::Graphql, "body:graphql"),
                "multipartForm" => (BodyType::Form, "body:multipart-form"),
                "formUrlEncoded" => (BodyType::FormUrl, "body:form-urlencoded"),
                "file" => (BodyType::File, "body:file"),
                _ => return Err(format!("unknown body type `{mode}`")),
            };
            Some(Body {
                type_: Some(type_),
                value: document
                    .block(block)
                    .and_then(|block| block.text())
                    .unwrap_or_default()
                    .to_string(),
            })
        }
    };

    let auth = request
        .get("auth")
        .filter(|auth| *auth != "none")
        .map(str::to_string);

    Ok(Dog {
        meta,
        method: Method {
            type_,
            url: request.get("url").unwrap_or_default().to_string(),
            body,
            auth,
        },
        variables: Variables {
            pre: PreVars {
                vars: enabled_pairs(document, "vars:pre-request"),
            },
            post: PostVars {
                vars: enabled_pairs(document, "vars:post-response"),
            },
        },
    })
}

fn enabled_pairs(document: &Document, name: &str) -> HashMap<String, String> {
    let Some(block) = document.block(name) else {
        return HashMap::new();
    };
    block
        .pairs()
        .iter()
        .filter_map(|pair| Some((pair.key.clone(), block.get(&pair.key)?.to_string())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dog(source: &str) -> Result<Dog, String> {
        document_to_dog(&bru::parse(source).unwrap())
    }

    #[test]
    fn maps_official_request_fixture() {
        let dog = dog(include_str!("../../test/fixtures/bru/request.bru")).unwrap();
        assert_eq!(dog.meta.name, "Send Bulk SMS");
        assert_eq!(dog.meta.type_, "http");
        assert_eq!(dog.method.type_, reqwest::Method::GET);
        assert_eq!(dog.method.url, "https://api.textlocal.in/send/:id");
        assert_eq!(dog.method.auth.as_deref(), Some("bearer"));
        assert_eq!(
            dog.method.body,
            Some(Body {
                type_: Some(BodyType::Json),
                value: "{\n  \"hello\": \"world\"\n}".to_string(),
            })
        );
        assert_eq!(
            dog.variables.pre.vars,
            HashMap::from([("departingDate".to_string(), "2020-01-01".to_string())])
        );
        assert_eq!(dog.variables.post.vars.len(), 2);
        assert_eq!(dog.variables.post.vars["token"], "$res.body.token");
    }

    #[test]
    fn maps_every_example_request() {
        for (source, method) in [
            (
                include_str!("../../test/yabruc-bruno/Example GET.bru"),
                reqwest::Method::GET,
            ),
            (
                include_str!("../../test/yabruc-bruno/Example POST.bru"),
                reqwest::Method::POST,
            ),
            (
                include_str!("../../test/yabruc-bruno/Example PUT.bru"),
                reqwest::Method::PUT,
            ),
        ] {
            let dog = dog(source).unwrap();
            assert_eq!(dog.method.type_, method);
            assert_eq!(dog.method.url, "http://localhost:1234");
        }
    }

    #[test]
    fn maps_graphql_and_custom_methods() {
        let dog = dog("meta {\n  name: gql\n}\n\nhttp {\n  method: purge\n  url: http://localhost\n  body: graphql\n}\n\nbody:graphql {\n  { launches { id } }\n}\n").unwrap();
        assert_eq!(dog.method.type_.as_str(), "PURGE");
        assert_eq!(dog.method.body.unwrap().value, "{ launches { id } }");
    }

    #[test]
    fn reports_invalid_requests() {
        assert_eq!(
            dog("meta {\n  name: no request\n}\n").unwrap_err(),
            "no request block (get, post, put...) found"
        );
        assert_eq!(
            dog("post {\n  url: http://localhost\n  body: yaml\n}\n").unwrap_err(),
            "unknown body type `yaml`"
        );
    }
}
