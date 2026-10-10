#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
fixture_dir=$(mktemp -d)
fixture_container="ultorglike-tls-check-$$"
cleanup() {
  docker rm -f "$fixture_container" >/dev/null 2>&1 || true
  rm -rf "$fixture_dir"
}
trap cleanup EXIT
cat > "$fixture_dir/cert.cnf" <<'CERT'
[req]
distinguished_name=dn
x509_extensions=extensions
prompt=no
[dn]
CN=localhost
[extensions]
subjectAltName=DNS:localhost
basicConstraints=critical,CA:TRUE
keyUsage=critical,digitalSignature,keyEncipherment,keyCertSign
extendedKeyUsage=serverAuth
CERT
openssl req -new -x509 -nodes -newkey rsa:2048 -days 2 -config "$fixture_dir/cert.cnf" \
  -keyout "$fixture_dir/server.key" -out "$fixture_dir/server.crt" 2>/dev/null
docker run -d --name "$fixture_container" -p 127.0.0.1::5432 \
  -e POSTGRES_PASSWORD=tls-fixture-only -v "$fixture_dir:/tls:ro" postgres:17-alpine \
  sh -c 'cp /tls/server.key /tmp/server.key; chown postgres:postgres /tmp/server.key; chmod 600 /tmp/server.key; exec docker-entrypoint.sh postgres -c ssl=on -c ssl_cert_file=/tls/server.crt -c ssl_key_file=/tmp/server.key' >/dev/null
for attempt in $(seq 1 30); do
  if docker exec "$fixture_container" pg_isready -U postgres >/dev/null 2>&1; then break; fi
  sleep 1
done
fixture_port=$(docker inspect -f '{{(index (index .NetworkSettings.Ports "5432/tcp") 0).HostPort}}' "$fixture_container")
export TEST_TLS_DATABASE_URL="postgresql://postgres:tls-fixture-only@localhost:$fixture_port/postgres?sslmode=require"
export PGSSLROOTCERT="$fixture_dir/server.crt"
cargo test -p query-engine --test tls --locked -- --ignored --test-threads=1
