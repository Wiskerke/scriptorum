#!/usr/bin/env bash
# Usage: gen-server-cert.sh [--name <name>] [--out <dir>] [--ca-dir <dir>] [hostname-or-ip ...]
#   --name    common name and output file prefix (default: server)
#   --out     output directory for server cert (default: ./certs)
#   --ca-dir  directory containing ca.pem and ca-key.pem (default: ./certs)
#   Remaining positional args are added to the certificate SAN.
#   DNS names and IP addresses are detected automatically.
#
# Examples:
#   gen-server-cert.sh example.com api.example.com
#   gen-server-cert.sh --name myserver example.com 192.168.1.10
set -euo pipefail

NAME="server"
OUT="./certs"
CA_DIR="./certs"
DAYS=3650
HOSTNAMES=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --name)   NAME="$2";   shift 2 ;;
    --out)    OUT="$2";    shift 2 ;;
    --ca-dir) CA_DIR="$2"; shift 2 ;;
    --*) echo "Unknown argument: $1" >&2; exit 1 ;;
    *) HOSTNAMES+=("$1"); shift ;;
  esac
done

mkdir -p "$OUT"

# Always include localhost.
SAN="DNS:localhost,IP:127.0.0.1"
for h in "${HOSTNAMES[@]}"; do
  if [[ "$h" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    SAN="$SAN,IP:$h"
  else
    SAN="$SAN,DNS:$h"
  fi
done
echo "Server SAN: $SAN"

echo "=== Generating server cert: $NAME ==="
openssl req -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
  -keyout "$OUT/$NAME-key.pem" -out "$OUT/$NAME.csr" \
  -nodes -subj "/CN=$NAME"

openssl x509 -req -in "$OUT/$NAME.csr" \
  -CA "$CA_DIR/ca.pem" -CAkey "$CA_DIR/ca-key.pem" -CAcreateserial \
  -out "$OUT/$NAME.pem" -days "$DAYS" \
  -extfile <(printf "subjectAltName=%s\nextendedKeyUsage=serverAuth" "$SAN")

rm -f "$OUT/$NAME.csr"

echo "Server cert written to $OUT/"
ls -la "$OUT/$NAME.pem" "$OUT/$NAME-key.pem"
