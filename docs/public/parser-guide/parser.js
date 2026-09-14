// Port to JavaScript of src/parser/bru.rs and src/parser/bru2struct.rs.
// It follows the Rust code line by line so it can be used to understand it, and it records a
// trace with every decision the parser takes.

(function (global) {
  'use strict';

  // Unicode White_Space, the definition used by Rust `char::is_whitespace` and `str::trim`.
  const WS = '\\t\\n\\u000B\\f\\r \\u0085\\u00A0\\u1680\\u2000-\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000';
  const TRIM = new RegExp('^[' + WS + ']+|[' + WS + ']+$', 'g');
  const TRIM_START = new RegExp('^[' + WS + ']+');
  const TRIM_END = new RegExp('[' + WS + ']+$');
  const HAS_WS = new RegExp('[' + WS + ']');
  const SPLIT_WS = new RegExp('[' + WS + ']+');
  const trim = (s) => s.replace(TRIM, '');
  const trimStart = (s) => s.replace(TRIM_START, '');
  const trimEnd = (s) => s.replace(TRIM_END, '');
  const splitWhitespace = (s) => trim(s).split(SPLIT_WS).filter((w) => w.length);

  class ParseError extends Error {
    constructor(line, message) {
      super('line ' + line + ': ' + message);
      this.line = line;
      this.detail = message;
    }
  }

  const TEXT_BLOCKS = ['body', 'body:json', 'body:text', 'body:xml', 'body:sparql', 'body:graphql', 'body:graphql:vars', 'tests', 'docs', 'example'];
  const DICTIONARY_BLOCKS = ['meta', 'app', 'settings', 'get', 'post', 'put', 'delete', 'patch', 'options', 'head', 'connect', 'trace', 'http', 'grpc', 'ws', 'headers', 'metadata', 'query', 'params:path', 'params:query', 'vars', 'vars:pre-request', 'vars:post-response', 'assert', 'auth', 'body:grpc', 'body:ws', 'body:form-urlencoded', 'body:multipart-form', 'body:file'];

  const isTextBlock = (name) => TEXT_BLOCKS.includes(name) || name.startsWith('script:');
  const isDictionaryBlock = (name) => DICTIONARY_BLOCKS.includes(name) || name.startsWith('auth:') || name.startsWith('vars:externalsecrets:');
  const isBlockEnd = (line) => trimEnd(line) === '}';
  const isBlockHeader = (line) => {
    const words = splitWhitespace(line);
    return !(line.startsWith(' ') || line.startsWith('\t')) && words.length === 2 && words[1] === '{';
  };
  const unclosed = (name, line) => new ParseError(line, 'unclosed block `' + name + '`, expected `}`');

  function blockName(name, line) {
    name = trim(name);
    if (!name.length || HAS_WS.test(name)) {
      throw new ParseError(line, 'invalid block name `' + name + '`');
    }
    return name;
  }

  function arrayItems(text) {
    return text.split(',').map(trim).filter((item) => item.length).map((item) =>
      item.startsWith('"') && item.endsWith('"') && item.length >= 2 ? item.slice(1, -1) : item
    );
  }

  function stripIndentation(text, spaces) {
    const indentation = text.length - text.replace(/^ +/, '').length;
    return text.slice(Math.min(indentation, spaces));
  }

  function outdentMultilineArgument(content) {
    if (!content.includes('\n')) return content;
    const lines = content.split('\n');
    if (lines.length && lines[0] === '') lines.shift();
    if (lines.length && trim(lines[lines.length - 1]) === '') lines.pop();
    const indents = lines.filter((l) => trim(l) !== '').map((l) => l.length - l.replace(/^[ \t]+/, '').length);
    const indentation = indents.length ? Math.min(...indents) : 0;
    return lines.map((l) => (trim(l) === '' ? '' : l.slice(indentation))).join('\n');
  }

  class Parser {
    constructor(source) {
      this.source = source;
      this.pos = 0;
      this.events = [];
    }

    // Records a step of the parser. `fn` is the name of the Rust function.
    trace(fn, message, line) {
      this.events.push({ fn, message, line: line === undefined ? this.line() : line, pos: this.pos, discarded: false });
    }

    document() {
      const blocks = [];
      for (;;) {
        while (!this.eof() && trim(this.restOfLine()) === '') {
          this.trace('document', 'línea vacía, se salta');
          this.takeLine();
        }
        if (this.eof()) {
          this.trace('document', 'fin del fichero: ' + blocks.length + ' bloque(s)');
          return { blocks };
        }
        blocks.push(this.block());
      }
    }

    block() {
      const line = this.line();
      const header = trim(this.takeLine());
      let name, content;
      if (header.endsWith('{}')) {
        name = blockName(header.slice(0, -2), line);
        content = isTextBlock(name) ? { type: 'Text', value: '' } : { type: 'Dictionary', value: [] };
        this.trace('block', 'cabecera `' + header + '` termina en `{}`: bloque `' + name + '` vacío', line);
      } else if (header.endsWith('{')) {
        name = blockName(header.slice(0, -1), line);
        this.trace('block', 'cabecera `' + header + '` termina en `{`: bloque `' + name + '`', line);
        content = this.bracesBlock(name, line);
      } else if (header.endsWith('[')) {
        name = blockName(header.slice(0, -1), line);
        this.trace('block', 'cabecera `' + header + '` termina en `[`: array `' + name + '` en varias líneas', line);
        content = { type: 'Array', value: this.array(name, line) };
      } else if (header.indexOf('[') !== -1 && header.endsWith(']')) {
        const start = header.indexOf('[');
        name = blockName(header.slice(0, start), line);
        content = { type: 'Array', value: arrayItems(header.slice(start + 1, header.length - 1)) };
        this.trace('block', 'cabecera `' + header + '` contiene `[...]`: array `' + name + '` en una línea', line);
      } else if (header.indexOf(':') !== -1) {
        const colon = header.indexOf(':');
        name = blockName(header.slice(0, colon), line);
        content = { type: 'Inline', value: trim(header.slice(colon + 1)) };
        this.trace('block', 'cabecera `' + header + '` tiene `:`: valor inline `' + name + '`', line);
      } else {
        throw new ParseError(line, 'expected a block, found `' + header + '`');
      }
      return { name, line, content };
    }

    bracesBlock(name, line) {
      if (isTextBlock(name)) {
        this.trace('braces_block', '`' + name + '` es un bloque de texto conocido', line);
        return { type: 'Text', value: this.text(name, line) };
      }
      if (isDictionaryBlock(name)) {
        this.trace('braces_block', '`' + name + '` es un bloque diccionario conocido', line);
        return { type: 'Dictionary', value: this.dictionary(name, line) };
      }
      this.trace('braces_block', '`' + name + '` es desconocido: se intenta como diccionario', line);
      const start = this.pos;
      const firstEvent = this.events.length;
      try {
        return { type: 'Dictionary', value: this.dictionary(name, line) };
      } catch (error) {
        if (!(error instanceof ParseError)) throw error;
        for (let i = firstEvent; i < this.events.length; i++) this.events[i].discarded = true;
        this.pos = start;
        this.trace('braces_block', 'no es un diccionario (' + error.message + '): se vuelve atrás y se lee como texto', line);
        return { type: 'Text', value: this.text(name, line) };
      }
    }

    text(name, line) {
      const lines = [];
      for (;;) {
        if (this.eof()) throw unclosed(name, line);
        const text = this.takeLine();
        if (isBlockEnd(text)) {
          this.trace('text', '`}` al inicio de línea: fin del texto (' + lines.length + ' línea(s))', this.line() - 1);
          break;
        }
        if (!lines.length && text === '') {
          this.trace('text', 'línea vacía al principio, no forma parte del texto', this.line() - 1);
          continue;
        }
        lines.push(text.startsWith('  ') ? text.slice(2) : text);
        this.trace('text', 'línea de texto, se le quitan 2 espacios de indentación', this.line() - 1);
      }
      return lines.join('\n');
    }

    dictionary(name, line) {
      const pairs = [];
      let annotations = [];
      for (;;) {
        if (this.eof()) throw unclosed(name, line);
        const text = this.restOfLine();
        if (isBlockEnd(text)) {
          this.trace('dictionary', '`}` al inicio de línea: fin de `' + name + '` (' + pairs.length + ' par(es))');
          this.takeLine();
          return pairs;
        }
        if (trim(text) === '') {
          this.trace('dictionary', 'línea vacía, se salta');
          this.takeLine();
          continue;
        }
        if (isBlockHeader(text)) {
          this.trace('dictionary', '`' + trim(text) + '` parece otra cabecera: `' + name + '` no se cerró');
          throw unclosed(name, line);
        }
        if (trimStart(text).startsWith('@')) {
          const annotation = this.annotation();
          if (annotation) {
            annotations.push(annotation);
            continue;
          }
        }
        const pair = this.pair(name === 'assert');
        pair.annotations = annotations;
        annotations = [];
        pairs.push(pair);
      }
    }

    pair(spacesInKeys) {
      const line = this.line();
      this.skipSpaces();
      const quoted = this.rest().startsWith('"') || this.rest().startsWith('~"');
      let key, enabled;
      if (quoted) {
        enabled = !this.eat('~');
        key = this.quotedKey(line);
      } else {
        const rest = this.restOfLine();
        let end = -1;
        for (let i = 0; i < rest.length; i++) {
          const c = rest[i];
          if (c === ':' || (!spacesInKeys && (c === ' ' || c === '\t'))) {
            end = i;
            break;
          }
        }
        if (end === -1) end = rest.length;
        this.pos += end;
        key = trimEnd(rest.slice(0, end));
        enabled = true;
      }
      if (!quoted && key.startsWith('~')) {
        key = key.slice(1);
        enabled = false;
      }
      if (!key.length) throw new ParseError(line, 'expected a key');
      this.skipSpaces();
      if (!this.eat(':')) throw new ParseError(line, 'expected `:` after the key `' + key + '`');
      this.skipSpaces();

      let value;
      const how = (quoted ? 'clave entre comillas' : spacesInKeys ? 'clave de assert (admite espacios)' : 'clave') +
        ' `' + key + '`' + (enabled ? '' : ' deshabilitada por `~`');
      if (trimEnd(this.restOfLine()) === '[') {
        this.trace('pair', how + ', el valor empieza con `[`: lista', line);
        value = { type: 'List', value: this.list(line) };
      } else if (this.rest().startsWith("'''")) {
        this.trace('pair', how + ", el valor empieza con `'''`: texto multilínea", line);
        value = { type: 'Text', value: this.multiline(line) };
      } else {
        value = { type: 'Text', value: trim(this.takeLine()) };
        this.trace('pair', how + ', valor de una línea `' + value.value + '`', line);
      }
      return { key, value, enabled, annotations: [], line };
    }

    quotedKey(line) {
      this.eat('"');
      let key = '';
      const rest = this.restOfLine();
      for (let index = 0; index < rest.length; index++) {
        const char = rest[index];
        if (char === '"') {
          this.pos += index + 1;
          return key;
        }
        if (char === '\\' && rest[index + 1] === '"') {
          index++;
          key += '"';
        } else {
          key += char;
        }
      }
      throw new ParseError(line, 'unclosed quoted key');
    }

    list(line) {
      this.takeLine();
      const items = [];
      for (;;) {
        if (this.eof() || isBlockEnd(this.restOfLine())) throw new ParseError(line, 'unclosed list, expected `]`');
        const item = trim(this.takeLine());
        if (item === ']') {
          this.trace('list', '`]`: fin de la lista (' + items.length + ' elemento(s))', this.line() - 1);
          return items;
        }
        if (item.length) {
          items.push(item);
          this.trace('list', 'elemento `' + item + '`', this.line() - 1);
        }
      }
    }

    multiline(line) {
      this.eat("'''");
      const end = this.rest().indexOf("'''");
      if (end === -1) throw new ParseError(line, "unclosed multiline value, expected `'''`");
      const content = this.rest().slice(0, end);
      this.pos += end + 3;
      const value = trim(content.split('\n').map((t) => stripIndentation(t, 4)).join('\n'));
      const after = trim(this.takeLine());
      if (after === '') {
        this.trace('multiline', "`'''` de cierre: se quitan hasta 4 espacios por línea y se hace trim", this.line() - 1);
        return value;
      }
      if (after.startsWith('@contentType(') && after.endsWith(')')) {
        this.trace('multiline', "`'''` de cierre seguido de `" + after + '`, que se añade al valor', this.line() - 1);
        return value + ' ' + after;
      }
      throw new ParseError(line, 'unexpected `' + after + '` after multiline value');
    }

    array(name, line) {
      const items = [];
      for (;;) {
        if (this.eof()) throw unclosed(name, line);
        const text = trim(this.takeLine());
        if (text === ']') {
          this.trace('array', '`]`: fin del array (' + items.length + ' elemento(s))', this.line() - 1);
          return items;
        }
        if (!text.startsWith('@')) {
          const found = arrayItems(text);
          items.push(...found);
          if (found.length) this.trace('array', 'elemento(s) ' + found.map((i) => '`' + i + '`').join(', '), this.line() - 1);
        }
      }
    }

    annotation() {
      const start = this.pos;
      const line = this.line();
      const annotation = this.tryAnnotation();
      if (!annotation) {
        this.pos = start;
        this.trace('annotation', 'empieza por `@` pero no es una anotación (lleva `:` o más texto): se lee como par', line);
      } else {
        this.trace('annotation', 'anotación `@' + annotation.name + '`' + (annotation.value === null ? '' : ' con valor `' + annotation.value + '`') + ', se guarda para el siguiente par', line);
      }
      return annotation;
    }

    tryAnnotation() {
      this.skipSpaces();
      if (!this.eat('@')) return null;
      const rest = this.rest();
      let nameLength = rest.search(/[() \t\n:]/);
      if (nameLength === -1) nameLength = rest.length;
      if (nameLength === 0) return null;
      const name = rest.slice(0, nameLength);
      this.pos += nameLength;
      let value = null;
      if (this.eat('(')) {
        value = this.annotationArgument();
        if (value === null) return null;
        if (!this.eat(')')) return null;
      }
      if (this.rest().startsWith(':') || trim(this.restOfLine()) !== '') return null;
      this.takeLine();
      return { name, value };
    }

    annotationArgument() {
      const rest = this.rest();
      if (rest.startsWith("'''")) {
        const content = rest.slice(3);
        const end = content.indexOf("'''");
        if (end === -1) return null;
        this.pos += end + 6;
        return outdentMultilineArgument(content.slice(0, end));
      }
      if (rest.startsWith("'")) {
        const content = rest.slice(1);
        const end = content.indexOf("'");
        if (end === -1) return null;
        this.pos += end + 2;
        return content.slice(0, end);
      }
      if (rest.startsWith('"')) {
        const content = rest.slice(1);
        let value = '';
        for (let index = 0; index < content.length; index++) {
          const char = content[index];
          if (char === '"') {
            this.pos += index + 2;
            return value;
          }
          if (char === '\\') {
            index++;
            if (index >= content.length) return null;
            const other = content[index];
            if (other === 'n') value += '\n';
            else if (other === 'r') value += '\r';
            else if (other === 't') value += '\t';
            else if (other === '"' || other === '\\') value += other;
            else value += '\\' + other;
          } else {
            value += char;
          }
        }
        return null;
      }
      const end = rest.indexOf(')');
      if (end === -1) return null;
      this.pos += end;
      return rest.slice(0, end);
    }

    eof() { return this.pos >= this.source.length; }
    rest() { return this.source.slice(this.pos); }
    restOfLine() {
      const rest = this.rest();
      const nl = rest.indexOf('\n');
      return nl === -1 ? rest : rest.slice(0, nl);
    }
    takeLine() {
      const text = this.restOfLine();
      this.pos = Math.min(this.pos + text.length + 1, this.source.length);
      return text;
    }
    skipSpaces() {
      const rest = this.rest();
      this.pos += rest.length - rest.replace(/^[ \t]+/, '').length;
    }
    eat(text) {
      const found = this.rest().startsWith(text);
      if (found) this.pos += text.length;
      return found;
    }
    line() {
      let count = 1;
      for (let i = 0; i < this.pos; i++) if (this.source[i] === '\n') count++;
      return count;
    }
  }

  // Returns { document, error, events, source } where source is the text with CRLF normalized.
  function parse(source) {
    source = source.split('\r\n').join('\n');
    const parser = new Parser(source);
    try {
      return { document: parser.document(), error: null, events: parser.events, source };
    } catch (error) {
      if (!(error instanceof ParseError)) throw error;
      parser.events.push({ fn: 'error', message: error.message, line: error.line, pos: parser.pos, discarded: false });
      return { document: null, error, events: parser.events, source };
    }
  }

  // ---------------------------------------------------------------------------------------------
  // bru2struct.rs

  const findBlock = (document, name) => document.blocks.find((b) => b.name === name);
  const pairsOf = (block) => (block && block.content.type === 'Dictionary' ? block.content.value : []);
  const textOf = (block) => (block && block.content.type === 'Text' ? block.content.value : null);
  const get = (block, key) => {
    const pair = pairsOf(block).find((p) => p.enabled && p.key === key);
    return pair && pair.value.type === 'Text' ? pair.value.value : null;
  };
  const pairText = (value) => (value.type === 'Text' ? value.value : value.value.join(','));
  const text = (document, name) => textOf(findBlock(document, name));
  const keyValues = (block) => pairsOf(block).map((p) => ({ name: p.key, value: pairText(p.value), enabled: p.enabled }));

  const RUST_FLOAT = /^[+-]?(inf|infinity|nan|\d+\.?\d*(e[+-]?\d+)?|\.\d+(e[+-]?\d+)?)$/i;
  function parseF64(s) {
    if (!RUST_FLOAT.test(s)) return null;
    const lower = s.toLowerCase().replace(/^[+-]/, '');
    const sign = s.startsWith('-') ? -1 : 1;
    if (lower === 'inf' || lower === 'infinity') return sign * Infinity;
    if (lower === 'nan') return NaN;
    return Number(s);
  }
  function parseUnsigned(s, max) {
    if (!/^\+?\d+$/.test(s)) return null;
    const n = BigInt(s.replace('+', ''));
    return n > max ? null : Number(n);
  }

  const METHODS = ['get', 'post', 'put', 'delete', 'patch', 'options', 'head', 'connect', 'trace'];

  function vars(document) {
    const variables = (block) => keyValues(block).map((p) => p.name.startsWith('@')
      ? { name: p.name.slice(1), value: p.value, enabled: p.enabled, local: true }
      : { name: p.name, value: p.value, enabled: p.enabled, local: false });
    return {
      pre_request: variables(findBlock(document, 'vars:pre-request')),
      post_response: variables(findBlock(document, 'vars:post-response'))
    };
  }

  function scripts(document) {
    return {
      pre_request: text(document, 'script:pre-request'),
      post_response: text(document, 'script:post-response'),
      tests: text(document, 'tests')
    };
  }

  function auth(document, mode) {
    const block = findBlock(document, 'auth:' + mode);
    const value = (key) => {
      const pair = pairsOf(block).find((p) => p.key === key);
      return pair ? pairText(pair.value) : '';
    };
    switch (mode) {
      case 'none': return { type: 'None' };
      case 'inherit': return { type: 'Inherit' };
      case 'basic': return { type: 'Basic', username: value('username'), password: value('password') };
      case 'bearer': return { type: 'Bearer', token: value('token') };
      case 'digest': return { type: 'Digest', username: value('username'), password: value('password') };
      case 'apikey': return { type: 'ApiKey', key: value('key'), value: value('value'), placement: value('placement') === 'queryparams' ? 'QueryParams' : 'Header' };
      default: return { type: 'Unsupported', mode };
    }
  }

  function splitContentType(value) {
    const start = value.indexOf('@contentType(');
    if (start !== -1) {
      const tail = trimEnd(value.slice(start));
      if (tail.startsWith('@contentType(') && tail.endsWith(')') && tail.length >= '@contentType()'.length) {
        return [trimEnd(value.slice(0, start)), trim(tail.slice('@contentType('.length, -1))];
      }
    }
    return [value, null];
  }

  const fileReference = (value) => (value.startsWith('@file(') && value.endsWith(')') && value.length >= '@file()'.length ? value.slice(6, -1) : null);

  function body(document, mode) {
    const t = (name) => text(document, name) || '';
    switch (mode) {
      case 'none': return { type: 'None' };
      case 'json': return { type: 'Json', value: t(findBlock(document, 'body:json') ? 'body:json' : 'body') };
      case 'text': return { type: 'Text', value: t('body:text') };
      case 'xml': return { type: 'Xml', value: t('body:xml') };
      case 'sparql': return { type: 'Sparql', value: t('body:sparql') };
      case 'graphql': return { type: 'Graphql', query: t('body:graphql'), variables: t('body:graphql:vars') };
      case 'formUrlEncoded': return { type: 'FormUrlEncoded', value: keyValues(findBlock(document, 'body:form-urlencoded')) };
      case 'multipartForm': return {
        type: 'MultipartForm',
        value: keyValues(findBlock(document, 'body:multipart-form')).map((p) => {
          const [value, contentType] = splitContentType(p.value);
          const files = fileReference(value);
          return {
            name: p.name,
            value: files === null ? { type: 'Text', value } : { type: 'Files', value: files.split('|').filter((f) => f.length) },
            content_type: contentType,
            enabled: p.enabled
          };
        })
      };
      case 'file': return {
        type: 'File',
        value: keyValues(findBlock(document, 'body:file')).flatMap((p) => {
          const [value, contentType] = splitContentType(p.value);
          const path = fileReference(value);
          return path === null ? [] : [{ path, content_type: contentType, selected: p.enabled }];
        })
      };
      default: throw new Error('unknown body type `' + mode + '`');
    }
  }

  function documentToRequest(document) {
    const meta = findBlock(document, 'meta');
    const metaValue = (key) => get(meta, key);
    const type = metaValue('type') === null ? 'http' : metaValue('type');
    let kind = 'Http';
    if (type === 'graphql') kind = 'Graphql';
    else if (['grpc', 'ws', 'app'].includes(type)) throw new Error('`' + type + '` requests are not supported');

    let method = null;
    let request = null;
    for (const block of document.blocks) {
      if (block.name === 'http') { method = get(block, 'method') || ''; request = block; break; }
      if (METHODS.includes(block.name)) { method = block.name; request = block; break; }
    }
    if (!request) throw new Error('no request block (get, post, put...) found');
    if (!/^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/.test(method)) throw new Error('invalid http method `' + method + '`');

    const tagsPair = pairsOf(meta).find((p) => p.key === 'tags');
    const tags = !tagsPair ? [] : tagsPair.value.type === 'List' ? tagsPair.value.value : tagsPair.value.value.length ? [tagsPair.value.value] : [];
    const seq = metaValue('seq') === null ? null : parseF64(metaValue('seq'));
    const params = [['query', 'Query'], ['params:query', 'Query'], ['params:path', 'Path']].flatMap(([name, kind]) =>
      keyValues(findBlock(document, name)).map((p) => ({ name: p.name, value: p.value, enabled: p.enabled, kind })));
    const settingsBlock = findBlock(document, 'settings');
    const boolean = (key) => (get(settingsBlock, key) === null ? null : get(settingsBlock, key) === 'true');
    const number = (key, max) => (get(settingsBlock, key) === null ? null : parseUnsigned(get(settingsBlock, key), max));

    return {
      name: metaValue('name') || '',
      kind,
      seq: seq === null ? 1 : seq,
      tags,
      method: method.toUpperCase(),
      url: get(request, 'url') || '',
      params,
      headers: keyValues(findBlock(document, 'headers')),
      auth: auth(document, get(request, 'auth') === null ? 'none' : get(request, 'auth')),
      body: body(document, get(request, 'body') === null ? 'none' : get(request, 'body')),
      vars: vars(document),
      assertions: keyValues(findBlock(document, 'assert')),
      scripts: scripts(document),
      settings: {
        encode_url: boolean('encodeUrl'),
        timeout: number('timeout', 18446744073709551615n),
        follow_redirects: boolean('followRedirects'),
        max_redirects: number('maxRedirects', 4294967295n)
      },
      docs: text(document, 'docs')
    };
  }

  function documentToDefaults(document) {
    const meta = findBlock(document, 'meta');
    const authBlock = findBlock(document, 'auth');
    const mode = get(authBlock, 'mode');
    const seq = get(meta, 'seq') === null ? null : parseF64(get(meta, 'seq'));
    return {
      name: get(meta, 'name'),
      seq,
      headers: keyValues(findBlock(document, 'headers')),
      auth: auth(document, mode === null ? 'none' : mode),
      vars: vars(document),
      scripts: scripts(document),
      docs: text(document, 'docs')
    };
  }

  function documentToEnvironment(name, document) {
    const variables = keyValues(findBlock(document, 'vars')).map((p) => ({ name: p.name, value: p.value, enabled: p.enabled, secret: false }));
    const secret = findBlock(document, 'vars:secret');
    if (secret && secret.content.type === 'Array') {
      for (const n of secret.content.value) {
        variables.push({ name: n.startsWith('~') ? n.slice(1) : n, value: '', enabled: !n.startsWith('~'), secret: true });
      }
    }
    const extendsBlock = findBlock(document, 'extends');
    const ext = !extendsBlock ? [] : extendsBlock.content.type === 'Inline' ? [extendsBlock.content.value] : extendsBlock.content.type === 'Array' ? extendsBlock.content.value : [];
    return { name, variables, extends: ext };
  }

  const api = { parse, documentToRequest, documentToDefaults, documentToEnvironment, isTextBlock, isDictionaryBlock };
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
  else global.BruParser = api;
})(this);
