#include "code.h"

#include <ctype.h>
#include <stdio.h>
#include <string.h>

#include "lcd.h"

#define COL_PLAIN RGB(0xe4, 0xec, 0xf2)
#define COL_KEYWORD RGB(0x4c, 0xc2, 0xff)
#define COL_TYPE RGB(0xc9, 0xa0, 0xff)
#define COL_STRING RGB(0x8f, 0xd8, 0xa8)
#define COL_NUMBER RGB(0xf0, 0xc6, 0x74)
#define COL_COMMENT RGB(0x6d, 0x7d, 0x8c)
#define COL_PUNCT RGB(0x9a, 0xa7, 0xb4)
#define COL_TAG RGB(0x4c, 0xc2, 0xff)
#define COL_ATTR RGB(0xf0, 0xc6, 0x74)
#define COL_PREPROC RGB(0xf0, 0xb8, 0x6a)
#define COL_GUTTER RGB(0x4f, 0x5d, 0x68)
#define COL_TRACK RGB(0x1a, 0x22, 0x2c)
#define COL_THUMB RGB(0x3f, 0x4e, 0x5e)
#define COL_LINE RGB(0x2a, 0x36, 0x44)

/* --------------------------------------------------------------- language -- */

static const char *KW_JS[] = {
    "const", "let", "var", "function", "return", "if", "else", "for", "while",
    "class", "new", "this", "import", "export", "from", "default", "async",
    "await", "try", "catch", "throw", "typeof", "null", "true", "false",
    "undefined", "of", "in"
};
static const char *KW_TS[] = {
    "interface", "type", "as", "extends", "implements", "readonly", "public",
    "private", "enum", "string", "number", "boolean", "void"
};
static const char *KW_PY[] = {
    "def", "class", "return", "if", "elif", "else", "for", "while", "in",
    "not", "and", "or", "import", "from", "as", "with", "try", "except",
    "raise", "yield", "lambda", "None", "True", "False", "self", "print",
    "pass", "global"
};
static const char *KW_C[] = {
    "return", "if", "else", "for", "while", "sizeof", "typedef", "struct",
    "static", "const", "switch", "case", "break", "continue", "goto",
    "union", "enum", "extern", "volatile", "printf", "include", "define"
};
static const char *TY_C[] = {
    "int", "char", "void", "float", "double", "long", "short", "unsigned",
    "signed", "size_t", "uint8_t", "uint32_t", "int64_t", "FILE"
};
static const char *KW_JSON[] = { "true", "false", "null" };
static const char *TY_PY[] = { "str", "int", "float", "list", "dict", "bool" };

typedef struct {
    const char **words;
    int count;
} word_list_t;

static word_list_t keywords_for(code_lang_t lang) {
    switch (lang) {
        case LANG_JS: return (word_list_t){ KW_JS, (int)(sizeof(KW_JS) / sizeof(KW_JS[0])) };
        case LANG_TS: return (word_list_t){ KW_TS, (int)(sizeof(KW_TS) / sizeof(KW_TS[0])) };
        case LANG_PY: return (word_list_t){ KW_PY, (int)(sizeof(KW_PY) / sizeof(KW_PY[0])) };
        case LANG_C: return (word_list_t){ KW_C, (int)(sizeof(KW_C) / sizeof(KW_C[0])) };
        case LANG_JSON: return (word_list_t){ KW_JSON, (int)(sizeof(KW_JSON) / sizeof(KW_JSON[0])) };
        default: return (word_list_t){ NULL, 0 };
    }
}

static word_list_t types_for(code_lang_t lang) {
    switch (lang) {
        case LANG_C: return (word_list_t){ TY_C, (int)(sizeof(TY_C) / sizeof(TY_C[0])) };
        case LANG_PY: return (word_list_t){ TY_PY, (int)(sizeof(TY_PY) / sizeof(TY_PY[0])) };
        case LANG_TS: return (word_list_t){ KW_TS, 0 }; /* TS types live in keywords */
        default: return (word_list_t){ NULL, 0 };
    }
}

static int word_in(const char *w, int n, word_list_t list) {
    for (int i = 0; i < list.count; i++) {
        if ((int)strlen(list.words[i]) == n && strncmp(list.words[i], w, n) == 0) return 1;
    }
    return 0;
}

code_lang_t code_lang_for(const char *filename) {
    if (!filename) return LANG_PLAIN;
    const char *dot = strrchr(filename, '.');
    if (!dot) return LANG_PLAIN;

    struct { const char *ext; code_lang_t lang; } k_map[] = {
        { "c", LANG_C }, { "h", LANG_C },
        { "js", LANG_JS }, { "mjs", LANG_JS },
        { "ts", LANG_TS }, { "tsx", LANG_TS },
        { "py", LANG_PY },
        { "json", LANG_JSON },
        { "html", LANG_HTML }, { "htm", LANG_HTML },
        { "xml", LANG_XML }, { "svg", LANG_XML },
        { "css", LANG_CSS }
    };
    for (int i = 0; i < (int)(sizeof(k_map) / sizeof(k_map[0])); i++) {
        if (strcmp(dot + 1, k_map[i].ext) == 0) return k_map[i].lang;
    }
    return LANG_PLAIN;
}

const char *code_lang_name(code_lang_t lang) {
    switch (lang) {
        case LANG_C: return "c";
        case LANG_JS: return "javascript";
        case LANG_TS: return "typescript";
        case LANG_PY: return "python";
        case LANG_JSON: return "json";
        case LANG_HTML: return "html";
        case LANG_XML: return "xml";
        case LANG_CSS: return "css";
        default: return "text";
    }
}

/* -------------------------------------------------------------- tokenizers -- */

static int is_word_start(char c) { return isalpha((unsigned char)c) || c == '_'; }
static int is_word_char(char c) { return isalnum((unsigned char)c) || c == '_'; }

static int emit(code_token_t *out, int max, int *n, int start, int len, int kind) {
    if (len <= 0 || *n >= max) return 0;
    out[*n].start = start;
    out[*n].len = len;
    out[*n].kind = kind;
    (*n)++;
    return 1;
}

static int lex_clike(code_lang_t lang, const char *s, int n,
                     code_token_t *out, int max) {
    const int py = (lang == LANG_PY);
    const int json = (lang == LANG_JSON);
    int count = 0;
    int i = 0;

    while (i < n) {
        /* Preprocessor / shebang style lines. */
        if (lang == LANG_C && s[i] == '#') {
            emit(out, max, &count, i, n - i, TK_PREPROC);
            break;
        }
        if (py && s[i] == '#') {
            emit(out, max, &count, i, n - i, TK_COMMENT);
            break;
        }
        if (!py && !json && s[i] == '/' && i + 1 < n && s[i + 1] == '/') {
            emit(out, max, &count, i, n - i, TK_COMMENT);
            break;
        }
        if (!py && !json && s[i] == '/' && i + 1 < n && s[i + 1] == '*') {
            int end = i + 2;
            while (end + 1 < n && !(s[end] == '*' && s[end + 1] == '/')) end++;
            end = (end + 1 < n) ? end + 2 : n;
            emit(out, max, &count, i, end - i, TK_COMMENT);
            i = end;
            continue;
        }
        if (s[i] == '"' || s[i] == '\'' || (s[i] == '`' && (lang == LANG_JS || lang == LANG_TS))) {
            const char quote = s[i];
            int end = i + 1;
            while (end < n && s[end] != quote) {
                if (s[end] == '\\') end++;
                end++;
            }
            if (end < n) end++;
            emit(out, max, &count, i, end - i, TK_STRING);
            i = end;
            continue;
        }
        if (isdigit((unsigned char)s[i])) {
            int end = i;
            while (end < n && (isalnum((unsigned char)s[end]) || s[end] == '.')) end++;
            emit(out, max, &count, i, end - i, TK_NUMBER);
            i = end;
            continue;
        }
        if (is_word_start(s[i])) {
            int end = i;
            while (end < n && is_word_char(s[end])) end++;
            int kind = TK_PLAIN;
            if (word_in(s + i, end - i, types_for(lang))) kind = TK_TYPE;
            else if (word_in(s + i, end - i, keywords_for(lang))) kind = TK_KEYWORD;
            emit(out, max, &count, i, end - i, kind);
            i = end;
            continue;
        }
        {
            const char c = s[i];
            const int punct = strchr("{}[]()<>+-*/%=&|!?:;,.#@$^~`\\", c) != NULL;
            emit(out, max, &count, i, 1, punct ? TK_PUNCT : TK_PLAIN);
        }
        i++;
    }
    return count;
}

static int lex_markup(const char *s, int n, code_token_t *out, int max) {
    int count = 0;
    int i = 0;

    while (i < n) {
        if (s[i] != '<') {
            int start = i;
            while (i < n && s[i] != '<') i++;
            emit(out, max, &count, start, i - start, TK_PLAIN);
            continue;
        }

        if (i + 3 < n && s[i + 1] == '!' && s[i + 2] == '-' && s[i + 3] == '-') {
            int end = i + 4;
            while (end + 2 < n && !(s[end] == '-' && s[end + 1] == '-')) end++;
            end = (end + 2 < n) ? end + 3 : n;
            emit(out, max, &count, i, end - i, TK_COMMENT);
            i = end;
            continue;
        }

        /* <tag attr="value" ...> or </tag> */
        emit(out, max, &count, i, 1, TK_PUNCT);
        i++;
        int slash = 0;
        if (i < n && s[i] == '/') { slash = 1; emit(out, max, &count, i, 1, TK_PUNCT); i++; }

        int name_start = i;
        while (i < n && (isalnum((unsigned char)s[i]) || s[i] == '-' || s[i] == '_' || s[i] == ':')) i++;
        emit(out, max, &count, name_start, i - name_start, TK_TAG);
        (void)slash;

        while (i < n && s[i] != '>') {
            if (isspace((unsigned char)s[i])) {
                int start = i;
                while (i < n && isspace((unsigned char)s[i])) i++;
                emit(out, max, &count, start, i - start, TK_PLAIN);
            } else if (s[i] == '"' || s[i] == '\'') {
                const char quote = s[i];
                int end = i + 1;
                while (end < n && s[end] != quote) end++;
                if (end < n) end++;
                emit(out, max, &count, i, end - i, TK_STRING);
                i = end;
            } else if (s[i] == '=') {
                emit(out, max, &count, i, 1, TK_PUNCT);
                i++;
            } else {
                int start = i;
                while (i < n && !isspace((unsigned char)s[i]) && s[i] != '=' &&
                       s[i] != '"' && s[i] != '\'' && s[i] != '>') i++;
                emit(out, max, &count, start, i - start, TK_ATTR);
            }
        }
        if (i < n && s[i] == '>') {
            emit(out, max, &count, i, 1, TK_PUNCT);
            i++;
        }
    }
    return count;
}

static int lex_css(const char *s, int n, code_token_t *out, int max) {
    int count = 0;
    int i = 0;

    while (i < n) {
        if (s[i] == '/' && i + 1 < n && s[i + 1] == '*') {
            int end = i + 2;
            while (end + 1 < n && !(s[end] == '*' && s[end + 1] == '/')) end++;
            end = (end + 1 < n) ? end + 2 : n;
            emit(out, max, &count, i, end - i, TK_COMMENT);
            i = end;
            continue;
        }
        if (s[i] == '"' || s[i] == '\'') {
            const char quote = s[i];
            int end = i + 1;
            while (end < n && s[end] != quote) end++;
            if (end < n) end++;
            emit(out, max, &count, i, end - i, TK_STRING);
            i = end;
            continue;
        }
        if (s[i] == '@') {
            int end = i + 1;
            while (end < n && is_word_char(s[end])) end++;
            emit(out, max, &count, i, end - i, TK_KEYWORD);
            i = end;
            continue;
        }
        if (s[i] == '#' && i + 1 < n && isxdigit((unsigned char)s[i + 1])) {
            int end = i + 1;
            while (end < n && isxdigit((unsigned char)s[end])) end++;
            emit(out, max, &count, i, end - i, TK_NUMBER);
            i = end;
            continue;
        }
        if (isdigit((unsigned char)s[i])) {
            int end = i;
            while (end < n && (isalnum((unsigned char)s[end]) || s[end] == '.')) end++;
            emit(out, max, &count, i, end - i, TK_NUMBER);
            i = end;
            continue;
        }
        if (is_word_start(s[i]) || s[i] == '-') {
            int end = i;
            while (end < n && (is_word_char(s[end]) || s[end] == '-')) end++;
            /* property name if the next non-space char is ':' */
            int probe = end;
            while (probe < n && s[probe] == ' ') probe++;
            const int is_prop = (probe < n && s[probe] == ':');
            emit(out, max, &count, i, end - i, is_prop ? TK_ATTR : TK_PLAIN);
            i = end;
            continue;
        }
        {
            const char c = s[i];
            const int punct = strchr("{}[]()<>=:;,.*+~/!", c) != NULL;
            emit(out, max, &count, i, 1, punct ? TK_PUNCT : TK_PLAIN);
        }
        i++;
    }
    return count;
}

int code_tokenize(code_lang_t lang, const char *line, int len,
                  code_token_t *out, int max) {
    if (!line || len <= 0 || !out || max <= 0) return 0;
    switch (lang) {
        case LANG_HTML:
        case LANG_XML:
            return lex_markup(line, len, out, max);
        case LANG_CSS:
            return lex_css(line, len, out, max);
        case LANG_C:
        case LANG_JS:
        case LANG_TS:
        case LANG_PY:
        case LANG_JSON:
            return lex_clike(lang, line, len, out, max);
        default: {
            int n = 0;
            emit(out, max, &n, 0, len, TK_PLAIN);
            return n;
        }
    }
}

/* ---------------------------------------------------------------- drawing -- */

static uint32_t colour_for(int kind) {
    switch (kind) {
        case TK_KEYWORD: return COL_KEYWORD;
        case TK_TYPE: return COL_TYPE;
        case TK_STRING: return COL_STRING;
        case TK_NUMBER: return COL_NUMBER;
        case TK_COMMENT: return COL_COMMENT;
        case TK_PUNCT: return COL_PUNCT;
        case TK_TAG: return COL_TAG;
        case TK_ATTR: return COL_ATTR;
        case TK_PREPROC: return COL_PREPROC;
        default: return COL_PLAIN;
    }
}

/* Tabs become four spaces; also gives the tokenizer a stable buffer. */
static int expand_line(const char *src, int len, char *dst, int cap) {
    int o = 0;
    for (int i = 0; i < len && o < cap - 1; i++) {
        if (src[i] == '\t') {
            int pad = 4 - (o % 4);
            while (pad-- > 0 && o < cap - 1) dst[o++] = ' ';
        } else if (src[i] == '\r') {
            continue;
        } else {
            dst[o++] = src[i];
        }
    }
    dst[o] = '\0';
    return o;
}

int code_line_count(const char *text) {
    if (!text) return 0;
    int n = 1;
    for (const char *p = text; *p; p++) {
        if (*p == '\n' && p[1] != '\0') n++;
    }
    return n;
}

void code_draw(fb_t *fb, const char *text, int x, int y, int w, int h,
               int cell, code_lang_t lang, int *line_off, int *col_off,
               int *max_line_off, int *max_col_off) {
    if (!fb || !fb->px || !text || cell <= 0) return;

    const int line_h = 7 * cell + 8;
    const int pad = 14;
    const int advance = 6 * cell;
    const int gutter_cell = cell * 3 / 4 < 2 ? 2 : cell * 3 / 4;

    /* Count lines and the widest expanded line, so scrolling can be clamped. */
    int lines = 1;
    int widest = 0;
    int cur = 0;
    for (const char *p = text; ; p++) {
        if (*p == '\n' || *p == '\0') {
            if (cur > widest) widest = cur;
            cur = 0;
            if (*p == '\0') break;
            lines++;
        } else {
            cur += (*p == '\t') ? 4 : 1;
        }
    }
    if (lines < 1) lines = 1;

    const int digits = snprintf(NULL, 0, "%d", lines);
    const int gutter = digits * 6 * gutter_cell + 18;
    const int text_x = x + pad + gutter;
    const int visible = (h - 2 * pad) / line_h;
    const int max_cols = (w - pad - gutter - pad) / advance;

    if (line_off) {
        if (*line_off > lines - visible) *line_off = lines - visible;
        if (*line_off < 0) *line_off = 0;
    }
    if (col_off) {
        if (*col_off > widest - max_cols) *col_off = widest - max_cols;
        if (*col_off < 0) *col_off = 0;
    }
    const int loff = line_off ? *line_off : 0;
    const int coff = col_off ? *col_off : 0;

    /* Report the limits so the caller can decide whether a key should scroll
     * the text or fall through to something else (e.g. tile navigation). */
    if (max_line_off) *max_line_off = lines - visible > 0 ? lines - visible : 0;
    if (max_col_off) *max_col_off = widest - max_cols > 0 ? widest - max_cols : 0;

    /* Gutter divider. */
    fb_fill(fb, x + pad + gutter - 10, y + pad, 1, h - 2 * pad, COL_LINE);

    char buf[256];
    const char *p = text;
    int line_no = 0;
    int drawn = 0;

    while (*p && drawn < visible) {
        const char *eol = strchr(p, '\n');
        int len = eol ? (int)(eol - p) : (int)strlen(p);

        if (line_no >= loff) {
            const int ly = y + pad + drawn * line_h;

            char num[12];
            snprintf(num, sizeof(num), "%d", line_no + 1);
            const int nw = lcd_text5x7_width(num, gutter_cell);
            lcd_draw_text5x7(fb, num, x + pad + gutter - 18 - nw, ly, gutter_cell,
                             COL_GUTTER);

            const int elen = expand_line(p, len, buf, sizeof(buf));
            code_token_t toks[96];
            const int nt = code_tokenize(lang, buf, elen, toks, 96);

            int cx = text_x;
            int cols = 0;
            for (int t = 0; t < nt && cols < coff + max_cols; t++) {
                int start = toks[t].start;
                int tlen = toks[t].len;

                if (start + tlen <= coff) continue;
                if (start < coff) {
                    tlen -= (coff - start);
                    start = coff;
                }
                cx = text_x + (start - coff) * advance;

                int room = max_cols - (start - coff);
                if (room <= 0) break;
                if (tlen > room) tlen = room;

                lcd_draw_text5x7_ex(fb, buf + start, tlen, cx, ly, cell,
                                    colour_for(toks[t].kind), 0, 0);
                cols = start + tlen;
            }
            drawn++;
        }

        line_no++;
        if (!eol) break;
        p = eol + 1;
    }

    /* Scrollbars: only when there is something to scroll to. */
    const int bar = 7;
    if (lines > visible) {
        const int track_x = x + w - bar;
        const int track_h = h - 2 * pad - (widest > max_cols ? bar + 4 : 0);
        int thumb_h = track_h * visible / lines;
        if (thumb_h < 24) thumb_h = 24;
        if (thumb_h > track_h) thumb_h = track_h;
        const int max_line = lines - visible;
        const int thumb_y = y + pad +
            (max_line > 0 ? (track_h - thumb_h) * loff / max_line : 0);
        fb_fill(fb, track_x, y + pad, bar, track_h, COL_TRACK);
        fb_fill(fb, track_x, thumb_y, bar, thumb_h, COL_THUMB);
    }
    if (widest > max_cols) {
        const int track_y = y + h - bar;
        const int track_x = text_x;
        const int track_w = w - (text_x - x) - pad - (lines > visible ? bar + 4 : 0);
        int thumb_w = track_w * max_cols / widest;
        if (thumb_w < 30) thumb_w = 30;
        if (thumb_w > track_w) thumb_w = track_w;
        const int max_col = widest - max_cols;
        const int thumb_x = track_x +
            (max_col > 0 ? (track_w - thumb_w) * coff / max_col : 0);
        fb_fill(fb, track_x, track_y, track_w, bar, COL_TRACK);
        fb_fill(fb, thumb_x, track_y, thumb_w, bar, COL_THUMB);
    }
}
