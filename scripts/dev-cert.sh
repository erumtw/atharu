#!/usr/bin/env bash
# One-time: a self-signed code-signing identity "Atharu Local Signing" in the login keychain, so every
# Atharu.app build is signed the same way. macOS keys Microphone / Screen & System Audio Recording grants to
# the app's signature; an ad-hoc build gets a new one each install and silently loses them.
# Local only: not an Apple Developer ID (no notarization); nothing leaves this Mac. Asks your password once
# (to trust the certificate for code signing). Undo: Keychain Access › login › delete "Atharu Local Signing".
set -euo pipefail
name="Atharu Local Signing"
kc=$HOME/Library/Keychains/login.keychain-db
if security find-identity -v -p codesigning | grep -q "\"$name\""; then
  echo "$name already set up"; exit 0
fi
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
openssl req -x509 -newkey rsa:2048 -nodes -days 3650 -subj "/CN=$name" \
  -keyout "$tmp/key.pem" -out "$tmp/cert.pem" \
  -addext "basicConstraints=critical,CA:false" -addext "keyUsage=critical,digitalSignature" \
  -addext "extendedKeyUsage=critical,codeSigning" 2>/dev/null
# -legacy: macOS's `security import` can't read OpenSSL 3's default PKCS#12 encryption.
openssl pkcs12 -export -legacy -inkey "$tmp/key.pem" -in "$tmp/cert.pem" -name "$name" \
  -out "$tmp/id.p12" -passout pass:atharu
# -T: codesign may use the key without a keychain prompt on every build.
security import "$tmp/id.p12" -k "$kc" -P atharu -T /usr/bin/codesign >/dev/null
security add-trusted-cert -r trustRoot -p codeSign -k "$kc" "$tmp/cert.pem"
security find-identity -v -p codesigning | grep "\"$name\"" && echo "ok: next scripts/install.sh --app signs with it"
