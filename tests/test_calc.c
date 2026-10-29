/* Native tests for the calculator state machine, keypad hit-testing and paint. */
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "calc.h"

static void press_text(calc_state_t *c, const char *keys) {
    for (const char *p = keys; *p; p++) {
        int key;
        if (*p >= '0' && *p <= '9') {
            key = CALC_KEY_DIGIT(*p - '0');
        } else {
            switch (*p) {
                case '+': key = CALC_KEY_PLUS; break;
                case '-': key = CALC_KEY_MINUS; break;
                case '*': key = CALC_KEY_MUL; break;
                case '/': key = CALC_KEY_DIV; break;
                case '=': key = CALC_KEY_EQ; break;
                case '.': key = CALC_KEY_DOT; break;
                case 'c': key = CALC_KEY_CLEAR; break;
                case '<': key = CALC_KEY_BACK; break;
                default: key = -1; break;
            }
        }
        if (key >= 0) calc_key(c, key);
    }
}

static const char *display(calc_state_t *c) {
    static char buf[24];
    calc_display(c, buf, sizeof(buf));
    return buf;
}

#define EXPECT(keys, want)                                             \
    do {                                                               \
        calc_state_t s;                                                \
        calc_init(&s);                                                 \
        press_text(&s, keys);                                          \
        if (strcmp(display(&s), want) != 0) {                          \
            fprintf(stderr, "FAIL %-18s -> '%s', want '%s'\n",         \
                    keys, display(&s), want);                          \
            failures++;                                                \
        }                                                              \
    } while (0)

static int failures = 0;

static void test_arithmetic(void) {
    EXPECT("2+3=", "5");
    EXPECT("2+3*4=", "20");        /* immediate execution, not precedence */
    EXPECT("9-12=", "-3");
    EXPECT("7*8=", "56");
    EXPECT("10/4=", "2.5");
    EXPECT("1/3=", "0.333");
    EXPECT("0.1+0.2=", "0.3");
    EXPECT("1.5*2=", "3");
    EXPECT("2+3=", "5");
    /* Thousands separators: display only, the value stays raw. */
    EXPECT("1234", "1,234");
    EXPECT("1234.5", "1,234.5");
    EXPECT("1234567+1=", "1,234,568");
    EXPECT("999999999", "999,999,999");
    EXPECT("123", "123");
}

static void test_chaining(void) {
    calc_state_t c;
    calc_init(&c);
    press_text(&c, "2+3=");
    assert(strcmp(display(&c), "5") == 0);
    press_text(&c, "+4=");         /* result becomes the accumulator */
    assert(strcmp(display(&c), "9") == 0);
    press_text(&c, "*2=");
    assert(strcmp(display(&c), "18") == 0);

    /* A second operator replaces the pending one rather than chaining. */
    calc_init(&c);
    press_text(&c, "6+*2=");
    assert(strcmp(display(&c), "12") == 0);
}

static void test_entry_editing(void) {
    calc_state_t c;

    calc_init(&c);
    press_text(&c, "12");
    assert(strcmp(display(&c), "12") == 0);
    calc_key(&c, CALC_KEY_BACK);
    assert(strcmp(display(&c), "1") == 0);
    calc_key(&c, CALC_KEY_BACK);
    assert(strcmp(display(&c), "0") == 0);   /* empty entry falls back to acc */
    assert(calc_key(&c, CALC_KEY_BACK) == 0); /* nothing left to delete */

    calc_init(&c);
    press_text(&c, ".5");
    assert(strcmp(display(&c), "0.5") == 0); /* leading zero is inserted */
    press_text(&c, ".");
    assert(strcmp(display(&c), "0.5") == 0); /* second dot ignored */

    calc_init(&c);
    press_text(&c, "12345678901234");
    assert(c.entry_len == 10);                /* entry is capped */
    assert(strcmp(display(&c), "1,234,567,890") == 0);

    calc_init(&c);
    press_text(&c, "12+34c");
    assert(strcmp(display(&c), "0") == 0);   /* AC clears everything */
    assert(c.acc == 0.0 && c.pending == 0);
}

static void test_errors(void) {
    calc_state_t c;

    calc_init(&c);
    press_text(&c, "5/0=");
    assert(strcmp(display(&c), "E") == 0);

    /* Any key after an error starts a fresh calculation. */
    press_text(&c, "7");
    assert(strcmp(display(&c), "7") == 0);
    assert(c.error == 0);

    calc_init(&c);
    press_text(&c, "999999999*9=");
    assert(strcmp(display(&c), "E") == 0);

    calc_init(&c);
    press_text(&c, "c");
    assert(strcmp(display(&c), "0") == 0);
}

static void test_keypad_hit_testing(void) {
    calc_state_t c;

    /* "7" sits in row 1, column 0. */
    calc_init(&c);
    assert(calc_click(&c, 190 + 108, 174 + 78 + 33) == 1);
    assert(strcmp(display(&c), "7") == 0);

    /* "=" spans rows 3-4 in column 3. */
    press_text(&c, "+3");
    assert(calc_click(&c, 190 + 3 * 228 + 108, 408 + 72) == 1);
    assert(strcmp(display(&c), "10") == 0);

    /* The period key is in row 4, column 2, next to the wide zero. */
    calc_init(&c);
    assert(calc_click(&c, 190 + 108, 174 + 3 * 78 + 33) == 1);            /* 1 */
    assert(calc_click(&c, 190 + 2 * 228 + 108, 174 + 4 * 78 + 33) == 1);  /* . */
    assert(calc_click(&c, 190 + 1 * 228 + 108, 174 + 2 * 78 + 33) == 1);  /* 5 */
    assert(strcmp(display(&c), "1.5") == 0);

    /* The display area and the gaps are not buttons. */
    calc_init(&c);
    assert(calc_click(&c, 10, 400) == 0);  /* left of the keypad */
    assert(calc_click(&c, 300, 110) == 0); /* readout panel */
    assert(calc_click(&c, 640, 650) == 0); /* thumbnail strip */
}

static void test_draw(void) {
    fb_t fb;
    fb.w = 1280;
    fb.h = 720;
    fb.px = (uint32_t *)calloc((size_t)fb.w * fb.h, 4);
    assert(fb.px != NULL);

    const uint32_t SENTINEL = RGBA(255, 0, 255, 255);
    fb_clear(&fb, SENTINEL);

    calc_state_t c;
    calc_init(&c);
    press_text(&c, "12.5");

    calc_draw(&fb, &c);

    /* Pixels changed inside the calculator screen ... */
    size_t changed = 0;
    for (int y = 0; y < CALC_SCREEN_H; y++) {
        for (int x = 0; x < 1280; x++) {
            if (fb.px[(size_t)y * 1280 + x] != SENTINEL) changed++;
        }
    }
    assert(changed > 10000);

    /* ... and the calculator paints only its panel and keys, no backdrop. */
    assert(fb.px[0] == SENTINEL);
    assert(fb.px[(size_t)719 * 1280 + 1279] == SENTINEL);

    /* NULL tolerance. */
    calc_draw(NULL, &c);
    calc_draw(&fb, NULL);

    free(fb.px);
}

static void test_tile_preview(void) {
    const uint32_t SENTINEL = RGBA(0, 0, 0, 255);
    fb_t fb;
    fb.w = 1280;
    fb.h = 720;
    fb.px = (uint32_t *)calloc((size_t)fb.w * fb.h, 4);
    assert(fb.px != NULL);

    fb_clear(&fb, SENTINEL);

    calc_state_t c;
    calc_init(&c);
    press_text(&c, "42/4=");           /* 10.5 */
    calc_draw_tile(&fb, &c, 288, 620, 128, 72);

    size_t inside = 0;
    for (int y = 620; y < 692; y++) {
        for (int x = 288; x < 416; x++) {
            if (fb.px[(size_t)y * 1280 + x] != SENTINEL) inside++;
        }
    }
    assert(inside > 100);
    assert(fb.px[0] == SENTINEL);
    assert(fb.px[(size_t)300 * 1280 + 300] == SENTINEL);
    assert(fb.px[(size_t)700 * 1280 + 640] == SENTINEL);

    free(fb.px);
}

int main(void) {
    test_arithmetic();
    test_chaining();
    test_entry_editing();
    test_errors();
    test_keypad_hit_testing();
    test_draw();
    test_tile_preview();

    if (failures) {
        fprintf(stderr, "%d check(s) failed\n", failures);
        return 1;
    }
    printf("test_calc: ok\n");
    return 0;
}
