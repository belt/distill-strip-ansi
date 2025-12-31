#!/usr/bin/env bash
# Generate complete TOML mapping files from UnicodeData.txt.
# Usage: ./bin/generate-toml.sh [UnicodeData.txt]
#
# Requires: awk, curl (if UnicodeData.txt not provided)

set -euo pipefail

UCD="${1:-UnicodeData.txt}"

if [ ! -f "$UCD" ]; then
    echo "Downloading UnicodeData.txt..."
    curl -sO https://unicode.org/Public/UNIDATA/UnicodeData.txt
    UCD="UnicodeData.txt"
fi

generate_pairs() {
    local pattern="$1"
    awk -F';' -v pat="$pattern" '
        $1 ~ pat && $6 != "" {
            # Strip decomposition type tags like <compat>, <font>, etc.
            target = $6
            gsub(/<[^>]+> */, "", target)
            gsub(/^ +| +$/, "", target)
            if (target == "") next

            n = split(target, parts, " ")
            if (n == 1) {
                printf "[[pairs]]\nfrom = \"%s\"\nto = \"%s\"\n\n", $1, parts[1]
            } else {
                printf "[[pairs]]\nfrom = \"%s\"\nto_seq = \"", $1
                for (i = 1; i <= n; i++) {
                    if (i > 1) printf " "
                    printf "%s", parts[i]
                }
                printf "\"\n\n"
            }
        }
    ' "$UCD"
}

echo "=== cjk-compat-ideographs.toml (F900-FAD9) ==="
generate_pairs '^F9|^FA[0-9A-D]' > /tmp/cjk-compat-pairs.toml
wc -l /tmp/cjk-compat-pairs.toml

echo "=== cjk-compat-ideographs-supplement.toml (2F800-2FA1F) ==="
generate_pairs '^2F[89A]' > /tmp/cjk-supplement-pairs.toml
wc -l /tmp/cjk-supplement-pairs.toml

echo "=== enclosed-cjk.toml (3200-32FF) ==="
generate_pairs '^32[0-9A-F]' > /tmp/enclosed-cjk-pairs.toml
wc -l /tmp/enclosed-cjk-pairs.toml

echo "=== cjk-compatibility.toml (3300-33FF) ==="
generate_pairs '^33[0-9A-F]' > /tmp/cjk-compat-pairs2.toml
wc -l /tmp/cjk-compat-pairs2.toml

echo "=== arabic-presentation-forms.toml (FB50-FDFF, FE70-FEFF) ==="
generate_pairs '^FB[5-9A-F]|^FC|^FD|^FE[7-9A-F]' > /tmp/arabic-pairs.toml
wc -l /tmp/arabic-pairs.toml

echo "=== enclosed-alphanumeric-supplement.toml (1F100-1F1FF) ==="
generate_pairs '^1F1[0-9A-E]|^1F10' > /tmp/enclosed-supp-pairs.toml
wc -l /tmp/enclosed-supp-pairs.toml

echo "Done. Pairs written to /tmp/*.toml"
