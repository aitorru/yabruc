// Differential test between parser.js and the Rust parser.
// Usage: node difftest.js path/to/difftest-binary   (or `devenv shell parser-guide-check`)
const fs = require('fs');
const os = require('os');
const path = require('path');
const { execFileSync } = require('child_process');
const P = require('./parser.js');

const binary = process.argv[2];
if (!binary) {
  console.error('usage: difftest.js <difftest binary>');
  process.exit(2);
}
const root = path.resolve(__dirname, '../..');
const files = execFileSync('git', ['ls-files', '-co', '--exclude-standard', '*.bru'], { cwd: root }).toString().trim().split('\n');
const base = files.map((f) => fs.readFileSync(path.join(root, f), 'utf8'));
const edge = [
  '', '\n\n', 'meta {', 'meta {}', 'docs {}', 'x {\n}', 'meta {\n  name: é😀\n}', 'meta {\r\n  name: crlf\r\n}\r\n',
  'headers {\n  "a\\"b": c\n  ~"d": e\n  ~f: g\n  "unclosed: x\n}', 'vars {\n  a: \'\'\'\n    x\n  \'\'\' @contentType(y)\n}',
  "vars {\n  a: '''x''' junk\n}", 'assert {\n  res.body a b : eq 1\n  ~ x: y\n}', 'future {\n  not a pair\n}',
  'vars:pre-request {\n  @description("a\\nb\\\\c\\"d\\q")\n  @number\n  @x(unquoted)\n  @y(\'single\')\n  @z(\'\'\'\n    m\n  \'\'\')\n  k: v\n}',
  'vars:pre-request {\n  @k: v\n  @a(x) trailing\n  b: c\n}', 'extends [a, "b c", ]\ncolor: red\nvars:secret [\n  x, ~y\n  @ann\n]',
  'http {\n  method: purge\n  url: u\n  body: multipartForm\n}\nbody:multipart-form {\n  f: @file(a|b|) @contentType( t )\n  g: text @contentType(x\n  h: @file()\n}',
  'post {\n  body: file\n}\nbody:file {\n  a: @file(x) @contentType(j)\n  ~b: @file(y)\n  c: nope\n}',
  'get {\n  auth: apikey\n}\nauth:apikey {\n  placement: queryparams\n}\nsettings {\n  timeout: +5\n  maxRedirects: 4294967296\n  encodeUrl: TRUE\n}\nmeta {\n  seq: 1e2\n  tags: one\n}',
  'meta {\n  seq: nan\n  type: grpc\n}', 'meta {\n  seq: -inf\n}\nget {\n}', 'http {\n}', 'http {\n  method: bad method\n}', 'get {\n  body: yaml\n}',
  'meta {\n  tags: [\n    a\n\n    b\n  ]\n}\nget {\n  url: x\n}\nbody {\n  {"legacy": true}\n}\nget {\n  body: json\n}',
  'get {\n  url: x\n  auth: basic\n}\nheaders {\n  x: [\n    a\n    b\n  ]\n}\nauth:basic {\n  username: [\n    u\n    v\n  ]\n}\nvars {\n  l: [\n    1\n    2\n  ]\n}',
  'meta {\n  name: a\n\nget {\n}', 'meta {\n  name: a\n   get {\n}', 'body:text {\n\n\n  a\n\n}', 'docs {\n  }\n}',
];

let seed = 42;
const rand = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
const pick = (a) => a[Math.floor(rand() * a.length)];
const tokens = ['}', '{', '[', ']', "'''", '@', '~', '"', ':', ' ', '\t', '\r', 'é', '😀', '\\', '(', ')', '@description("x")', '@contentType(a)', 'get {', 'docs {}', 'vars:secret [', 'x: [', ',', '  a: b', '~"q": w', "  k: '''", ' ', ' ', '﻿'];
function mutate(src) {
  let lines = src.split('\n');
  const n = 1 + Math.floor(rand() * 4);
  for (let i = 0; i < n; i++) {
    const at = Math.floor(rand() * (lines.length + 1));
    switch (Math.floor(rand() * 7)) {
      case 0: lines.splice(at, 1); break;
      case 1: if (lines[at] !== undefined) lines.splice(at, 0, lines[at]); break;
      case 2: lines.splice(at, 0, pick(tokens)); break;
      case 3: if (lines[at] !== undefined) lines[at] = pick(['', ' ', '  ', '\t']) + lines[at].replace(/^ {0,2}/, ''); break;
      case 4: if (lines[at] !== undefined) { const p = Math.floor(rand() * (lines[at].length + 1)); lines[at] = lines[at].slice(0, p) + pick(tokens) + lines[at].slice(p); } break;
      case 5: { const s = lines.join('\n'); const p = Math.floor(rand() * (s.length + 1)); lines = s.slice(0, p).split('\n'); break; }
      case 6: if (lines[at] !== undefined && lines[at + 1] !== undefined) [lines[at], lines[at + 1]] = [lines[at + 1], lines[at]]; break;
    }
  }
  let out = lines.join('\n');
  if (rand() < 0.1) out = out.split('\n').join('\r\n');
  return out;
}
const inputs = [...base, ...edge];
for (let i = 0; i < 6000; i++) inputs.push(mutate(pick(base.concat(edge))));

const num = (n) => (n === null ? null : Number.isNaN(n) ? 'NaN' : n === Infinity ? 'Infinity' : n === -Infinity ? '-Infinity' : n);
function js(source) {
  const r = P.parse(source);
  if (r.error) return { error: r.error.message };
  const d = r.document;
  let request;
  try { request = P.documentToRequest(d); request.seq = num(request.seq); } catch (e) { request = { error: e.message }; }
  const defaults = P.documentToDefaults(d); defaults.seq = num(defaults.seq);
  return {
    document: d.blocks.map((b) => ({ name: b.name, line: b.line, content: b.content.type === 'Dictionary' ? { type: 'Dictionary', value: b.content.value.map((p) => ({ key: p.key, enabled: p.enabled, line: p.line, value: p.value, annotations: p.annotations })) } : b.content })),
    request,
    defaults,
    environment: P.documentToEnvironment('env', d),
  };
}
const canon = (v) => JSON.stringify(v, (k, x) => (x && typeof x === 'object' && !Array.isArray(x) ? Object.fromEntries(Object.keys(x).sort().map((key) => [key, x[key]])) : x));

const inputsFile = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'yabruc-difftest-')), 'inputs.json');
fs.writeFileSync(inputsFile, JSON.stringify(inputs));
const rust = JSON.parse(execFileSync(binary, [inputsFile], { maxBuffer: 1 << 30 }).toString());
let failures = 0, errors = 0, requests = 0;
inputs.forEach((source, i) => {
  const a = canon(rust[i]), b = canon(js(source));
  if (rust[i].error) errors++; else if (!rust[i].request.error) requests++;
  if (a !== b) {
    if (failures++ < 3) {
      console.log('MISMATCH #' + i, JSON.stringify(source));
      console.log(' rust:', a.slice(0, 1500));
      console.log(' js:  ', b.slice(0, 1500));
    }
  }
});
console.log(`inputs=${inputs.length} (files=${base.length}, cases=${edge.length}) syntax_errors=${errors} valid_requests=${requests} mismatches=${failures}`);
process.exit(failures ? 1 : 0);
