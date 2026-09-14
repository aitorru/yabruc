//! Reads requests in the OpenCollection YAML format, used by default since Bruno v3.1.
//!
//! The mapping follows the official implementation in `@usebruno/filestore`:
//! <https://github.com/usebruno/bruno/tree/main/packages/bruno-filestore/src/formats/yml>

use serde_yaml_ng::Value;

use crate::model::{
    ApiKeyPlacement, Auth, Body, FileBody, KeyValue, MultipartField, MultipartValue, Param,
    ParamKind, Request, RequestKind, Scripts, Settings, Variable, Vars,
};

pub fn parse(source: &str) -> Result<Request, String> {
    let root: Value = serde_yaml_ng::from_str(source).map_err(|error| error.to_string())?;
    yaml_to_request(&root)
}

fn yaml_to_request(root: &Value) -> Result<Request, String> {
    let info = &root["info"];
    let (kind, details, default_method) = match text(&info["type"]).as_str() {
        "http" => (RequestKind::Http, &root["http"], "GET"),
        "graphql" => (RequestKind::Graphql, &root["graphql"], "POST"),
        "" => return Err("missing `info.type`".to_string()),
        kind @ ("grpc" | "websocket") => {
            return Err(format!("`{kind}` requests are not supported"));
        }
        kind => return Err(format!("`{kind}` items are not requests")),
    };
    let runtime = &root["runtime"];

    let method = match text(&details["method"]) {
        method if method.is_empty() => default_method.to_string(),
        method => method,
    };
    let method = reqwest::Method::from_bytes(method.to_uppercase().as_bytes())
        .map_err(|_| format!("invalid http method `{method}`"))?;

    let body = match kind {
        RequestKind::Http => body(&details["body"])?,
        RequestKind::Graphql => Body::Graphql {
            query: text(&details["body"]["query"]),
            variables: text(&details["body"]["variables"]),
        },
    };

    Ok(Request {
        name: match text(&info["name"]) {
            name if name.is_empty() => "Untitled Request".to_string(),
            name => name,
        },
        kind,
        seq: info["seq"].as_f64().unwrap_or(1.0),
        tags: list(&info["tags"]).iter().map(text).collect(),
        method,
        url: text(&details["url"]),
        params: list(&details["params"])
            .iter()
            .map(|param| Param {
                name: text(&param["name"]),
                value: text(&param["value"]),
                enabled: enabled(param),
                kind: match param["type"].as_str() {
                    Some("path") => ParamKind::Path,
                    _ => ParamKind::Query,
                },
            })
            .collect(),
        headers: key_values(&details["headers"]),
        auth: auth(&details["auth"]),
        body,
        vars: Vars {
            pre_request: list(&runtime["variables"])
                .iter()
                .map(|variable| Variable {
                    name: text(&variable["name"]),
                    value: variable_value(&variable["value"]),
                    enabled: enabled(variable),
                    local: false,
                })
                .collect(),
            post_response: list(&runtime["actions"])
                .iter()
                .filter(|action| {
                    action["type"].as_str() == Some("set-variable")
                        && action["phase"].as_str() == Some("after-response")
                })
                .map(|action| Variable {
                    name: text(&action["variable"]["name"]),
                    value: text(&action["selector"]["expression"]),
                    enabled: enabled(action),
                    local: false,
                })
                .collect(),
        },
        assertions: list(&runtime["assertions"])
            .iter()
            .map(|assertion| KeyValue {
                name: text(&assertion["expression"]),
                value: match (text(&assertion["operator"]), &assertion["value"]) {
                    (operator, Value::Null) => operator,
                    (operator, value) => format!("{operator} {}", text(value)),
                },
                enabled: enabled(assertion),
            })
            .collect(),
        scripts: scripts(&runtime["scripts"]),
        settings: settings(&root["settings"]),
        docs: Some(description(&root["docs"])).filter(|docs| !docs.is_empty()),
    })
}

/// Converts a scalar to a string, like `ensureString` in Bruno.
fn text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        _ => String::new(),
    }
}

/// Descriptions and docs can be a string or a `{ content }` object.
fn description(value: &Value) -> String {
    match value {
        Value::Mapping(_) => text(&value["content"]),
        value => text(value),
    }
}

fn list(value: &Value) -> &[Value] {
    value.as_sequence().map(Vec::as_slice).unwrap_or_default()
}

fn enabled(value: &Value) -> bool {
    value["disabled"].as_bool() != Some(true)
}

fn key_values(value: &Value) -> Vec<KeyValue> {
    list(value)
        .iter()
        .map(|entry| KeyValue {
            name: text(&entry["name"]),
            value: text(&entry["value"]),
            enabled: enabled(entry),
        })
        .collect()
}

/// Variables can be a scalar or a typed value like `{ type: number, data: 42 }`.
fn variable_value(value: &Value) -> String {
    match value {
        Value::Mapping(_) => text(&value["data"]),
        value => text(value),
    }
}

fn auth(value: &Value) -> Auth {
    if value.as_str() == Some("inherit") {
        return Auth::Inherit;
    }
    let field = |key| text(&value[key]);
    match value["type"].as_str() {
        None => Auth::None,
        Some("basic") => Auth::Basic {
            username: field("username"),
            password: field("password"),
        },
        Some("bearer") => Auth::Bearer {
            token: field("token"),
        },
        Some("digest") => Auth::Digest {
            username: field("username"),
            password: field("password"),
        },
        Some("apikey") => Auth::ApiKey {
            key: field("key"),
            value: field("value"),
            placement: match value["placement"].as_str() {
                Some("query") => ApiKeyPlacement::QueryParams,
                _ => ApiKeyPlacement::Header,
            },
        },
        Some(other) => Auth::Unsupported(other.to_string()),
    }
}

fn body(value: &Value) -> Result<Body, String> {
    if value.is_null() {
        return Ok(Body::None);
    }
    let data = || text(&value["data"]);
    Ok(match text(&value["type"]).as_str() {
        "json" => Body::Json(data()),
        "text" => Body::Text(data()),
        "xml" => Body::Xml(data()),
        "sparql" => Body::Sparql(data()),
        "form-urlencoded" => Body::FormUrlEncoded(key_values(&value["data"])),
        "multipart-form" => Body::MultipartForm(
            list(&value["data"])
                .iter()
                .map(|entry| MultipartField {
                    name: text(&entry["name"]),
                    value: match (entry["type"].as_str(), &entry["value"]) {
                        (Some("file"), Value::Sequence(files)) => {
                            MultipartValue::Files(files.iter().map(text).collect())
                        }
                        (Some("file"), file) => MultipartValue::Files(
                            vec![text(file)]
                                .into_iter()
                                .filter(|file| !file.is_empty())
                                .collect(),
                        ),
                        (_, value) => MultipartValue::Text(text(value)),
                    },
                    content_type: optional_text(&entry["contentType"]),
                    enabled: enabled(entry),
                })
                .collect(),
        ),
        "file" => Body::File(
            list(&value["data"])
                .iter()
                .map(|file| FileBody {
                    path: text(&file["filePath"]),
                    content_type: optional_text(&file["contentType"]),
                    selected: file["selected"].as_bool().unwrap_or(false),
                })
                .collect(),
        ),
        mode => return Err(format!("unknown body type `{mode}`")),
    })
}

fn optional_text(value: &Value) -> Option<String> {
    Some(text(value)).filter(|text| !text.is_empty())
}

fn scripts(value: &Value) -> Scripts {
    let code = |kind| {
        list(value)
            .iter()
            .find(|script| script["type"].as_str() == Some(kind))
            .map(|script| text(&script["code"]))
            .filter(|code| !code.is_empty())
    };
    Scripts {
        pre_request: code("before-request"),
        post_response: code("after-response"),
        tests: code("tests"),
    }
}

fn settings(value: &Value) -> Settings {
    Settings {
        encode_url: value["encodeUrl"].as_bool(),
        timeout: value["timeout"].as_u64(),
        follow_redirects: value["followRedirects"].as_bool(),
        max_redirects: value["maxRedirects"]
            .as_u64()
            .and_then(|max| max.try_into().ok()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{bru, bru2struct};

    #[test]
    fn same_request_in_bru_and_yml_is_equal() {
        let from_bru = bru2struct::document_to_request(
            &bru::parse(include_str!("../../test/fixtures/parity/request.bru")).unwrap(),
        )
        .unwrap();
        let from_yml = parse(include_str!("../../test/fixtures/parity/request.yml")).unwrap();
        assert_eq!(from_bru, from_yml);
        // Make sure the fixture is not empty
        assert_eq!(from_yml.params.len(), 3);
        assert_eq!(from_yml.assertions[1].value, "isJson");
        assert_eq!(from_yml.vars.post_response[0].value, "res.body.id");
    }

    #[test]
    fn parses_official_bruno_cli_fixtures() {
        let request = parse(include_str!("../../test/fixtures/yml/get-users.yml")).unwrap();
        assert_eq!(request.name, "Get Users");
        assert_eq!(request.method, reqwest::Method::GET);
        assert_eq!(request.url, "https://api.example.com/users");
        assert_eq!(request.auth, Auth::None);
        assert_eq!(request.body, Body::None);

        let request = parse(include_str!("../../test/fixtures/yml/request-level.yml")).unwrap();
        assert_eq!(
            request.auth,
            Auth::Digest {
                username: "user".into(),
                password: "{{digestPw}}".into(),
            }
        );
        assert!(
            request
                .scripts
                .tests
                .unwrap()
                .starts_with("test('digest auth")
        );
    }

    #[test]
    fn parses_bodies() {
        let body = |yaml: &str| {
            parse(&format!(
                "info:\n  type: http\nhttp:\n  url: http://localhost\n  body:\n{yaml}"
            ))
            .map(|request| request.body)
        };
        assert_eq!(
            body("    type: xml\n    data: <a/>\n"),
            Ok(Body::Xml("<a/>".into()))
        );
        assert_eq!(
            body(
                "    type: form-urlencoded\n    data:\n      - name: a\n        value: 1\n      - name: b\n        value: two\n        disabled: true\n"
            ),
            Ok(Body::FormUrlEncoded(vec![
                KeyValue {
                    name: "a".into(),
                    value: "1".into(),
                    enabled: true
                },
                KeyValue {
                    name: "b".into(),
                    value: "two".into(),
                    enabled: false
                },
            ]))
        );
        assert_eq!(
            body(
                "    type: multipart-form\n    data:\n      - name: files\n        type: file\n        value:\n          - a.png\n          - b.png\n      - name: meta\n        type: text\n        value: '{}'\n        contentType: application/json\n"
            ),
            Ok(Body::MultipartForm(vec![
                MultipartField {
                    name: "files".into(),
                    value: MultipartValue::Files(vec!["a.png".into(), "b.png".into()]),
                    content_type: None,
                    enabled: true,
                },
                MultipartField {
                    name: "meta".into(),
                    value: MultipartValue::Text("{}".into()),
                    content_type: Some("application/json".into()),
                    enabled: true,
                },
            ]))
        );
        assert_eq!(
            body(
                "    type: file\n    data:\n      - filePath: a.json\n        contentType: application/json\n        selected: true\n"
            ),
            Ok(Body::File(vec![FileBody {
                path: "a.json".into(),
                content_type: Some("application/json".into()),
                selected: true,
            }]))
        );
        assert_eq!(
            body("    type: yaml\n"),
            Err("unknown body type `yaml`".into())
        );
    }

    #[test]
    fn parses_graphql_requests() {
        let request = parse("info:\n  name: g\n  type: graphql\ngraphql:\n  url: http://localhost/graphql\n  auth: inherit\n  body:\n    query: '{ users { id } }'\n    variables: '{\"limit\": 5}'\n").unwrap();
        assert_eq!(request.kind, RequestKind::Graphql);
        assert_eq!(request.method, reqwest::Method::POST);
        assert_eq!(request.auth, Auth::Inherit);
        assert_eq!(
            request.body,
            Body::Graphql {
                query: "{ users { id } }".into(),
                variables: "{\"limit\": 5}".into(),
            }
        );
    }

    #[test]
    fn parses_auth_and_typed_variables() {
        let request = parse("info:\n  type: http\nhttp:\n  url: http://localhost\n  auth:\n    type: apikey\n    key: token\n    value: 123\n    placement: query\nruntime:\n  variables:\n    - name: count\n      value:\n        type: number\n        data: 42\n    - name: plain\n      value: true\n").unwrap();
        assert_eq!(request.name, "Untitled Request");
        assert_eq!(
            request.auth,
            Auth::ApiKey {
                key: "token".into(),
                value: "123".into(),
                placement: ApiKeyPlacement::QueryParams,
            }
        );
        assert_eq!(request.vars.pre_request[0].value, "42");
        assert_eq!(request.vars.pre_request[1].value, "true");

        let auth = |auth: &str| {
            parse(&format!(
                "info:\n  type: http\nhttp:\n  url: http://localhost\n  auth:\n    type: {auth}\n"
            ))
            .unwrap()
            .auth
        };
        assert_eq!(
            auth("bearer"),
            Auth::Bearer {
                token: String::new()
            }
        );
        assert_eq!(auth("oauth2"), Auth::Unsupported("oauth2".into()));
    }

    #[test]
    fn reports_invalid_items() {
        assert_eq!(
            parse("info:\n  name: x\n").unwrap_err(),
            "missing `info.type`"
        );
        assert_eq!(
            parse("info:\n  name: Users\n  type: folder\n").unwrap_err(),
            "`folder` items are not requests"
        );
        assert_eq!(
            parse("info:\n  type: grpc\ngrpc:\n  url: localhost:50051\n").unwrap_err(),
            "`grpc` requests are not supported"
        );
        assert_eq!(
            parse("info: [").unwrap_err(),
            "did not find expected node content at line 2 column 1, while parsing a flow node"
        );
    }
}
