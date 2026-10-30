//! Syntax-Highlighted Source Code Preview (`code.rs`).
//!
//! Renders mounted source files (`.c`, `.js`, `.ts`, `.py`, `.json`, `.html`,
//! `.xml`, `.css`) directly into the Right 2/3 Preview Pane (`852 x 688`) of
//! the `Files` application with line numbers and token coloring.

use crate::fb::{rgb, Framebuffer};
use crate::font::{draw_str, text_width};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CodeLang {
    Plain,
    C,
    Js,
    Ts,
    Py,
    Json,
    Html,
    Xml,
    Css,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TokenKind {
    Plain,
    Keyword,
    Type,
    String,
    Number,
    Comment,
    Punct,
    Tag,
    Attr,
    Preproc,
}

#[derive(Clone, Copy)]
pub struct CodeToken {
    pub start: usize,
    pub len: usize,
    pub kind: TokenKind,
}

impl CodeToken {
    pub const fn empty() -> Self {
        Self {
            start: 0,
            len: 0,
            kind: TokenKind::Plain,
        }
    }
}

const KW_JS: &[&[u8]] = &[
    b"const", b"let", b"var", b"function", b"return", b"if", b"else", b"for", b"while",
    b"class", b"new", b"this", b"import", b"export", b"from", b"default", b"async",
    b"await", b"try", b"catch", b"throw", b"typeof", b"null", b"true", b"false",
    b"undefined", b"of", b"in", b"Boolean", b"String", b"Error",
];

const KW_TS: &[&[u8]] = &[
    b"const", b"let", b"function", b"return", b"if", b"else", b"for", b"while",
    b"export", b"import", b"from", b"interface", b"type", b"as", b"extends",
    b"implements", b"readonly", b"public", b"private", b"enum", b"of", b"in",
];

const TY_TS: &[&[u8]] = &[
    b"string", b"number", b"boolean", b"void", b"OrderStatus", b"LineItem", b"OrderSummary",
];

const KW_PY: &[&[u8]] = &[
    b"def", b"class", b"return", b"if", b"elif", b"else", b"for", b"while", b"in",
    b"not", b"and", b"or", b"import", b"from", b"as", b"with", b"try", b"except",
    b"raise", b"yield", b"lambda", b"None", b"True", b"False", b"self", b"print",
    b"pass", b"global",
];

const TY_PY: &[&[u8]] = &[
    b"str", b"int", b"float", b"list", b"dict", b"bool", b"Dict", b"List", b"Bucket", b"len",
];

const KW_C: &[&[u8]] = &[
    b"return", b"if", b"else", b"for", b"while", b"sizeof", b"typedef", b"struct",
    b"static", b"const", b"switch", b"case", b"break", b"continue", b"goto",
    b"union", b"enum", b"extern", b"volatile", b"printf",
];

const TY_C: &[&[u8]] = &[
    b"int", b"char", b"void", b"float", b"double", b"long", b"short", b"unsigned",
    b"signed", b"size_t", b"uint8_t", b"uint32_t", b"int64_t", b"line_item_t",
];

const KW_JSON: &[&[u8]] = &[b"true", b"false", b"null"];

pub fn lang_for_filename(name: &str) -> CodeLang {
    let bytes = name.as_bytes();
    let mut dot = None;
    let mut i = bytes.len();
    while i > 0 {
        i -= 1;
        if bytes[i] == b'.' {
            dot = Some(i + 1);
            break;
        }
    }
    let ext = match dot {
        Some(pos) => &bytes[pos..],
        None => return CodeLang::Plain,
    };
    match ext {
        b"c" | b"h" => CodeLang::C,
        b"js" | b"mjs" => CodeLang::Js,
        b"ts" | b"tsx" => CodeLang::Ts,
        b"py" => CodeLang::Py,
        b"json" => CodeLang::Json,
        b"html" | b"htm" => CodeLang::Html,
        b"xml" | b"svg" => CodeLang::Xml,
        b"css" => CodeLang::Css,
        _ => CodeLang::Plain,
    }
}

pub fn is_image_filename(name: &str) -> bool {
    let bytes = name.as_bytes();
    let mut dot = None;
    let mut i = bytes.len();
    while i > 0 {
        i -= 1;
        if bytes[i] == b'.' {
            dot = Some(i + 1);
            break;
        }
    }
    match dot.map(|pos| &bytes[pos..]) {
        Some(b"png" | b"jpg" | b"jpeg") => true,
        _ => false,
    }
}

fn word_in(word: &[u8], list: &[&[u8]]) -> bool {
    let mut i = 0;
    while i < list.len() {
        if list[i] == word {
            return true;
        }
        i += 1;
    }
    false
}

#[inline]
fn is_word_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

#[inline]
fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[inline]
fn emit(out: &mut [CodeToken], count: &mut usize, start: usize, len: usize, kind: TokenKind) {
    if len > 0 && *count < out.len() {
        out[*count] = CodeToken { start, len, kind };
        *count += 1;
    }
}

pub fn tokenize_line(lang: CodeLang, line: &[u8], out: &mut [CodeToken]) -> usize {
    if line.is_empty() || out.is_empty() {
        return 0;
    }
    match lang {
        CodeLang::Html | CodeLang::Xml => lex_markup(line, out),
        CodeLang::Css => lex_css(line, out),
        CodeLang::C | CodeLang::Js | CodeLang::Ts | CodeLang::Py | CodeLang::Json => {
            lex_clike(lang, line, out)
        }
        CodeLang::Plain => {
            let mut n = 0;
            emit(out, &mut n, 0, line.len(), TokenKind::Plain);
            n
        }
    }
}

fn lex_clike(lang: CodeLang, s: &[u8], out: &mut [CodeToken]) -> usize {
    let py = lang == CodeLang::Py;
    let json = lang == CodeLang::Json;
    let n = s.len();
    let mut count = 0;
    let mut i = 0;

    while i < n {
        if lang == CodeLang::C && s[i] == b'#' {
            emit(out, &mut count, i, n - i, TokenKind::Preproc);
            break;
        }
        if py && s[i] == b'#' {
            emit(out, &mut count, i, n - i, TokenKind::Comment);
            break;
        }
        if py && s[i] == b'@' {
            let mut end = i + 1;
            while end < n && is_word_char(s[end]) {
                end += 1;
            }
            emit(out, &mut count, i, end - i, TokenKind::Preproc);
            i = end;
            continue;
        }
        if !py && !json && s[i] == b'/' && i + 1 < n && s[i + 1] == b'/' {
            emit(out, &mut count, i, n - i, TokenKind::Comment);
            break;
        }
        if !py && !json && s[i] == b'/' && i + 1 < n && s[i + 1] == b'*' {
            let mut end = i + 2;
            while end + 1 < n && !(s[end] == b'*' && s[end + 1] == b'/') {
                end += 1;
            }
            end = if end + 1 < n { end + 2 } else { n };
            emit(out, &mut count, i, end - i, TokenKind::Comment);
            i = end;
            continue;
        }
        if s[i] == b'"'
            || s[i] == b'\''
            || (s[i] == b'`' && (lang == CodeLang::Js || lang == CodeLang::Ts))
        {
            let quote = s[i];
            let mut end = i + 1;
            while end < n && s[end] != quote {
                if s[end] == b'\\' {
                    end += 1;
                }
                end += 1;
            }
            if end < n {
                end += 1;
            }
            emit(out, &mut count, i, end - i, TokenKind::String);
            i = end;
            continue;
        }
        if s[i].is_ascii_digit() {
            let mut end = i;
            while end < n && (s[end].is_ascii_alphanumeric() || s[end] == b'.') {
                end += 1;
            }
            emit(out, &mut count, i, end - i, TokenKind::Number);
            i = end;
            continue;
        }
        if is_word_start(s[i]) {
            let mut end = i;
            while end < n && is_word_char(s[end]) {
                end += 1;
            }
            let w = &s[i..end];
            let kind = match lang {
                CodeLang::C => {
                    if word_in(w, TY_C) {
                        TokenKind::Type
                    } else if word_in(w, KW_C) {
                        TokenKind::Keyword
                    } else {
                        TokenKind::Plain
                    }
                }
                CodeLang::Js => {
                    if word_in(w, KW_JS) {
                        TokenKind::Keyword
                    } else {
                        TokenKind::Plain
                    }
                }
                CodeLang::Ts => {
                    if word_in(w, TY_TS) {
                        TokenKind::Type
                    } else if word_in(w, KW_TS) {
                        TokenKind::Keyword
                    } else {
                        TokenKind::Plain
                    }
                }
                CodeLang::Py => {
                    if word_in(w, TY_PY) {
                        TokenKind::Type
                    } else if word_in(w, KW_PY) {
                        TokenKind::Keyword
                    } else {
                        TokenKind::Plain
                    }
                }
                CodeLang::Json => {
                    if word_in(w, KW_JSON) {
                        TokenKind::Keyword
                    } else {
                        TokenKind::Plain
                    }
                }
                _ => TokenKind::Plain,
            };
            emit(out, &mut count, i, end - i, kind);
            i = end;
            continue;
        }
        let c = s[i];
        let is_punct = matches!(
            c,
            b'{' | b'}'
                | b'['
                | b']'
                | b'('
                | b')'
                | b'<'
                | b'>'
                | b'+'
                | b'-'
                | b'*'
                | b'/'
                | b'%'
                | b'='
                | b'&'
                | b'|'
                | b'!'
                | b'?'
                | b':'
                | b';'
                | b','
                | b'.'
        );
        emit(
            out,
            &mut count,
            i,
            1,
            if is_punct {
                TokenKind::Punct
            } else {
                TokenKind::Plain
            },
        );
        i += 1;
    }
    count
}

fn lex_markup(s: &[u8], out: &mut [CodeToken]) -> usize {
    let n = s.len();
    let mut count = 0;
    let mut i = 0;

    while i < n {
        if s[i] != b'<' {
            let start = i;
            while i < n && s[i] != b'<' {
                i += 1;
            }
            emit(out, &mut count, start, i - start, TokenKind::Plain);
            continue;
        }
        if i + 3 < n && s[i + 1] == b'!' && s[i + 2] == b'-' && s[i + 3] == b'-' {
            let mut end = i + 4;
            while end + 2 < n && !(s[end] == b'-' && s[end + 1] == b'-') {
                end += 1;
            }
            end = if end + 2 < n { end + 3 } else { n };
            emit(out, &mut count, i, end - i, TokenKind::Comment);
            i = end;
            continue;
        }
        emit(out, &mut count, i, 1, TokenKind::Punct);
        i += 1;
        if i < n && (s[i] == b'/' || s[i] == b'!' || s[i] == b'?') {
            emit(out, &mut count, i, 1, TokenKind::Punct);
            i += 1;
        }
        let name_start = i;
        while i < n
            && (s[i].is_ascii_alphanumeric() || s[i] == b'-' || s[i] == b'_' || s[i] == b':')
        {
            i += 1;
        }
        emit(out, &mut count, name_start, i - name_start, TokenKind::Tag);

        while i < n && s[i] != b'>' {
            if s[i].is_ascii_whitespace() {
                let start = i;
                while i < n && s[i].is_ascii_whitespace() {
                    i += 1;
                }
                emit(out, &mut count, start, i - start, TokenKind::Plain);
            } else if s[i] == b'"' || s[i] == b'\'' {
                let quote = s[i];
                let mut end = i + 1;
                while end < n && s[end] != quote {
                    end += 1;
                }
                if end < n {
                    end += 1;
                }
                emit(out, &mut count, i, end - i, TokenKind::String);
                i = end;
            } else if s[i] == b'=' || s[i] == b'/' || s[i] == b'?' {
                emit(out, &mut count, i, 1, TokenKind::Punct);
                i += 1;
            } else {
                let start = i;
                while i < n
                    && !s[i].is_ascii_whitespace()
                    && s[i] != b'='
                    && s[i] != b'"'
                    && s[i] != b'\''
                    && s[i] != b'>'
                {
                    i += 1;
                }
                emit(out, &mut count, start, i - start, TokenKind::Attr);
            }
        }
        if i < n && s[i] == b'>' {
            emit(out, &mut count, i, 1, TokenKind::Punct);
            i += 1;
        }
    }
    count
}

fn lex_css(s: &[u8], out: &mut [CodeToken]) -> usize {
    let n = s.len();
    let mut count = 0;
    let mut i = 0;

    while i < n {
        if s[i] == b'/' && i + 1 < n && s[i + 1] == b'*' {
            let mut end = i + 2;
            while end + 1 < n && !(s[end] == b'*' && s[end + 1] == b'/') {
                end += 1;
            }
            end = if end + 1 < n { end + 2 } else { n };
            emit(out, &mut count, i, end - i, TokenKind::Comment);
            i = end;
            continue;
        }
        if s[i] == b'"' || s[i] == b'\'' {
            let quote = s[i];
            let mut end = i + 1;
            while end < n && s[end] != quote {
                end += 1;
            }
            if end < n {
                end += 1;
            }
            emit(out, &mut count, i, end - i, TokenKind::String);
            i = end;
            continue;
        }
        if s[i] == b'@' {
            let mut end = i + 1;
            while end < n && is_word_char(s[end]) {
                end += 1;
            }
            emit(out, &mut count, i, end - i, TokenKind::Keyword);
            i = end;
            continue;
        }
        if s[i] == b'#' && i + 1 < n && s[i + 1].is_ascii_hexdigit() {
            let mut end = i + 1;
            while end < n && s[end].is_ascii_hexdigit() {
                end += 1;
            }
            emit(out, &mut count, i, end - i, TokenKind::Number);
            i = end;
            continue;
        }
        if s[i].is_ascii_digit() {
            let mut end = i;
            while end < n && (s[end].is_ascii_alphanumeric() || s[end] == b'.') {
                end += 1;
            }
            emit(out, &mut count, i, end - i, TokenKind::Number);
            i = end;
            continue;
        }
        if is_word_start(s[i]) || s[i] == b'-' {
            let mut end = i;
            while end < n && (is_word_char(s[end]) || s[end] == b'-') {
                end += 1;
            }
            let mut probe = end;
            while probe < n && s[probe] == b' ' {
                probe += 1;
            }
            let is_prop = probe < n && s[probe] == b':';
            emit(
                out,
                &mut count,
                i,
                end - i,
                if is_prop {
                    TokenKind::Attr
                } else {
                    TokenKind::Plain
                },
            );
            i = end;
            continue;
        }
        let c = s[i];
        let is_punct = matches!(
            c,
            b'{' | b'}' | b'(' | b')' | b':' | b';' | b',' | b'.' | b'*' | b'+'
        );
        emit(
            out,
            &mut count,
            i,
            1,
            if is_punct {
                TokenKind::Punct
            } else {
                TokenKind::Plain
            },
        );
        i += 1;
    }
    count
}

fn color_for(kind: TokenKind) -> u32 {
    match kind {
        TokenKind::Plain => rgb(0xe4, 0xec, 0xf2),
        TokenKind::Keyword => rgb(0x4c, 0xc2, 0xff),
        TokenKind::Type => rgb(0xc9, 0xa0, 0xff),
        TokenKind::String => rgb(0x8f, 0xd8, 0xa8),
        TokenKind::Number => rgb(0xf0, 0xc6, 0x74),
        TokenKind::Comment => rgb(0x6d, 0x7d, 0x8c),
        TokenKind::Punct => rgb(0x9a, 0xa7, 0xb4),
        TokenKind::Tag => rgb(0x4c, 0xc2, 0xff),
        TokenKind::Attr => rgb(0xf0, 0xc6, 0x74),
        TokenKind::Preproc => rgb(0xf0, 0xb8, 0x6a),
    }
}

fn expand_tabs(src: &[u8], dst: &mut [u8; 160]) -> usize {
    let mut o = 0;
    for &b in src {
        if o >= dst.len() {
            break;
        }
        if b == b'\t' {
            let pad = 4 - (o % 4);
            for _ in 0..pad {
                if o < dst.len() {
                    dst[o] = b' ';
                    o += 1;
                }
            }
        } else if b != b'\r' {
            dst[o] = b;
            o += 1;
        }
    }
    o
}

/// Render syntax-highlighted code into the region `(x, y, w, h)`.
pub fn draw_code_preview(
    fb: &mut Framebuffer,
    text: &[u8],
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    lang: CodeLang,
    scroll_line: usize,
) {
    let scale = 2;
    let char_adv = 6 * scale; // 12px per character at scale 2
    let line_h = 20;
    let pad_top = 20;
    let gutter_w = 56;
    let text_x = x + gutter_w + 16;
    let max_cols = ((w - gutter_w - 28).max(0) / char_adv) as usize;
    let visible_lines = ((h - 2 * pad_top).max(0) / line_h) as usize;

    // Subtle gutter divider line
    fb.fill(
        x + gutter_w,
        y + 12,
        1,
        h - 24,
        rgb(0x1e, 0x2b, 0x38),
    );

    let mut line_idx = 0usize;
    let mut drawn = 0usize;
    let mut start = 0usize;
    let n = text.len();

    while start <= n && drawn < visible_lines {
        let mut end = start;
        while end < n && text[end] != b'\n' {
            end += 1;
        }
        if start == n && n > 0 && text[n - 1] == b'\n' {
            break;
        }

        if line_idx >= scroll_line {
            let ly = y + pad_top + (drawn as i32) * line_h;

            // Line number in gutter
            let mut num_buf = [b' '; 8];
            let num_str = fmt_line_no(&mut num_buf, (line_idx + 1) as u32);
            let nw = text_width(num_str, scale);
            draw_str(
                fb,
                x + gutter_w - 12 - nw,
                ly,
                num_str,
                scale,
                rgb(0x52, 0x64, 0x74),
            );

            // Expand tabs and tokenize
            let mut expanded = [b' '; 160];
            let elen = expand_tabs(&text[start..end], &mut expanded);
            let mut toks = [CodeToken::empty(); 64];
            let nt = tokenize_line(lang, &expanded[..elen], &mut toks);

            for t in toks.iter().take(nt) {
                if t.start >= max_cols {
                    break;
                }
                let avail = max_cols - t.start;
                let tlen = t.len.min(avail);
                if let Ok(seg) = core::str::from_utf8(&expanded[t.start..t.start + tlen]) {
                    let tx = text_x + (t.start as i32) * char_adv;
                    draw_str(fb, tx, ly, seg, scale, color_for(t.kind));
                }
            }

            drawn += 1;
        }

        line_idx += 1;
        if end >= n {
            break;
        }
        start = end + 1;
    }
}

fn fmt_line_no(buf: &mut [u8; 8], mut val: u32) -> &str {
    if val == 0 {
        buf[0] = b'0';
        return core::str::from_utf8(&buf[..1]).unwrap_or("0");
    }
    let mut tmp = [0u8; 8];
    let mut n = 0;
    while val > 0 && n < tmp.len() {
        tmp[n] = b'0' + (val % 10) as u8;
        val /= 10;
        n += 1;
    }
    let mut pos = 0;
    while n > 0 {
        n -= 1;
        buf[pos] = tmp[n];
        pos += 1;
    }
    core::str::from_utf8(&buf[..pos]).unwrap_or("")
}
