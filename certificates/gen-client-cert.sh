#!/usr/bin/env bash
# Usage: gen-client-cert.sh [--name <cn>] [--out <dir>] [--ca-dir <dir>]
#   --name    common name and output file prefix (default: client)
#   --out     output directory for client cert files (default: ./certs)
#   --ca-dir  directory containing ca.pem and ca-key.pem (default: ./certs)
#
# Outputs:
#   <out>/<name>.pem       — certificate (PEM)
#   <out>/<name>-key.pem   — private key (PEM)
#   <out>/<name>.p12       — PKCS#12 bundle for browser/OS import (no password)
set -euo pipefail

NAME="client"
OUT="./certs"
CA_DIR="./certs"
DAYS=3650

while [[ $# -gt 0 ]]; do
  case "$1" in
    --name)   NAME="$2";   shift 2 ;;
    --out)    OUT="$2";    shift 2 ;;
    --ca-dir) CA_DIR="$2"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

mkdir -p "$OUT"

echo "=== Generating client cert: $NAME ==="
openssl req -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
  -keyout "$OUT/$NAME-key.pem" -out "$OUT/$NAME.csr" \
  -nodes -subj "/CN=$NAME"

openssl x509 -req -in "$OUT/$NAME.csr" \
  -CA "$CA_DIR/ca.pem" -CAkey "$CA_DIR/ca-key.pem" -CAcreateserial \
  -out "$OUT/$NAME.pem" -days "$DAYS" \
  -extfile <(printf "extendedKeyUsage=clientAuth")

# PKCS#12 bundle for importing into browsers and OS certificate stores.
# The bundle includes the client cert, its key, and the CA cert chain.
# Exported with no password — set one with -passout pass:yourpassword if needed.
openssl pkcs12 -export \
  -in "$OUT/$NAME.pem" -inkey "$OUT/$NAME-key.pem" \
  -certfile "$CA_DIR/ca.pem" \
  -out "$OUT/$NAME.p12" \
  -passout pass:

rm -f "$OUT/$NAME.csr"

echo "Client cert written to $OUT/"
ls -la "$OUT/$NAME.pem" "$OUT/$NAME-key.pem" "$OUT/$NAME.p12"
echo "Import $OUT/$NAME.p12 into browsers/OS (no password)."
