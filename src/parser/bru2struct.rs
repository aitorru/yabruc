use super::bru::{Block, Content, Document, Value};
use crate::model::{
    ApiKeyPlacement, Auth, Body, Defaults, Environment, EnvironmentVariable, FileBody, KeyValue,
    MultipartField, MultipartValue, Param, ParamKind, Request, RequestKind, Scripts, Settings,
    Variable, Vars,
};

const METHODS: [&str; 9] = [
    "get", "post", "put", "delete", "patch", "options", "head", "connect", "trace",
];

pub fn document_to_request(document: &Document) -> Result<Request, String> {
    let meta = document.block("meta");
    let meta_value = |key| meta.and_then(|meta| meta.get(key));

    let kind = match meta_value("type").unwrap_or("http") {
        "graphql" => RequestKind::Graphql,
        kind @ ("grpc" | "ws" | "app") => {
            return Err(format!("`{kind}` requests are not supported"));
        }
        _ => RequestKind::Http,
    };

    let (method, request) = document
        .blocks
        .iter()
        .find_map(|block| match block.name.as_str() {
            "http" => Some((block.get("method").unwrap_or_default(), block)),
            name if METHODS.contains(&name) => Some((name, block)),
            _ => None,
        })
        .ok_or("no request block (get, post, put...) found")?;
    let method = reqwest::Method::from_bytes(method.to_uppercase().as_bytes())
        .map_err(|_| format!("invalid http method `{method}`"))?;

    let tags = meta
        .and_then(|meta| meta.pairs().iter().find(|pair| pair.key == "tags"))
        .map(|pair| match &pair.value {
            Value::List(tags) => tags.clone(),
            Value::Text(tag) if !tag.is_empty() => vec![tag.clone()],
            Value::Text(_) => vec![],
        })
        .unwrap_or_default();

    Ok(Request {
        name: meta_value("name").unwrap_or_default().to_string(),
        kind,
        seq: meta_value("seq")
            .and_then(|seq| seq.parse().ok())
            .unwrap_or(1.0),
        tags,
        method,
        url: request.get("url").unwrap_or_default().to_string(),
        params: params(document),
        headers: key_values(document.block("headers")),
        auth: auth(document, request.get("auth").unwrap_or("none")),
        body: body(document, request.get("body").unwrap_or("none"))?,
        vars: vars(document),
        assertions: key_values(document.block("assert")),
        scripts: scripts(document),
        settings: settings(document.block("settings")),
        docs: text(document, "docs"),
    })
}

/// Reads the request defaults of a `collection.bru` or `folder.bru` file.
pub fn document_to_defaults(document: &Document) -> Defaults {
    let meta = document.block("meta");
    let mode = document
        .block("auth")
        .and_then(|auth| auth.get("mode"))
        .unwrap_or("none");
    Defaults {
        name: meta.and_then(|meta| meta.get("name")).map(str::to_string),
        seq: meta
            .and_then(|meta| meta.get("seq"))
            .and_then(|seq| seq.parse().ok()),
        headers: key_values(document.block("headers")),
        auth: auth(document, mode),
        vars: vars(document),
        scripts: scripts(document),
        docs: text(document, "docs"),
    }
}

/// Reads an environment file, named after the file.
pub fn document_to_environment(name: &str, document: &Document) -> Environment {
    let mut variables: Vec<EnvironmentVariable> = key_values(document.block("vars"))
        .into_iter()
        .map(|pair| EnvironmentVariable {
            name: pair.name,
            value: pair.value,
            enabled: pair.enabled,
            secret: false,
        })
        .collect();
    if let Some(Content::Array(names)) = document.block("vars:secret").map(|block| &block.content) {
        variables.extend(names.iter().map(|name| EnvironmentVariable {
            name: name.strip_prefix('~').unwrap_or(name).to_string(),
            value: String::new(),
            enabled: !name.starts_with('~'),
            secret: true,
        }));
    }
    let extends = match document.block("extends").map(|block| &block.content) {
        Some(Content::Inline(name)) => vec![name.clone()],
        Some(Content::Array(names)) => names.clone(),
        _ => vec![],
    };
    Environment {
        name: name.to_string(),
        variables,
        extends,
    }
}

fn vars(document: &Document) -> Vars {
    Vars {
        pre_request: variables(document.block("vars:pre-request")),
        post_response: variables(document.block("vars:post-response")),
    }
}

fn scripts(document: &Document) -> Scripts {
    Scripts {
        pre_request: text(document, "script:pre-request"),
        post_response: text(document, "script:post-response"),
        tests: text(document, "tests"),
    }
}

fn text(document: &Document, name: &str) -> Option<String> {
    document
        .block(name)
        .and_then(Block::text)
        .map(str::to_string)
}

fn pair_text(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        Value::List(items) => items.join(","),
    }
}

fn key_values(block: Option<&Block>) -> Vec<KeyValue> {
    block
        .map(Block::pairs)
        .unwrap_or_default()
        .iter()
        .map(|pair| KeyValue {
            name: pair.key.clone(),
            value: pair_text(&pair.value),
            enabled: pair.enabled,
        })
        .collect()
}

fn params(document: &Document) -> Vec<Param> {
    // `query` is the old name of `params:query`
    [
        ("query", ParamKind::Query),
        ("params:query", ParamKind::Query),
        ("params:path", ParamKind::Path),
    ]
    .into_iter()
    .flat_map(|(name, kind)| {
        key_values(document.block(name))
            .into_iter()
            .map(move |pair| Param {
                name: pair.name,
                value: pair.value,
                enabled: pair.enabled,
                kind,
            })
    })
    .collect()
}

fn variables(block: Option<&Block>) -> Vec<Variable> {
    key_values(block)
        .into_iter()
        .map(|pair| match pair.name.strip_prefix('@') {
            Some(name) => Variable {
                name: name.to_string(),
                value: pair.value,
                enabled: pair.enabled,
                local: true,
            },
            None => Variable {
                name: pair.name,
                value: pair.value,
                enabled: pair.enabled,
                local: false,
            },
        })
        .collect()
}

fn auth(document: &Document, mode: &str) -> Auth {
    let block = document.block(&format!("auth:{mode}"));
    let value = |key| {
        block
            .and_then(|block| block.pairs().iter().find(|pair| pair.key == key))
            .map(|pair| pair_text(&pair.value))
            .unwrap_or_default()
    };
    match mode {
        "none" => Auth::None,
        "inherit" => Auth::Inherit,
        "basic" => Auth::Basic {
            username: value("username"),
            password: value("password"),
        },
        "bearer" => Auth::Bearer {
            token: value("token"),
        },
        "digest" => Auth::Digest {
            username: value("username"),
            password: value("password"),
        },
        "apikey" => Auth::ApiKey {
            key: value("key"),
            value: value("value"),
            placement: match value("placement").as_str() {
                "queryparams" => ApiKeyPlacement::QueryParams,
                _ => ApiKeyPlacement::Header,
            },
        },
        other => Auth::Unsupported(other.to_string()),
    }
}

fn body(document: &Document, mode: &str) -> Result<Body, String> {
    let text = |name| text(document, name).unwrap_or_default();
    Ok(match mode {
        "none" => Body::None,
        // `body` is the old name of `body:json`
        "json" => Body::Json(text(
            document.block("body:json").map_or("body", |_| "body:json"),
        )),
        "text" => Body::Text(text("body:text")),
        "xml" => Body::Xml(text("body:xml")),
        "sparql" => Body::Sparql(text("body:sparql")),
        "graphql" => Body::Graphql {
            query: text("body:graphql"),
            variables: text("body:graphql:vars"),
        },
        "formUrlEncoded" => {
            Body::FormUrlEncoded(key_values(document.block("body:form-urlencoded")))
        }
        "multipartForm" => Body::MultipartForm(
            key_values(document.block("body:multipart-form"))
                .into_iter()
                .map(multipart_field)
                .collect(),
        ),
        "file" => Body::File(
            key_values(document.block("body:file"))
                .into_iter()
                .filter_map(|pair| {
                    let (value, content_type) = split_content_type(&pair.value);
                    Some(FileBody {
                        path: file_reference(value)?.to_string(),
                        content_type,
                        selected: pair.enabled,
                    })
                })
                .collect(),
        ),
        _ => return Err(format!("unknown body type `{mode}`")),
    })
}

fn multipart_field(pair: KeyValue) -> MultipartField {
    let (value, content_type) = split_content_type(&pair.value);
    let value = match file_reference(value) {
        Some(files) => MultipartValue::Files(
            files
                .split('|')
                .filter(|file| !file.is_empty())
                .map(str::to_string)
                .collect(),
        ),
        None => MultipartValue::Text(value.to_string()),
    };
    MultipartField {
        name: pair.name,
        value,
        content_type,
        enabled: pair.enabled,
    }
}

/// Splits `value @contentType(application/json)` into the value and the content type.
fn split_content_type(value: &str) -> (&str, Option<String>) {
    if let Some(start) = value.find("@contentType(")
        && let Some(content_type) = value[start..]
            .trim_end()
            .strip_prefix("@contentType(")
            .and_then(|rest| rest.strip_suffix(')'))
    {
        return (
            value[..start].trim_end(),
            Some(content_type.trim().to_string()),
        );
    }
    (value, None)
}

/// Returns the path of `@file(path)`.
fn file_reference(value: &str) -> Option<&str> {
    value.strip_prefix("@file(")?.strip_suffix(')')
}

fn settings(block: Option<&Block>) -> Settings {
    let value = |key| block.and_then(|block| block.get(key));
    let boolean = |key| value(key).map(|value| value == "true");
    Settings {
        encode_url: boolean("encodeUrl"),
        timeout: value("timeout").and_then(|timeout| timeout.parse().ok()),
        follow_redirects: boolean("followRedirects"),
        max_redirects: value("maxRedirects").and_then(|max| max.parse().ok()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::bru;

    fn request(source: &str) -> Result<Request, String> {
        document_to_request(&bru::parse(source).unwrap())
    }

    fn key_value(name: &str, value: &str, enabled: bool) -> KeyValue {
        KeyValue {
            name: name.into(),
            value: value.into(),
            enabled,
        }
    }

    fn variable(name: &str, value: &str, enabled: bool, local: bool) -> Variable {
        Variable {
            name: name.into(),
            value: value.into(),
            enabled,
            local,
        }
    }

    #[test]
    fn maps_official_request_fixture() {
        let request = request(include_str!("../../test/fixtures/bru/request.bru")).unwrap();
        assert_eq!(request.name, "Send Bulk SMS");
        assert_eq!(request.kind, RequestKind::Http);
        assert_eq!(request.seq, 1.0);
        assert_eq!(request.tags, ["foo", "bar"]);
        assert_eq!(request.method, reqwest::Method::GET);
        assert_eq!(request.url, "https://api.textlocal.in/send/:id");

        assert_eq!(request.params.len(), 9);
        assert_eq!(
            request.params[6],
            Param {
                name: "disabled:colon:parameter".into(),
                value: "is allowed".into(),
                enabled: false,
                kind: ParamKind::Query,
            }
        );
        assert_eq!(
            request.params[8],
            Param {
                name: "id".into(),
                value: "123".into(),
                enabled: true,
                kind: ParamKind::Path,
            }
        );

        assert_eq!(request.headers.len(), 8);
        assert_eq!(
            request.headers[0],
            key_value("content-type", "application/json", true)
        );
        assert_eq!(
            request.headers[7],
            key_value("transaction-id", "{{transactionId}}", false)
        );

        assert_eq!(
            request.auth,
            Auth::Bearer {
                token: "123".into()
            }
        );
        assert_eq!(
            request.body,
            Body::Json("{\n  \"hello\": \"world\"\n}".into())
        );

        assert_eq!(
            request.vars,
            Vars {
                pre_request: vec![
                    variable("departingDate", "2020-01-01", true, false),
                    variable("returningDate", "2020-01-02", false, false),
                ],
                post_response: vec![
                    variable("token", "$res.body.token", true, false),
                    variable("orderNumber", "$res.body.orderNumber", true, true),
                    variable("petId", "$res.body.id", false, false),
                    variable("transactionId", "$res.body.transactionId", false, true),
                ],
            }
        );
        assert_eq!(
            request.assertions,
            [
                key_value("$res.status", "200", true),
                key_value("$res.body.message", "success", false),
            ]
        );
        assert_eq!(
            request.scripts,
            Scripts {
                pre_request: Some("const foo = 'bar';".into()),
                post_response: None,
                tests: Some(
                    "function onResponse(request, response) {\n  expect(response.status).to.equal(200);\n}"
                        .into()
                ),
            }
        );
        assert_eq!(
            request.docs.as_deref(),
            Some("This request needs auth token to be set in the headers.")
        );
        assert_eq!(request.settings, Settings::default());
    }

    #[test]
    fn maps_every_body_of_the_official_fixture() {
        let source = include_str!("../../test/fixtures/bru/request.bru");
        let body = |mode: &str| {
            request(&source.replace("body: json", &format!("body: {mode}")))
                .unwrap()
                .body
        };

        assert_eq!(body("none"), Body::None);
        assert_eq!(body("text"), Body::Text("This is a text body".into()));
        assert_eq!(
            body("xml"),
            Body::Xml("<xml>\n  <name>John</name>\n  <age>30</age>\n</xml>".into())
        );
        assert!(matches!(body("sparql"), Body::Sparql(query) if query.starts_with("SELECT *")));
        assert_eq!(
            body("graphql"),
            Body::Graphql {
                query: "{\n  launchesPast {\n    launch_site {\n      site_name\n    }\n    launch_success\n  }\n}".into(),
                variables: "{\n  \"limit\": 5\n}".into(),
            }
        );
        let Body::FormUrlEncoded(form) = body("formUrlEncoded") else {
            panic!("expected a form");
        };
        assert_eq!(form[1], key_value("numbers", "+91998877665", true));
        assert_eq!(
            form[7],
            key_value("disabled colon:parameter", "is allowed", false)
        );

        let Body::MultipartForm(multipart) = body("multipartForm") else {
            panic!("expected a multipart form");
        };
        assert_eq!(multipart.len(), 8);
        assert_eq!(
            multipart[0],
            MultipartField {
                name: "apikey".into(),
                value: MultipartValue::Text("secret".into()),
                content_type: None,
                enabled: true,
            }
        );
        assert_eq!(
            body("file"),
            Body::File(vec![
                FileBody {
                    path: "path/to/file.json".into(),
                    content_type: Some("application/json".into()),
                    selected: true,
                },
                FileBody {
                    path: "path/to/file.json".into(),
                    content_type: Some("application/json".into()),
                    selected: true,
                },
                FileBody {
                    path: "path/to/file2.json".into(),
                    content_type: Some("application/json".into()),
                    selected: false,
                },
            ])
        );
        assert_eq!(
            request(&source.replace("body: json", "body: yaml")).unwrap_err(),
            "unknown body type `yaml`"
        );
    }

    #[test]
    fn maps_every_auth_of_the_official_fixture() {
        let source = include_str!("../../test/fixtures/bru/request.bru");
        let auth = |mode: &str| {
            request(&source.replace("auth: bearer", &format!("auth: {mode}")))
                .unwrap()
                .auth
        };

        assert_eq!(auth("none"), Auth::None);
        assert_eq!(auth("inherit"), Auth::Inherit);
        assert_eq!(
            auth("basic"),
            Auth::Basic {
                username: "john".into(),
                password: "secret".into(),
            }
        );
        assert_eq!(
            auth("digest"),
            Auth::Digest {
                username: "john".into(),
                password: "secret".into(),
            }
        );
        assert_eq!(auth("oauth2"), Auth::Unsupported("oauth2".into()));
        assert_eq!(auth("awsv4"), Auth::Unsupported("awsv4".into()));
    }

    #[test]
    fn maps_multipart_files_api_keys_and_settings() {
        let request = request(
            "meta {\n  name: upload\n  type: graphql\n  seq: 4\n}\n\nhttp {\n  method: purge\n  url: http://localhost\n  body: multipartForm\n  auth: apikey\n}\n\nbody:multipart-form {\n  files: @file(a.png|b.png)\n  data: '''\n    {\"a\": 1}\n  ''' @contentType(application/json)\n}\n\nauth:apikey {\n  key: token\n  value: secret\n  placement: queryparams\n}\n\nsettings {\n  encodeUrl: true\n  timeout: 5000\n  followRedirects: false\n  maxRedirects: 3\n}\n",
        )
        .unwrap();
        assert_eq!(request.kind, RequestKind::Graphql);
        assert_eq!(request.seq, 4.0);
        assert_eq!(request.method.as_str(), "PURGE");
        assert_eq!(
            request.body,
            Body::MultipartForm(vec![
                MultipartField {
                    name: "files".into(),
                    value: MultipartValue::Files(vec!["a.png".into(), "b.png".into()]),
                    content_type: None,
                    enabled: true,
                },
                MultipartField {
                    name: "data".into(),
                    value: MultipartValue::Text("{\"a\": 1}".into()),
                    content_type: Some("application/json".into()),
                    enabled: true,
                },
            ])
        );
        assert_eq!(
            request.auth,
            Auth::ApiKey {
                key: "token".into(),
                value: "secret".into(),
                placement: ApiKeyPlacement::QueryParams,
            }
        );
        assert_eq!(
            request.settings,
            Settings {
                encode_url: Some(true),
                timeout: Some(5000),
                follow_redirects: Some(false),
                max_redirects: Some(3),
            }
        );
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
            let request = request(source).unwrap();
            assert_eq!(request.method, method);
            assert_eq!(request.url, "http://localhost:1234");
        }
    }

    #[test]
    fn maps_official_collection_fixture_to_defaults() {
        let defaults = document_to_defaults(
            &bru::parse(include_str!("../../test/fixtures/bru/collection.bru")).unwrap(),
        );
        assert_eq!(defaults.name, None);
        assert_eq!(defaults.headers.len(), 3);
        assert_eq!(
            defaults.headers[2],
            key_value("transaction-id", "{{transactionId}}", false)
        );
        // `auth { mode: none }` wins over the `auth:*` blocks
        assert_eq!(defaults.auth, Auth::None);
        assert_eq!(defaults.vars.pre_request.len(), 2);
        assert_eq!(
            defaults.scripts.post_response.as_deref(),
            Some("console.log(\"In Collection post Request Script\");")
        );

        let defaults = document_to_defaults(
            &bru::parse("meta {\n  name: Users\n  seq: 2\n}\n\nauth {\n  mode: bearer\n}\n\nauth:bearer {\n  token: abc\n}\n")
                .unwrap(),
        );
        assert_eq!(defaults.name.as_deref(), Some("Users"));
        assert_eq!(defaults.seq, Some(2.0));
        assert_eq!(
            defaults.auth,
            Auth::Bearer {
                token: "abc".into()
            }
        );
    }

    #[test]
    fn maps_environments() {
        let environment = document_to_environment(
            "local",
            &bru::parse("vars {\n  host: http://localhost\n  ~port: 80\n}\nvars:secret [\n  token,\n  ~password\n]\ncolor: #ff0000\nextends: base\n")
                .unwrap(),
        );
        assert_eq!(environment.name, "local");
        assert_eq!(environment.extends, ["base"]);
        assert_eq!(
            environment.variables,
            [
                EnvironmentVariable {
                    name: "host".into(),
                    value: "http://localhost".into(),
                    enabled: true,
                    secret: false,
                },
                EnvironmentVariable {
                    name: "port".into(),
                    value: "80".into(),
                    enabled: false,
                    secret: false,
                },
                EnvironmentVariable {
                    name: "token".into(),
                    value: String::new(),
                    enabled: true,
                    secret: true,
                },
                EnvironmentVariable {
                    name: "password".into(),
                    value: String::new(),
                    enabled: false,
                    secret: true,
                },
            ]
        );
    }

    #[test]
    fn reports_invalid_requests() {
        assert_eq!(
            request("meta {\n  name: no request\n}\n").unwrap_err(),
            "no request block (get, post, put...) found"
        );
        assert_eq!(
            request("meta {\n  type: grpc\n}\n\ngrpc {\n  url: localhost\n}\n").unwrap_err(),
            "`grpc` requests are not supported"
        );
        assert_eq!(
            request("http {\n  method: not valid\n}\n").unwrap_err(),
            "invalid http method `not valid`"
        );
    }
}
