#ifndef DISTILL_STRIP_ANSI_H
#define DISTILL_STRIP_ANSI_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define DSA_ERROR_INVALID_ARGUMENT ((intptr_t)-1)
#define DSA_ERROR_PANIC ((intptr_t)-2)

uint32_t dsa_abi_version(void);

/*
 * Strip ANSI sequences into caller-owned output storage.
 *
 * Allocate at least input_len writable bytes for output. For non-empty input,
 * input and output must be valid and non-overlapping. Returns the number of
 * output bytes, or one of DSA_ERROR_* above.
 */
intptr_t dsa_strip(const uint8_t *input, size_t input_len, uint8_t *output);

/* Returns 1 when an ANSI sequence is present, 0 otherwise, -1 on bad input. */
int32_t dsa_contains_ansi(const uint8_t *input, size_t input_len);

#ifdef __cplusplus
}
#endif

#endif