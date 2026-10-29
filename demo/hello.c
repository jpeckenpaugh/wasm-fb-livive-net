#include <stdint.h>
#include <stdio.h>

/* fill a buffer with a pattern */
static void fill(uint8_t *px, int n, uint8_t v) {
    for (int i = 0; i < n; i++) px[i] = v;
}

int main(void) {
    uint8_t buf[16];
    fill(buf, sizeof buf, 0x20);
    printf("%d\n", (int)buf[0]);
    return 0;
}
