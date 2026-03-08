#!/usr/bin/env bash
# Generates throwaway placeholder certs for the Android assets directory.
# These are NON-FUNCTIONAL for real use — they exist only so `just apk`
# builds out of the box without requiring `just install-certs` first.
# Replace them with real certs by running: just gen-certs && just install-certs
set -euo pipefail

OUT="android/app/src/main/assets/certs"
mkdir -p "$OUT"

DAYS=3650

echo "=== Generating placeholder CA ==="
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
  -keyout "$OUT/ca-key-placeholder.pem" -out "$OUT/ca.pem" \
  -days "$DAYS" -nodes -subj "/CN=Scriptorum Placeholder CA"

echo "=== Generating placeholder client cert ==="
openssl req -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
  -keyout "$OUT/client-key.pem" -out "$OUT/client.csr" \
  -nodes -subj "/CN=scriptorum-client-placeholder"

openssl x509 -req -in "$OUT/client.csr" \
  -CA "$OUT/ca.pem" -CAkey "$OUT/ca-key-placeholder.pem" -CAcreateserial \
  -out "$OUT/client.pem" -days "$DAYS"

# Clean up ephemeral files (keep only the three files the app needs)
rm -f "$OUT/ca-key-placeholder.pem" "$OUT/client.csr" "$OUT"/*.srl

echo "=== Done ==="
echo "Placeholder certs written to $OUT/"
echo "  ca.pem, client.pem, client-key.pem"
echo ""
echo "NOTE: These are placeholder certs and will NOT authenticate against any"
echo "      real server.  Run 'just gen-certs && just install-certs' to replace"
echo "      them with certs signed by your own CA."
