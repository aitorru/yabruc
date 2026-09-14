//! Parser for the `.bru` file syntax.
//!
//! A bru file is a list of blocks. There are four kinds of blocks:
//!
//! 1. Dictionary blocks, with key value pairs.
//!    ```text
//!    headers {
//!      content-type: application/json
//!    }
//!    ```
//! 2. Text blocks, with free text.
//!    ```text
//!    body:json {
//!      {
//!        "username": "John Nash"
//!      }
//!    }
//!    ```
//! 3. Array blocks, only used by environments.
//!    ```text
//!    vars:secret [
//!      token,
//!      password
//!    ]
//!    ```
//! 4. Inline values, only used by environments.
//!    ```text
//!    color: #ff0000
//!    ```
//!
//! This module only understands the syntax. Giving a meaning to each block is done by the callers.
//! The grammar follows the official implementation in `@usebruno/lang`:
//! <https://github.com/usebruno/bruno/blob/main/packages/bruno-lang/v2/src/bruToJson.js>

use std::fmt;

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Document {
    pub blocks: Vec<Block>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Block {
    pub name: String,
    /// Line (starting at 1) where the block starts.
    pub line: usize,
    pub content: Content,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Content {
    Dictionary(Vec<Pair>),
    Text(String),
    Array(Vec<String>),
    Inline(String),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Pair {
    pub key: String,
    pub value: Value,
    /// Keys prefixed with `~` are disabled.
    pub enabled: bool,
    pub annotations: Vec<Annotation>,
    /// Line (starting at 1) where the pair starts.
    pub line: usize,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Value {
    Text(String),
    List(Vec<String>),
}

/// Decorator placed in the lines before a pair, like `@description('the user id')`.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Annotation {
    pub name: String,
    pub value: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ParseError {
    /// Line (starting at 1) where the error was found.
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

impl Document {
    /// Returns the first block with the given name.
    pub fn block(&self, name: &str) -> Option<&Block> {
        self.blocks.iter().find(|block| block.name == name)
    }
}

impl Block {
    pub fn pairs(&self) -> &[Pair] {
        match &self.content {
            Content::Dictionary(pairs) => pairs,
            _ => &[],
        }
    }

    pub fn text(&self) -> Option<&str> {
        match &self.content {
            Content::Text(text) => Some(text),
            _ => None,
        }
    }

    /// Returns the text value of the first enabled pair with the given key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.pairs()
            .iter()
            .find(|pair| pair.enabled && pair.key == key)
            .and_then(|pair| match &pair.value {
                Value::Text(text) => Some(text.as_str()),
                Value::List(_) => None,
            })
    }
}

pub fn parse(source: &str) -> Result<Document, ParseError> {
    let source = source.replace("\r\n", "\n");
    Parser {
        source: &source,
        pos: 0,
    }
    .document()
}

fn is_text_block(name: &str) -> bool {
    matches!(
        name,
        "body"
            | "body:json"
            | "body:text"
            | "body:xml"
            | "body:sparql"
            | "body:graphql"
            | "body:graphql:vars"
            | "tests"
            | "docs"
            | "example"
    ) || name.starts_with("script:")
}

fn is_dictionary_block(name: &str) -> bool {
    matches!(
        name,
        "meta"
            | "app"
            | "settings"
            | "get"
            | "post"
            | "put"
            | "delete"
            | "patch"
            | "options"
            | "head"
            | "connect"
            | "trace"
            | "http"
            | "grpc"
            | "ws"
            | "headers"
            | "metadata"
            | "query"
            | "params:path"
            | "params:query"
            | "vars"
            | "vars:pre-request"
            | "vars:post-response"
            | "assert"
            | "auth"
            | "body:grpc"
            | "body:ws"
            | "body:form-urlencoded"
            | "body:multipart-form"
            | "body:file"
    ) || name.starts_with("auth:")
        || name.starts_with("vars:externalsecrets:")
}

struct Parser<'a> {
    source: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn document(mut self) -> Result<Document, ParseError> {
        let mut blocks = vec![];
        loop {
            while !self.eof() && self.rest_of_line().trim().is_empty() {
                self.take_line();
            }
            if self.eof() {
                return Ok(Document { blocks });
            }
            blocks.push(self.block()?);
        }
    }

    fn block(&mut self) -> Result<Block, ParseError> {
        let line = self.line();
        let header = self.take_line().trim();

        let (name, content) = if let Some(name) = header.strip_suffix("{}") {
            let name = block_name(name, line)?;
            let content = if is_text_block(name) {
                Content::Text(String::new())
            } else {
                Content::Dictionary(vec![])
            };
            (name, content)
        } else if let Some(name) = header.strip_suffix('{') {
            let name = block_name(name, line)?;
            (name, self.braces_block(name, line)?)
        } else if let Some(name) = header.strip_suffix('[') {
            let name = block_name(name, line)?;
            (name, Content::Array(self.array(name, line)?))
        } else if let (Some(start), true) = (header.find('['), header.ends_with(']')) {
            let name = block_name(&header[..start], line)?;
            let items = array_items(&header[start + 1..header.len() - 1]);
            (name, Content::Array(items))
        } else if let Some((name, value)) = header.split_once(':') {
            let name = block_name(name, line)?;
            (name, Content::Inline(value.trim().to_string()))
        } else {
            return Err(error(line, format!("expected a block, found `{header}`")));
        };

        Ok(Block {
            name: name.to_string(),
            line,
            content,
        })
    }

    fn braces_block(&mut self, name: &str, line: usize) -> Result<Content, ParseError> {
        if is_text_block(name) {
            return Ok(Content::Text(self.text(name, line)?));
        }
        if is_dictionary_block(name) {
            return Ok(Content::Dictionary(self.dictionary(name, line)?));
        }
        // Unknown blocks are kept so new Bruno versions do not break the parser.
        let start = self.pos;
        match self.dictionary(name, line) {
            Ok(pairs) => Ok(Content::Dictionary(pairs)),
            Err(_) => {
                self.pos = start;
                Ok(Content::Text(self.text(name, line)?))
            }
        }
    }

    fn text(&mut self, name: &str, line: usize) -> Result<String, ParseError> {
        let mut lines = vec![];
        loop {
            if self.eof() {
                return Err(unclosed(name, line));
            }
            let text = self.take_line();
            if is_block_end(text) {
                break;
            }
            // Empty lines at the start are not part of the content
            if lines.is_empty() && text.is_empty() {
                continue;
            }
            lines.push(text.strip_prefix("  ").unwrap_or(text));
        }
        Ok(lines.join("\n"))
    }

    fn dictionary(&mut self, name: &str, line: usize) -> Result<Vec<Pair>, ParseError> {
        let mut pairs = vec![];
        let mut annotations = vec![];
        loop {
            if self.eof() {
                return Err(unclosed(name, line));
            }
            let text = self.rest_of_line();
            if is_block_end(text) {
                self.take_line();
                return Ok(pairs);
            }
            if text.trim().is_empty() {
                self.take_line();
                continue;
            }
            if is_block_header(text) {
                return Err(unclosed(name, line));
            }
            if text.trim_start().starts_with('@')
                && let Some(annotation) = self.annotation()
            {
                annotations.push(annotation);
                continue;
            }
            let mut pair = self.pair(name == "assert")?;
            pair.annotations = std::mem::take(&mut annotations);
            pairs.push(pair);
        }
    }

    fn pair(&mut self, spaces_in_keys: bool) -> Result<Pair, ParseError> {
        let line = self.line();
        self.skip_spaces();

        let quoted = self.rest().starts_with('"') || self.rest().starts_with("~\"");
        let (mut key, mut enabled) = if quoted {
            let enabled = !self.eat("~");
            (self.quoted_key(line)?, enabled)
        } else {
            let rest = self.rest_of_line();
            let end = if spaces_in_keys {
                rest.find(':')
            } else {
                rest.find([':', ' ', '\t'])
            }
            .unwrap_or(rest.len());
            self.pos += end;
            (rest[..end].trim_end().to_string(), true)
        };
        if !quoted && let Some(stripped) = key.strip_prefix('~') {
            key = stripped.to_string();
            enabled = false;
        }
        if key.is_empty() {
            return Err(error(line, "expected a key"));
        }

        self.skip_spaces();
        if !self.eat(":") {
            return Err(error(line, format!("expected `:` after the key `{key}`")));
        }
        self.skip_spaces();

        let value = if self.rest_of_line().trim_end() == "[" {
            Value::List(self.list(line)?)
        } else if self.rest().starts_with("'''") {
            Value::Text(self.multiline(line)?)
        } else {
            Value::Text(self.take_line().trim().to_string())
        };

        Ok(Pair {
            key,
            value,
            enabled,
            annotations: vec![],
            line,
        })
    }

    fn quoted_key(&mut self, line: usize) -> Result<String, ParseError> {
        self.eat("\"");
        let mut key = String::new();
        let mut chars = self.rest_of_line().char_indices();
        while let Some((index, char)) = chars.next() {
            match char {
                '"' => {
                    self.pos += index + 1;
                    return Ok(key);
                }
                '\\' if self.rest()[index + 1..].starts_with('"') => {
                    chars.next();
                    key.push('"');
                }
                _ => key.push(char),
            }
        }
        Err(error(line, "unclosed quoted key"))
    }

    fn list(&mut self, line: usize) -> Result<Vec<String>, ParseError> {
        self.take_line();
        let mut items = vec![];
        loop {
            if self.eof() || is_block_end(self.rest_of_line()) {
                return Err(error(line, "unclosed list, expected `]`"));
            }
            let item = self.take_line().trim();
            if item == "]" {
                return Ok(items);
            }
            if !item.is_empty() {
                items.push(item.to_string());
            }
        }
    }

    fn multiline(&mut self, line: usize) -> Result<String, ParseError> {
        self.eat("'''");
        let Some(end) = self.rest().find("'''") else {
            return Err(error(line, "unclosed multiline value, expected `'''`"));
        };
        let content = &self.rest()[..end];
        self.pos += end + 3;

        let value = content
            .split('\n')
            .map(|text| strip_indentation(text, 4))
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string();

        let after = self.take_line().trim();
        if after.is_empty() {
            Ok(value)
        } else if after.starts_with("@contentType(") && after.ends_with(')') {
            Ok(format!("{value} {after}"))
        } else {
            Err(error(
                line,
                format!("unexpected `{after}` after multiline value"),
            ))
        }
    }

    fn array(&mut self, name: &str, line: usize) -> Result<Vec<String>, ParseError> {
        let mut items = vec![];
        loop {
            if self.eof() {
                return Err(unclosed(name, line));
            }
            let text = self.take_line().trim();
            if text == "]" {
                return Ok(items);
            }
            if !text.starts_with('@') {
                items.extend(array_items(text));
            }
        }
    }

    /// Parses an annotation line. Returns `None`, without consuming anything, if the line is not
    /// an annotation, like the key `@local: value`.
    fn annotation(&mut self) -> Option<Annotation> {
        let start = self.pos;
        let annotation = self.try_annotation();
        if annotation.is_none() {
            self.pos = start;
        }
        annotation
    }

    fn try_annotation(&mut self) -> Option<Annotation> {
        self.skip_spaces();
        if !self.eat("@") {
            return None;
        }
        let rest = self.rest();
        let name_len = rest
            .find(['(', ')', ' ', '\t', '\n', ':'])
            .unwrap_or(rest.len());
        if name_len == 0 {
            return None;
        }
        let name = rest[..name_len].to_string();
        self.pos += name_len;

        let value = if self.eat("(") {
            let value = self.annotation_argument()?;
            if !self.eat(")") {
                return None;
            }
            Some(value)
        } else {
            None
        };

        if self.rest().starts_with(':') || !self.rest_of_line().trim().is_empty() {
            return None;
        }
        self.take_line();
        Some(Annotation { name, value })
    }

    fn annotation_argument(&mut self) -> Option<String> {
        let rest = self.rest();
        if let Some(content) = rest.strip_prefix("'''") {
            let end = content.find("'''")?;
            self.pos += end + 6;
            Some(outdent_multiline_argument(&content[..end]))
        } else if let Some(content) = rest.strip_prefix('\'') {
            let end = content.find('\'')?;
            self.pos += end + 2;
            Some(content[..end].to_string())
        } else if let Some(content) = rest.strip_prefix('"') {
            let mut value = String::new();
            let mut chars = content.char_indices();
            while let Some((index, char)) = chars.next() {
                match char {
                    '"' => {
                        self.pos += index + 2;
                        return Some(value);
                    }
                    '\\' => match chars.next()?.1 {
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),
                        other @ ('"' | '\\') => value.push(other),
                        other => {
                            value.push('\\');
                            value.push(other);
                        }
                    },
                    _ => value.push(char),
                }
            }
            None
        } else {
            let end = rest.find(')')?;
            self.pos += end;
            Some(rest[..end].to_string())
        }
    }

    fn eof(&self) -> bool {
        self.pos >= self.source.len()
    }

    fn rest(&self) -> &'a str {
        &self.source[self.pos..]
    }

    fn rest_of_line(&self) -> &'a str {
        let rest = self.rest();
        &rest[..rest.find('\n').unwrap_or(rest.len())]
    }

    /// Returns the rest of the current line and moves to the next one.
    fn take_line(&mut self) -> &'a str {
        let text = self.rest_of_line();
        self.pos = (self.pos + text.len() + 1).min(self.source.len());
        text
    }

    fn skip_spaces(&mut self) {
        let rest = self.rest();
        self.pos += rest.len() - rest.trim_start_matches([' ', '\t']).len();
    }

    fn eat(&mut self, text: &str) -> bool {
        let found = self.rest().starts_with(text);
        if found {
            self.pos += text.len();
        }
        found
    }

    fn line(&self) -> usize {
        self.source[..self.pos].matches('\n').count() + 1
    }
}

/// Blocks are closed by a `}` at the start of a line.
fn is_block_end(line: &str) -> bool {
    line.trim_end() == "}"
}

/// A line like `get {` at the start of a line, found where a pair was expected.
fn is_block_header(line: &str) -> bool {
    let mut words = line.split_whitespace();
    !line.starts_with([' ', '\t'])
        && matches!(
            (words.next(), words.next(), words.next()),
            (Some(_), Some("{"), None)
        )
}

fn block_name(name: &str, line: usize) -> Result<&str, ParseError> {
    let name = name.trim();
    if name.is_empty() || name.contains(char::is_whitespace) {
        return Err(error(line, format!("invalid block name `{name}`")));
    }
    Ok(name)
}

fn array_items(text: &str) -> Vec<String> {
    text.split(',')
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(|item| {
            item.strip_prefix('"')
                .and_then(|item| item.strip_suffix('"'))
                .unwrap_or(item)
                .to_string()
        })
        .collect()
}

fn strip_indentation(text: &str, spaces: usize) -> &str {
    let indentation = text.len() - text.trim_start_matches(' ').len();
    &text[indentation.min(spaces)..]
}

fn outdent_multiline_argument(content: &str) -> String {
    if !content.contains('\n') {
        return content.to_string();
    }
    let mut lines: Vec<&str> = content.split('\n').collect();
    if lines.first().is_some_and(|line| line.is_empty()) {
        lines.remove(0);
    }
    if lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    let indentation = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                ""
            } else {
                &line[indentation..]
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn unclosed(name: &str, line: usize) -> ParseError {
    error(line, format!("unclosed block `{name}`, expected `}}`"))
}

fn error(line: usize, message: impl Into<String>) -> ParseError {
    ParseError {
        line,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &str) -> Value {
        Value::Text(value.to_string())
    }

    fn keys(block: &Block) -> Vec<(&str, bool)> {
        block
            .pairs()
            .iter()
            .map(|pair| (pair.key.as_str(), pair.enabled))
            .collect()
    }

    #[test]
    fn parses_official_request_fixture() {
        let document = parse(include_str!("../../test/fixtures/bru/request.bru")).unwrap();
        let names: Vec<&str> = document.blocks.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "meta",
                "get",
                "params:query",
                "params:path",
                "headers",
                "auth:awsv4",
                "auth:basic",
                "auth:wsse",
                "auth:bearer",
                "auth:digest",
                "auth:oauth2",
                "body:json",
                "body:text",
                "body:xml",
                "body:sparql",
                "body:form-urlencoded",
                "body:multipart-form",
                "body:file",
                "body:graphql",
                "body:graphql:vars",
                "vars:pre-request",
                "vars:post-response",
                "assert",
                "script:pre-request",
                "tests",
                "docs",
            ]
        );

        let meta = document.block("meta").unwrap();
        assert_eq!(meta.get("name"), Some("Send Bulk SMS"));
        assert_eq!(meta.get("seq"), Some("1"));
        assert_eq!(
            meta.pairs()[3].value,
            Value::List(vec!["foo".into(), "bar".into()])
        );

        let get = document.block("get").unwrap();
        assert_eq!(get.get("url"), Some("https://api.textlocal.in/send/:id"));
        assert_eq!(get.get("body"), Some("json"));
    }

    #[test]
    fn parses_quoted_and_disabled_keys() {
        let document = parse(include_str!("../../test/fixtures/bru/request.bru")).unwrap();
        assert_eq!(
            keys(document.block("params:query").unwrap()),
            [
                ("apiKey", true),
                ("numbers", true),
                ("key with spaces", true),
                ("colon:parameter", true),
                ("nested escaped \"quote\"", true),
                ("{braces}", true),
                ("disabled:colon:parameter", false),
                ("message", false),
            ]
        );
        let headers = document.block("headers").unwrap();
        assert_eq!(headers.pairs()[1].value, text("Bearer 123"));
        assert_eq!(headers.pairs()[7].key, "transaction-id");
        assert_eq!(headers.pairs()[7].value, text("{{transactionId}}"));
        // Empty values are allowed
        let oauth2 = document.block("auth:oauth2").unwrap();
        assert_eq!(oauth2.get("refresh_token_url"), Some(""));
    }

    #[test]
    fn parses_text_blocks_keeping_indentation() {
        let document = parse(include_str!("../../test/fixtures/bru/request.bru")).unwrap();
        assert_eq!(
            document.block("body:json").unwrap().text(),
            Some("{\n  \"hello\": \"world\"\n}")
        );
        assert_eq!(
            document.block("body:sparql").unwrap().text(),
            Some("SELECT * WHERE {\n  ?subject ?predicate ?object .\n}\nLIMIT 10")
        );
        assert_eq!(
            document.block("tests").unwrap().text(),
            Some(
                "function onResponse(request, response) {\n  expect(response.status).to.equal(200);\n}"
            )
        );
    }

    #[test]
    fn keys_starting_with_at_are_not_annotations() {
        let document = parse(include_str!("../../test/fixtures/bru/request.bru")).unwrap();
        let vars = document.block("vars:post-response").unwrap();
        assert_eq!(
            keys(vars),
            [
                ("token", true),
                ("@orderNumber", true),
                ("petId", false),
                ("@transactionId", false),
            ]
        );
        assert!(vars.pairs().iter().all(|pair| pair.annotations.is_empty()));

        let assert = document.block("assert").unwrap();
        assert_eq!(
            keys(assert),
            [("$res.status", true), ("$res.body.message", false)]
        );

        let file = document.block("body:file").unwrap();
        assert_eq!(
            file.pairs()[0].value,
            text("@file(path/to/file.json) @contentType(application/json)")
        );
    }

    #[test]
    fn parses_official_collection_fixture() {
        let document = parse(include_str!("../../test/fixtures/bru/collection.bru")).unwrap();
        assert_eq!(
            document.block("meta").unwrap().get("type"),
            Some("collection")
        );
        assert_eq!(document.block("auth").unwrap().get("mode"), Some("none"));
        assert_eq!(
            document.block("script:post-response").unwrap().text(),
            Some("console.log(\"In Collection post Request Script\");")
        );
    }

    #[test]
    fn parses_annotations() {
        let document = parse(include_str!("../../test/fixtures/bru/annotations.bru")).unwrap();
        let pairs = document.block("vars:pre-request").unwrap().pairs();
        assert_eq!(pairs[0].key, "key");
        assert_eq!(pairs[0].value, text("value"));
        assert_eq!(
            pairs[0].annotations,
            [Annotation {
                name: "description".into(),
                value: Some("found in C:\\Users\\File\\Path".into()),
            }]
        );
        assert_eq!(
            pairs[1].annotations[0].value.as_deref(),
            Some("height of 2' ")
        );

        let document = parse(
            "vars:pre-request {\n  @number\n  @description('''\n    first\n      second\n  ''')\n  count: 1\n}\n",
        )
        .unwrap();
        let pair = &document.blocks[0].pairs()[0];
        assert_eq!(pair.key, "count");
        assert_eq!(
            pair.annotations,
            [
                Annotation {
                    name: "number".into(),
                    value: None,
                },
                Annotation {
                    name: "description".into(),
                    value: Some("first\n  second".into()),
                },
            ]
        );
    }

    #[test]
    fn parses_multiline_values() {
        let source = "body:multipart-form {\n  data: '''\n    {\n      \"a\": 1\n    }\n  ''' @contentType(application/json)\n  other: value\n}\n";
        let pairs = parse(source).unwrap().blocks[0].pairs().to_vec();
        assert_eq!(
            pairs[0].value,
            text("{\n  \"a\": 1\n} @contentType(application/json)")
        );
        assert_eq!(pairs[1].key, "other");
        assert_eq!(pairs[1].line, 7);
    }

    #[test]
    fn parses_environments() {
        let source = "vars {\n  host: http://localhost\n  token: '''\n    line one\n    line two\n  '''\n}\nvars:secret [\n  password,\n  apiKey\n]\ncolor: #ff0000\nextends [base, \"with spaces\"]\n";
        let document = parse(source).unwrap();
        let vars = document.block("vars").unwrap();
        assert_eq!(vars.get("host"), Some("http://localhost"));
        assert_eq!(vars.get("token"), Some("line one\nline two"));
        assert_eq!(
            document.block("vars:secret").unwrap().content,
            Content::Array(vec!["password".into(), "apiKey".into()])
        );
        assert_eq!(
            document.block("color").unwrap().content,
            Content::Inline("#ff0000".into())
        );
        assert_eq!(
            document.block("extends").unwrap().content,
            Content::Array(vec!["base".into(), "with spaces".into()])
        );
    }

    #[test]
    fn handles_whitespace_crlf_and_empty_blocks() {
        let source = "meta {\r\n  name: crlf\r\n}\r\n   \r\n\t\r\nheaders {\r\n}\r\ndocs {}\r\n\r\nbody:text {\r\n\r\n  \r\n  hello\r\n}";
        let document = parse(source).unwrap();
        assert_eq!(document.block("meta").unwrap().get("name"), Some("crlf"));
        assert_eq!(
            document.block("headers").unwrap().content,
            Content::Dictionary(vec![])
        );
        assert_eq!(
            document.block("docs").unwrap().content,
            Content::Text(String::new())
        );
        assert_eq!(document.block("body:text").unwrap().text(), Some("\nhello"));
        assert_eq!(document.block("body:text").unwrap().line, 10);
    }

    #[test]
    fn assert_keys_can_contain_spaces() {
        let document = parse("assert {\n  res.body.items.length : gt 2\n}\n").unwrap();
        let pair = &document.blocks[0].pairs()[0];
        assert_eq!(pair.key, "res.body.items.length");
        assert_eq!(pair.value, text("gt 2"));
    }

    #[test]
    fn keeps_unknown_blocks() {
        let document =
            parse("future {\n  key: value\n}\nfuture:text {\n  no pairs here\n}\n").unwrap();
        assert_eq!(document.blocks[0].get("key"), Some("value"));
        assert_eq!(document.blocks[1].text(), Some("no pairs here"));
    }

    #[test]
    fn reports_errors_with_line_numbers() {
        let cases = [
            (
                "meta {\n  name: test\n",
                "line 1: unclosed block `meta`, expected `}`",
            ),
            (
                "meta {\n  name: test\n\nget {\n}\n",
                "line 1: unclosed block `meta`, expected `}`",
            ),
            (
                "meta {\n  name test\n}\n",
                "line 2: expected `:` after the key `name`",
            ),
            (
                "meta {\n}\n\nnot a block\n",
                "line 4: expected a block, found `not a block`",
            ),
            (
                "headers {\n  \"open: value\n}\n",
                "line 2: unclosed quoted key",
            ),
            (
                "vars {\n  a: '''\n    text\n}\n",
                "line 2: unclosed multiline value, expected `'''`",
            ),
            (
                "meta {\n  tags: [\n    a\n}\n",
                "line 2: unclosed list, expected `]`",
            ),
            ("my block {\n}\n", "line 1: invalid block name `my block`"),
        ];
        for (source, message) in cases {
            assert_eq!(
                parse(source).unwrap_err().to_string(),
                message,
                "{source:?}"
            );
        }
    }
}
