//! Format independent representation of a Bruno request.
//!
//! Both `.bru` and OpenCollection `.yml` files are converted to these types, which mirror the
//! request item used internally by Bruno.

#[derive(Debug, PartialEq, Clone)]
pub struct Request {
    pub name: String,
    pub kind: RequestKind,
    /// Position of the request inside its folder.
    pub seq: f64,
    pub tags: Vec<String>,
    pub method: reqwest::Method,
    pub url: String,
    pub params: Vec<Param>,
    pub headers: Vec<KeyValue>,
    pub auth: Auth,
    pub body: Body,
    pub vars: Vars,
    pub assertions: Vec<KeyValue>,
    pub scripts: Scripts,
    pub settings: Settings,
    pub docs: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RequestKind {
    Http,
    Graphql,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct KeyValue {
    pub name: String,
    pub value: String,
    pub enabled: bool,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Param {
    pub name: String,
    pub value: String,
    pub enabled: bool,
    pub kind: ParamKind,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ParamKind {
    Query,
    Path,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Auth {
    None,
    /// Uses the auth of the parent folder or collection.
    Inherit,
    Basic {
        username: String,
        password: String,
    },
    Bearer {
        token: String,
    },
    Digest {
        username: String,
        password: String,
    },
    ApiKey {
        key: String,
        value: String,
        placement: ApiKeyPlacement,
    },
    /// Auth modes that yabruc does not support yet, like `oauth2` or `awsv4`.
    Unsupported(String),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ApiKeyPlacement {
    Header,
    QueryParams,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Body {
    None,
    Json(String),
    Text(String),
    Xml(String),
    Sparql(String),
    Graphql { query: String, variables: String },
    FormUrlEncoded(Vec<KeyValue>),
    MultipartForm(Vec<MultipartField>),
    File(Vec<FileBody>),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct MultipartField {
    pub name: String,
    pub value: MultipartValue,
    pub content_type: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum MultipartValue {
    Text(String),
    Files(Vec<String>),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct FileBody {
    pub path: String,
    pub content_type: Option<String>,
    /// Only one file can be selected to be sent.
    pub selected: bool,
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Vars {
    pub pre_request: Vec<Variable>,
    /// The value is an expression evaluated against the response, like `res.body.token`.
    pub post_response: Vec<Variable>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Variable {
    pub name: String,
    pub value: String,
    pub enabled: bool,
    /// Local variables only exist during the request.
    pub local: bool,
}

/// JavaScript code. yabruc does not run it, but it is kept to warn about it.
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Scripts {
    pub pre_request: Option<String>,
    pub post_response: Option<String>,
    pub tests: Option<String>,
}

/// Unset values use the defaults of the collection.
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Settings {
    pub encode_url: Option<bool>,
    /// Timeout in milliseconds.
    pub timeout: Option<u64>,
    pub follow_redirects: Option<bool>,
    pub max_redirects: Option<u32>,
}

/// Request defaults defined by a collection (`collection.bru`, `opencollection.yml`) or a folder
/// (`folder.bru`, `folder.yml`).
#[derive(Debug, PartialEq, Clone)]
pub struct Defaults {
    pub name: Option<String>,
    /// Position of the folder inside its parent.
    pub seq: Option<f64>,
    pub headers: Vec<KeyValue>,
    pub auth: Auth,
    pub vars: Vars,
    pub scripts: Scripts,
    pub docs: Option<String>,
}

impl Default for Defaults {
    fn default() -> Self {
        Defaults {
            name: None,
            seq: None,
            headers: vec![],
            auth: Auth::None,
            vars: Vars::default(),
            scripts: Scripts::default(),
            docs: None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Environment {
    pub name: String,
    pub variables: Vec<EnvironmentVariable>,
    /// Names of the environments this one inherits variables from.
    pub extends: Vec<String>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct EnvironmentVariable {
    pub name: String,
    pub value: String,
    pub enabled: bool,
    /// Secret values are not stored in the file, they must be given when running.
    pub secret: bool,
}
