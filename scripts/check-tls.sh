#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
fixture_dir=$(mktemp -d)
fixture_container="ultorglike-tls-check-$$"
cleanup() {
  status=$?
  if [ "$status" -ne 0 ]; then
    docker logs "$fixture_container" >&2 || true
  fi
  docker rm -fv "$fixture_container" >/dev/null 2>&1 || true
  rm -rf "$fixture_dir"
  exit "$status"
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
  sh -ec '
    # The host mktemp directory is mode 700; postgres cannot traverse it on Linux.
    cp /tls/server.key /tmp/server.key
    cp /tls/server.crt /tmp/server.crt
    chown postgres:postgres /tmp/server.key /tmp/server.crt
    chmod 600 /tmp/server.key
    chmod 644 /tmp/server.crt
    exec docker-entrypoint.sh postgres -c ssl=on -c ssl_cert_file=/tmp/server.crt -c ssl_key_file=/tmp/server.key
  ' >/dev/null
fixture_ready=false
for attempt in $(seq 1 30); do
  if [ "$(docker inspect -f '{{.State.Running}}' "$fixture_container")" != "true" ]; then
    echo "TLS fixture exited before PostgreSQL became ready" >&2
    exit 1
  fi
  # The temporary initialization server only listens on a Unix socket.
  if docker exec "$fixture_container" pg_isready -h 127.0.0.1 -U postgres >/dev/null 2>&1; then
    fixture_ready=true
    break
  fi
  sleep 1
done
if [ "$fixture_ready" != "true" ]; then
  echo "Timed out waiting for the TLS PostgreSQL fixture" >&2
  exit 1
fi
fixture_port=$(docker inspect -f '{{(index (index .NetworkSettings.Ports "5432/tcp") 0).HostPort}}' "$fixture_container")
export TEST_TLS_DATABASE_URL="postgresql://postgres:tls-fixture-only@localhost:$fixture_port/postgres?sslmode=require"
export PGSSLROOTCERT="$fixture_dir/server.crt"
cargo test -p query-engine --test tls --locked -- --ignored --test-threads=1
