#!/usr/bin/env bash
# Usage: gen-ca.sh [--out <dir>] [--name <name>]
#   --out   output directory for CA files (default: ./certs)
#   --name    common name for the CA certificate (default: Scriptorum CA)
set -euo pipefail

OUT="./certs"
CN="Scriptorum CA"
DAYS=3650

while [[ $# -gt 0 ]]; do
  case "$1" in
    --out) OUT="$2"; shift 2 ;;
    --name)  CN="$2";  shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

mkdir -p "$OUT"

echo "=== Generating CA ==="
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
  -keyout "$OUT/ca-key.pem" -out "$OUT/ca.pem" \
  -days "$DAYS" -nodes -subj "/CN=$CN"

echo "CA written to $OUT/"
ls -la "$OUT/ca.pem" "$OUT/ca-key.pem"
