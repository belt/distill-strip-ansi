package distillstripansi

/*
#cgo CFLAGS: -I${SRCDIR}/../c-abi/include
#cgo LDFLAGS: -ldistill_strip_ansi_c
#include <stdint.h>
#include <stdlib.h>
#include "distill_strip_ansi.h"
*/
import "C"

import (
	"errors"
	"unsafe"
)

var ErrInvalidArgument = errors.New("distill-strip-ansi: invalid argument")
var ErrNativeFailure = errors.New("distill-strip-ansi: native failure")

// Strip removes ANSI control sequences from input.
func Strip(input []byte) ([]byte, error) {
	output := make([]byte, len(input))
	var inputPointer, outputPointer *C.uint8_t
	if len(input) > 0 {
		inputPointer = (*C.uint8_t)(unsafe.Pointer(&input[0]))
		outputPointer = (*C.uint8_t)(unsafe.Pointer(&output[0]))
	}
	outputLen := C.dsa_strip(inputPointer, C.size_t(len(input)), outputPointer)
	if outputLen == C.intptr_t(-1) {
		return nil, ErrInvalidArgument
	}
	if outputLen < 0 {
		return nil, ErrNativeFailure
	}
	return output[:int(outputLen)], nil
}

// StripString removes ANSI control sequences from UTF-8 text.
func StripString(input string) (string, error) {
	output, err := Strip([]byte(input))
	if err != nil {
		return "", err
	}
	return string(output), nil
}

// ContainsANSI reports whether input contains an ANSI escape sequence.
func ContainsANSI(input []byte) (bool, error) {
	var inputPointer *C.uint8_t
	if len(input) > 0 {
		inputPointer = (*C.uint8_t)(unsafe.Pointer(&input[0]))
	}
	result := C.dsa_contains_ansi(inputPointer, C.size_t(len(input)))
	if result < 0 {
		return false, ErrInvalidArgument
	}
	return result == 1, nil
}
