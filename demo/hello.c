#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

typedef struct {
    const char *sku;
    uint32_t unit_cents;
    uint32_t quantity;
} line_item_t;

static uint32_t subtotal_cents(const line_item_t *items, size_t n) {
    uint32_t sum = 0;
    for (size_t i = 0; i < n; i++) {
        sum += items[i].unit_cents * items[i].quantity;
    }
    return sum;
}

int main(void) {
    const line_item_t cart[] = {
        { "KB-MECH-87", 12900, 1 },
        { "CABLE-USBC",  1400, 2 },
        { "KEYCAP-SET",  3900, 1 },
    };
    const size_t count = sizeof(cart) / sizeof(cart[0]);
    const uint32_t total = subtotal_cents(cart, count);

    printf("items: %zu  total: $%u.%02u\n",
           count, total / 100, total % 100);
    return 0;
}
