#!/usr/bin/env bash
# check-file-tokens.sh — per-file token budget check.
#
# Budget: 5000 tokens/file. Token estimate = bytes / 4 → 20000 byte limit.
# A file at exactly 20000 bytes (5000 tokens) FAILS — the limit is <20000.
# Exits 1 and lists every violating file (worst first). Any find/sort/awk
# failure also exits nonzero — a broken pipeline can never print a false OK.
#
# Usage: tools/check-file-tokens.sh [root] [max_bytes]
#        tools/check-file-tokens.sh src 16000

set -o pipefail

root="${1:-.}"
max_bytes="${2:-20000}"

# -size "+N c" is strictly-greater-than N bytes, so use max_bytes-1 so that
# a file at exactly max_bytes is reported as a violation.
max_m1=$((max_bytes - 1))

find "$root" -type f \
    \( -name '*.rs' -o -name '*.md' -o -name '*.toml' -o -name '*.ps1' \
       -o -name '*.vbs' -o -name '*.sh' -o -name '*.json' -o -name '*.html' \
       -o -name '*.css' -o -name '*.js' -o -name '*.ts' -o -name '*.yaml' \
       -o -name '*.yml' -o -name '*.txt' \) \
    ! -path '*/.git/*' \
    ! -path '*/target/*' \
    ! -path '*/target-*/*' \
    ! -path '*/.devctl/*' \
    ! -path '*/reference/*' \
    ! -path '*/benchmark/*' \
    ! -path '*/assets/*' \
    ! -path '*/docs/*' \
    ! -path '*/node_modules/*' \
    -size "+${max_m1}c" -printf '%s\t%p\n' \
  | sort -rn \
  | awk -v max="$max_bytes" -F'\t' '{
      t=int($1/4); lim=int(max/4)
      printf "FAIL  %6d tok  %s\n", t, $2
      bad=1
    } END {
      if (bad) { printf "--- limit %d tok/file (tokens=bytes/4) ---\n", lim > "/dev/stderr"; exit 1 }
      else     { printf "OK — every file under %d tokens\n", lim }
    }'
