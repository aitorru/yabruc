//! Prints, as JSON, what the real parser of yabruc returns for every input of a JSON array.

#[path = "../../../../src/model.rs"]
#[allow(dead_code)]
mod model;
#[allow(dead_code)]
mod parser;

use model::*;
use parser::bru::{Content, Value as BruValue};
use serde_json::{Value, json};

fn num(n: f64) -> Value {
    if n.is_nan() { json!("NaN") } else if n.is_infinite() { json!(if n > 0.0 { "Infinity" } else { "-Infinity" }) } else { json!(n) }
}
fn kv(list: &[KeyValue]) -> Value {
    Value::Array(list.iter().map(|k| json!({"name": k.name, "value": k.value, "enabled": k.enabled})).collect())
}
fn vars(v: &Vars) -> Value {
    let f = |l: &[Variable]| Value::Array(l.iter().map(|x| json!({"name": x.name, "value": x.value, "enabled": x.enabled, "local": x.local})).collect());
    json!({"pre_request": f(&v.pre_request), "post_response": f(&v.post_response)})
}
fn scripts(s: &Scripts) -> Value {
    json!({"pre_request": s.pre_request, "post_response": s.post_response, "tests": s.tests})
}
fn auth(a: &Auth) -> Value {
    match a {
        Auth::None => json!({"type": "None"}),
        Auth::Inherit => json!({"type": "Inherit"}),
        Auth::Basic { username, password } => json!({"type": "Basic", "username": username, "password": password}),
        Auth::Bearer { token } => json!({"type": "Bearer", "token": token}),
        Auth::Digest { username, password } => json!({"type": "Digest", "username": username, "password": password}),
        Auth::ApiKey { key, value, placement } => json!({"type": "ApiKey", "key": key, "value": value, "placement": match placement { ApiKeyPlacement::Header => "Header", ApiKeyPlacement::QueryParams => "QueryParams" }}),
        Auth::Unsupported(mode) => json!({"type": "Unsupported", "mode": mode}),
    }
}
fn body(b: &Body) -> Value {
    match b {
        Body::None => json!({"type": "None"}),
        Body::Json(v) => json!({"type": "Json", "value": v}),
        Body::Text(v) => json!({"type": "Text", "value": v}),
        Body::Xml(v) => json!({"type": "Xml", "value": v}),
        Body::Sparql(v) => json!({"type": "Sparql", "value": v}),
        Body::Graphql { query, variables } => json!({"type": "Graphql", "query": query, "variables": variables}),
        Body::FormUrlEncoded(v) => json!({"type": "FormUrlEncoded", "value": kv(v)}),
        Body::MultipartForm(v) => json!({"type": "MultipartForm", "value": v.iter().map(|f| json!({
            "name": f.name,
            "value": match &f.value { MultipartValue::Text(t) => json!({"type": "Text", "value": t}), MultipartValue::Files(fs) => json!({"type": "Files", "value": fs}) },
            "content_type": f.content_type, "enabled": f.enabled})).collect::<Vec<_>>()}),
        Body::File(v) => json!({"type": "File", "value": v.iter().map(|f| json!({"path": f.path, "content_type": f.content_type, "selected": f.selected})).collect::<Vec<_>>()}),
    }
}
fn document(d: &parser::bru::Document) -> Value {
    Value::Array(d.blocks.iter().map(|b| json!({
        "name": b.name, "line": b.line,
        "content": match &b.content {
            Content::Dictionary(pairs) => json!({"type": "Dictionary", "value": pairs.iter().map(|p| json!({
                "key": p.key, "enabled": p.enabled, "line": p.line,
                "value": match &p.value { BruValue::Text(t) => json!({"type": "Text", "value": t}), BruValue::List(l) => json!({"type": "List", "value": l}) },
                "annotations": p.annotations.iter().map(|a| json!({"name": a.name, "value": a.value})).collect::<Vec<_>>(),
            })).collect::<Vec<_>>()}),
            Content::Text(t) => json!({"type": "Text", "value": t}),
            Content::Array(a) => json!({"type": "Array", "value": a}),
            Content::Inline(i) => json!({"type": "Inline", "value": i}),
        }
    })).collect())
}

fn main() {
    let inputs: Vec<String> = serde_json::from_str(&std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap()).unwrap();
    let out: Vec<Value> = inputs.iter().map(|source| match parser::bru::parse(source) {
        Err(e) => json!({"error": e.to_string()}),
        Ok(d) => {
            let request = match parser::bru2struct::document_to_request(&d) {
                Ok(r) => json!({
                    "name": r.name, "kind": match r.kind { RequestKind::Http => "Http", RequestKind::Graphql => "Graphql" },
                    "seq": num(r.seq), "tags": r.tags, "method": r.method.as_str(), "url": r.url,
                    "params": r.params.iter().map(|p| json!({"name": p.name, "value": p.value, "enabled": p.enabled, "kind": match p.kind { ParamKind::Query => "Query", ParamKind::Path => "Path" }})).collect::<Vec<_>>(),
                    "headers": kv(&r.headers), "auth": auth(&r.auth), "body": body(&r.body), "vars": vars(&r.vars),
                    "assertions": kv(&r.assertions), "scripts": scripts(&r.scripts),
                    "settings": {"encode_url": r.settings.encode_url, "timeout": r.settings.timeout, "follow_redirects": r.settings.follow_redirects, "max_redirects": r.settings.max_redirects},
                    "docs": r.docs,
                }),
                Err(e) => json!({"error": e}),
            };
            let defaults = parser::bru2struct::document_to_defaults(&d);
            let env = parser::bru2struct::document_to_environment("env", &d);
            json!({
                "document": document(&d),
                "request": request,
                "defaults": {"name": defaults.name, "seq": defaults.seq.map(num), "headers": kv(&defaults.headers), "auth": auth(&defaults.auth), "vars": vars(&defaults.vars), "scripts": scripts(&defaults.scripts), "docs": defaults.docs},
                "environment": {"name": env.name, "variables": env.variables.iter().map(|v| json!({"name": v.name, "value": v.value, "enabled": v.enabled, "secret": v.secret})).collect::<Vec<_>>(), "extends": env.extends},
            })
        }
    }).collect();
    println!("{}", serde_json::to_string(&out).unwrap());
}
