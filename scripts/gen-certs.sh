#!/usr/bin/env bash
# Usage: gen-certs.sh [output-dir]
# Optional env vars:
#   SERVER_HOSTNAME — extra DNS name or IP added to the server cert SAN
#                     (e.g. SERVER_HOSTNAME=your.server.example.com)
set -euo pipefail

OUT="${1:-./certs}"
mkdir -p "$OUT"

DAYS=3650

# Build the SAN extension for the server cert.
# Always include localhost + emulator host; add SERVER_HOSTNAME if provided.
SAN="DNS:localhost,IP:127.0.0.1,IP:10.0.2.2"
if [[ -n "${SERVER_HOSTNAME:-}" ]]; then
  # Detect whether it looks like an IP address or a DNS name
  if [[ "$SERVER_HOSTNAME" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    SAN="$SAN,IP:$SERVER_HOSTNAME"
  else
    SAN="$SAN,DNS:$SERVER_HOSTNAME"
  fi
  echo "Server SAN: $SAN"
fi

echo "=== Generating CA ==="
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
  -keyout "$OUT/ca-key.pem" -out "$OUT/ca.pem" \
  -days "$DAYS" -nodes -subj "/CN=Scriptorum CA"

echo "=== Generating server cert ==="
openssl req -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
  -keyout "$OUT/server-key.pem" -out "$OUT/server.csr" \
  -nodes -subj "/CN=scriptorum-server"

openssl x509 -req -in "$OUT/server.csr" \
  -CA "$OUT/ca.pem" -CAkey "$OUT/ca-key.pem" -CAcreateserial \
  -out "$OUT/server.pem" -days "$DAYS" \
  -extfile <(printf "subjectAltName=%s" "$SAN")

echo "=== Generating client cert ==="
openssl req -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
  -keyout "$OUT/client-key.pem" -out "$OUT/client.csr" \
  -nodes -subj "/CN=scriptorum-client"

openssl x509 -req -in "$OUT/client.csr" \
  -CA "$OUT/ca.pem" -CAkey "$OUT/ca-key.pem" -CAcreateserial \
  -out "$OUT/client.pem" -days "$DAYS"

# Clean up CSRs and serial file
rm -f "$OUT"/*.csr "$OUT"/*.srl

echo "=== Done ==="
echo "Certs written to $OUT/"
ls -la "$OUT"/*.pem
