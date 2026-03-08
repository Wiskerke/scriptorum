#!/usr/bin/env bash
# inject-certs.sh — Inject real certs (and optionally a server URL) into a
# distributed Scriptorum APK, then re-align and re-sign it.
#
# Usage:
#   inject-certs.sh [OPTIONS] <input.apk> <output.apk>
#
# Required:
#   --ca <ca.pem>          CA certificate
#   --cert <client.pem>    Client certificate
#   --key <client-key.pem> Client private key
#
# Optional:
#   --url <https://...>    Server URL (replaces placeholder in assets/config.json)
#   --keystore <path>      Signing keystore (default: ~/.android/debug.keystore)
#   --ks-alias <alias>     Keystore alias (default: androiddebugkey)
#   --ks-pass <pass>       Keystore password (default: android)
#
# Required tools: zip, zipalign, apksigner (all in the Nix dev shell)
set -euo pipefail

die() { echo "ERROR: $*" >&2; exit 1; }

# Defaults
KEYSTORE="${HOME}/.android/debug.keystore"
KS_ALIAS="androiddebugkey"
KS_PASS="android"
CA=""
CERT=""
KEY=""
URL=""

usage() {
  sed -n '/^# Usage/,/^[^#]/p' "$0" | sed 's/^# \?//' | head -n -1
  exit 1
}

# Parse args
while [[ $# -gt 0 ]]; do
  case "$1" in
    --ca)      CA="$2";       shift 2 ;;
    --cert)    CERT="$2";     shift 2 ;;
    --key)     KEY="$2";      shift 2 ;;
    --url)     URL="$2";      shift 2 ;;
    --keystore) KEYSTORE="$2"; shift 2 ;;
    --ks-alias) KS_ALIAS="$2"; shift 2 ;;
    --ks-pass)  KS_PASS="$2";  shift 2 ;;
    --help|-h) usage ;;
    -*)        die "Unknown option: $1" ;;
    *)         break ;;
  esac
done

INPUT="${1:-}"
OUTPUT="${2:-}"

[[ -n "$INPUT" && -n "$OUTPUT" ]] || { echo "Missing input/output APK arguments."; usage; }
[[ -f "$INPUT" ]]  || die "Input APK not found: $INPUT"
[[ -n "$CA" ]]     || die "--ca is required"
[[ -n "$CERT" ]]   || die "--cert is required"
[[ -n "$KEY" ]]    || die "--key is required"
[[ -f "$CA" ]]     || die "CA cert not found: $CA"
[[ -f "$CERT" ]]   || die "Client cert not found: $CERT"
[[ -f "$KEY" ]]    || die "Client key not found: $KEY"

for tool in zip zipalign apksigner; do
  command -v "$tool" &>/dev/null || die "$tool not found — are you in the Nix dev shell?"
done

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

UNSIGNED="$WORKDIR/unsigned.apk"
ALIGNED="$WORKDIR/aligned.apk"

echo "==> Copying APK..."
cp "$INPUT" "$UNSIGNED"

echo "==> Removing old cert/config entries..."
zip -d "$UNSIGNED" \
  "assets/certs/ca.pem" \
  "assets/certs/client.pem" \
  "assets/certs/client-key.pem" \
  "assets/config.json" 2>/dev/null || true

echo "==> Injecting new certs..."
# zip -j stores files with no directory prefix; we need them at assets/certs/
# Create a temp dir with the right layout
STAGING="$WORKDIR/staging"
mkdir -p "$STAGING/assets/certs"
cp "$CA"   "$STAGING/assets/certs/ca.pem"
cp "$CERT" "$STAGING/assets/certs/client.pem"
cp "$KEY"  "$STAGING/assets/certs/client-key.pem"

if [[ -n "$URL" ]]; then
  echo "==> Injecting server URL: $URL"
  printf '{\n  "server_url": "%s"\n}\n' "$URL" > "$STAGING/assets/config.json"
fi

# Add files preserving the assets/ path structure
(cd "$STAGING" && zip -r "$UNSIGNED" assets/)

echo "==> Aligning..."
zipalign -f -v 4 "$UNSIGNED" "$ALIGNED"

echo "==> Signing with keystore: $KEYSTORE (alias: $KS_ALIAS)..."
apksigner sign \
  --ks "$KEYSTORE" \
  --ks-key-alias "$KS_ALIAS" \
  --ks-pass "pass:$KS_PASS" \
  --out "$OUTPUT" \
  "$ALIGNED"

echo ""
echo "Done! Signed APK written to: $OUTPUT"
