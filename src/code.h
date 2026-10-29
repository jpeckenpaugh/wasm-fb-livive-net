/* Source-code viewer with a small per-family highlighter.
 *
 * Three scanners cover eight languages:
 *   C-like  -> C, JS, TS, JSON, Python (keyword tables + comment markers)
 *   markup  -> HTML, XML (tags, attributes, strings, comments)
 *   css     -> comments, strings, @rules, #hex, numbers, property names
 *
 * Tokens are produced line by line, which keeps the scanner stateless and
 * cheap: only visible lines are ever examined.
 */
#ifndef CODE_H
#define CODE_H

#include "framebuffer.h"

typedef enum {
    LANG_PLAIN = 0,
    LANG_C,
    LANG_JS,
    LANG_TS,
    LANG_PY,
    LANG_JSON,
    LANG_HTML,
    LANG_XML,
    LANG_CSS
} code_lang_t;

/* Token classes; the renderer maps these to colours. */
typedef enum {
    TK_PLAIN = 0,
    TK_KEYWORD,
    TK_TYPE,
    TK_STRING,
    TK_NUMBER,
    TK_COMMENT,
    TK_PUNCT,
    TK_TAG,
    TK_ATTR,
    TK_PREPROC
} code_token_kind;

typedef struct {
    int start;
    int len;
    int kind;
} code_token_t;

/* Pick a language from a filename's extension. */
code_lang_t code_lang_for(const char *filename);
const char *code_lang_name(code_lang_t lang);

/* Split one (tab-expanded) line into at most `max` tokens. Returns the count. */
int code_tokenize(code_lang_t lang, const char *line, int len,
                  code_token_t *out, int max);

/* Number of lines in the text (1 for an empty string). */
int code_line_count(const char *text);

/* Draw the file with a gutter and scrollbars. Updates line_off/col_off to
 * clamped values so the caller can keep scroll state across frames. */
void code_draw(fb_t *fb, const char *text, int x, int y, int w, int h,
               int cell, code_lang_t lang, int *line_off, int *col_off);

#endif /* CODE_H */
