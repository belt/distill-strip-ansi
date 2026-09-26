import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import koffi from "koffi";

const ABI_VERSION = 1;
const INVALID_ARGUMENT = -1;
const root = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");

function libraryCandidates(): string[] {
  const configured = process.env.DISTILL_STRIP_ANSI_LIB;
  const name = process.platform === "darwin"
    ? "libdistill_strip_ansi_c.dylib"
    : process.platform === "win32"
      ? "distill_strip_ansi_c.dll"
      : "libdistill_strip_ansi_c.so";
  const candidates = configured ? [configured] : [];
  candidates.push(
    resolve(root, "target/release", name),
    resolve(root, "target/debug", name),
    name,
  );
  return candidates;
}

function loadNative() {
  const candidates = libraryCandidates();
  const path = candidates.find((candidate) => candidate === "libdistill_strip_ansi_c.so" || existsSync(candidate));
  if (!path) {
    throw new Error(`Could not find libdistill_strip_ansi_c; set DISTILL_STRIP_ANSI_LIB. Tried: ${candidates.join(", ")}`);
  }
  const library = koffi.load(path);
  const abiVersion = library.func("uint32_t dsa_abi_version(void)") as () => number;
  const nativeStrip = library.func(
    "intptr_t dsa_strip(const uint8_t *input, size_t input_len, uint8_t *output)",
  ) as (input: Buffer, inputLength: number, output: Buffer) => number;
  const nativeContains = library.func(
    "int32_t dsa_contains_ansi(const uint8_t *input, size_t input_len)",
  ) as (input: Buffer, inputLength: number) => number;
  const version = abiVersion();
  if (version !== ABI_VERSION) {
    throw new Error(`Unsupported distill-strip-ansi C ABI version ${version}`);
  }
  return { nativeStrip, nativeContains };
}

let native: ReturnType<typeof loadNative> | undefined;
function api() {
  native ??= loadNative();
  return native;
}

/** Strip ANSI control sequences from a byte buffer. */
export function strip(input: Uint8Array): Buffer {
  const bytes = Buffer.from(input.buffer, input.byteOffset, input.byteLength);
  const output = Buffer.alloc(bytes.length);
  const result = api().nativeStrip(bytes, bytes.length, output);
  if (result === INVALID_ARGUMENT) throw new TypeError("Invalid input passed to distill-strip-ansi");
  if (result < 0) throw new Error(`distill-strip-ansi failed with status ${result}`);
  return output.subarray(0, result);
}

/** Return whether a byte buffer contains an ANSI escape sequence. */
export function containsAnsi(input: Uint8Array): boolean {
  const bytes = Buffer.from(input.buffer, input.byteOffset, input.byteLength);
  const result = api().nativeContains(bytes, bytes.length);
  if (result < 0) throw new TypeError("Invalid input passed to distill-strip-ansi");
  return result === 1;
}