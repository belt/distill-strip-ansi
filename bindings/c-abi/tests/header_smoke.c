#include <assert.h>
#include <stdint.h>
#include <string.h>

#include "distill_strip_ansi.h"

int main(void) {
    const uint8_t input[] = "a\033[31mb\033[0m";
    uint8_t output[sizeof(input)];
    intptr_t output_len = dsa_strip(input, sizeof(input) - 1, output);

    assert(dsa_abi_version() == 1);
    assert(output_len == 2);
    assert(memcmp(output, "ab", 2) == 0);
    assert(dsa_contains_ansi(input, sizeof(input) - 1) == 1);
    assert(dsa_contains_ansi((const uint8_t *)"plain", 5) == 0);
    return 0;
}