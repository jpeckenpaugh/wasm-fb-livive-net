/* Native tests for language detection, tokenizing and code painting. */
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "code.h"

static int count_kind(const code_token_t *t, int n, int kind) {
    int c = 0;
    for (int i = 0; i < n; i++) if (t[i].kind == kind) c++;
    return c;
}

int main(void) {
    /* Extension mapping. */
    assert(code_lang_for("page.html") == LANG_HTML);
    assert(code_lang_for("app.js") == LANG_JS);
    assert(code_lang_for("types.ts") == LANG_TS);
    assert(code_lang_for("style.css") == LANG_CSS);
    assert(code_lang_for("data.json") == LANG_JSON);
    assert(code_lang_for("feed.xml") == LANG_XML);
    assert(code_lang_for("script.py") == LANG_PY);
    assert(code_lang_for("hello.c") == LANG_C);
    assert(code_lang_for("README") == LANG_PLAIN);
    assert(strcmp(code_lang_name(LANG_PY), "python") == 0);

    code_token_t toks[64];

    /* C: type, number, comment, preprocessor. */
    const char *c = "int x = 42; // note";
    int n = code_tokenize(LANG_C, c, (int)strlen(c), toks, 64);
    assert(n > 0);
    assert(count_kind(toks, n, TK_TYPE) == 1);
    assert(count_kind(toks, n, TK_NUMBER) == 1);
    assert(count_kind(toks, n, TK_COMMENT) == 1);

    const char *pp = "#include <stdio.h>";
    n = code_tokenize(LANG_C, pp, (int)strlen(pp), toks, 64);
    assert(n == 1 && toks[0].kind == TK_PREPROC);

    /* Python: keyword and comment. */
    const char *py = "def go(x):  # run";
    n = code_tokenize(LANG_PY, py, (int)strlen(py), toks, 64);
    assert(count_kind(toks, n, TK_KEYWORD) == 1);
    assert(count_kind(toks, n, TK_COMMENT) == 1);

    /* JS: string with a backtick, keyword. */
    const char *js = "const s = `hi`;";
    n = code_tokenize(LANG_JS, js, (int)strlen(js), toks, 64);
    assert(count_kind(toks, n, TK_KEYWORD) == 1);
    assert(count_kind(toks, n, TK_STRING) == 1);

    /* HTML: tag, attribute, quoted value. */
    const char *html = "<a href=\"x\">t</a>";
    n = code_tokenize(LANG_HTML, html, (int)strlen(html), toks, 64);
    assert(count_kind(toks, n, TK_TAG) == 2);
    assert(count_kind(toks, n, TK_ATTR) == 1);
    assert(count_kind(toks, n, TK_STRING) == 1);

    /* CSS: property, hex colour, at-rule. */
    const char *css = ".a { color: #fff; }";
    n = code_tokenize(LANG_CSS, css, (int)strlen(css), toks, 64);
    assert(count_kind(toks, n, TK_ATTR) == 1);
    assert(count_kind(toks, n, TK_NUMBER) == 1);

    const char *at = "@media screen { }";
    n = code_tokenize(LANG_CSS, at, (int)strlen(at), toks, 64);
    assert(count_kind(toks, n, TK_KEYWORD) == 1);

    /* JSON literals. */
    const char *json = "{ \"ok\": true }";
    n = code_tokenize(LANG_JSON, json, (int)strlen(json), toks, 64);
    assert(count_kind(toks, n, TK_KEYWORD) == 1);
    assert(count_kind(toks, n, TK_STRING) == 1);

    /* Painting: gutter + code inside the box only. */
    const uint32_t SENTINEL = RGBA(255, 0, 255, 255);
    fb_t fb;
    fb.w = 1280;
    fb.h = 720;
    fb.px = (uint32_t *)calloc((size_t)fb.w * fb.h, 4);
    assert(fb.px != NULL);
    fb_clear(&fb, SENTINEL);

    int lo = 0, co = 0, maxlo = 0, maxco = 0;
    code_draw(&fb, c, 40, 40, 600, 200, 4, LANG_C, &lo, &co, &maxlo, &maxco);
    assert(maxlo == 0 && maxco == 0); /* one short line: nothing to scroll */
    size_t changed = 0;
    for (int y = 40; y < 240; y++)
        for (int x = 40; x < 640; x++)
            if (fb.px[(size_t)y * 1280 + x] != SENTINEL) changed++;
    assert(changed > 100);
    assert(fb.px[0] == SENTINEL);

    free(fb.px);
    printf("test_code: ok\n");
    return 0;
}
