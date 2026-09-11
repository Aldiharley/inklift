#!/usr/bin/env bash
# A/B the local pipeline against a hosted image model on the same pages.
#
#   ./compare.sh <pages_dir> <groundtruth_dir> [provider]
#
# Ground truth is black ink on white, named <page>_GT.png. Requires the
# provider's API key in the environment (GEMINI_API_KEY or OPENAI_API_KEY).
set -euo pipefail

PAGES=${1:?usage: compare.sh <pages_dir> <groundtruth_dir> [provider]}
GT=${2:?usage: compare.sh <pages_dir> <groundtruth_dir> [provider]}
PROVIDER=${3:-gemini}
OUT=${OUT:-compare-out}

BIN=./target/release/inklift
SCORE=./target/release/inklift-score
[ -x "$BIN" ] || { echo "build first: cargo build --release" >&2; exit 1; }

rm -rf "$OUT"
mkdir -p "$OUT/local" "$OUT/$PROVIDER"

shopt -s nullglob
pages=("$PAGES"/*.png "$PAGES"/*.jpg "$PAGES"/*.jpeg)
[ ${#pages[@]} -gt 0 ] || { echo "no images in $PAGES" >&2; exit 1; }

echo "== local pipeline (${#pages[@]} pages) =="
for f in "${pages[@]}"; do
    "$BIN" "$f" --white -o "$OUT/local/$(basename "${f%.*}").png" -q
done

echo "== $PROVIDER =="
failed=0
for f in "${pages[@]}"; do
    name=$(basename "${f%.*}")
    if ! "$BIN" "$f" --via "$PROVIDER" --white -o "$OUT/$PROVIDER/$name.png" -q; then
        echo "  !! $name failed" >&2
        failed=$((failed + 1))
    fi
done
[ "$failed" -eq 0 ] || echo "($failed page(s) failed on $PROVIDER)"

echo
echo "################ local ################"
"$SCORE" "$OUT/local" "$GT" --csv "$OUT/local.csv"
echo
echo "################ $PROVIDER ################"
# --resize: hosted models return their own dimensions.
"$SCORE" "$OUT/$PROVIDER" "$GT" --resize --csv "$OUT/$PROVIDER.csv"
echo
echo "per-image CSVs: $OUT/local.csv, $OUT/$PROVIDER.csv"
