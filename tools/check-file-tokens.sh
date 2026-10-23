#!/usr/bin/env bash
# check-file-tokens.sh — per-file token budget check.
#
# Budget: 5000 tokens/file. Token estimate = bytes / 4 → 20000 byte limit.
# Exits 1 and lists every violating file (worst first).
#
# Usage: tools/check-file-tokens.sh [root] [max_bytes]
#        tools/check-file-tokens.sh src 16000

root="${1:-.}"
max_bytes="${2:-20000}"

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
    -size "+${max_bytes}c" -printf '%s\t%p\n' \
  | sort -rn \
  | awk -v max="$max_bytes" -F'\t' '{
      t=int($1/4); lim=int(max/4)
      printf "FAIL  %6d tok  %s\n", t, $2
      bad=1
    } END {
      if (bad) { printf "--- limit %d tok/file (tokens=bytes/4) ---\n", lim > "/dev/stderr"; exit 1 }
      else     { printf "OK — every file under %d tokens\n", lim }
    }'
