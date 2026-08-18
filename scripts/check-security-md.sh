#!/usr/bin/env bash
# Verify security/SECURITY.md points at (or matches) root SECURITY.md.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SEC="$ROOT/security/SECURITY.md"
ROOT_SEC="$ROOT/SECURITY.md"

if [[ ! -f "$ROOT_SEC" ]]; then
  echo "ERROR: root SECURITY.md does not exist"
  exit 1
fi

if [[ ! -e "$SEC" ]]; then
  echo "ERROR: security/SECURITY.md does not exist"
  exit 1
fi

resolve() {
  local path="$1"
  if command -v realpath &>/dev/null; then
    realpath "$path"
  else
    echo "$(cd "$(dirname "$path")" && pwd -P)/$(basename "$path")"
  fi
}

if [[ -L "$SEC" ]]; then
  resolved="$(resolve "$SEC")"
  root_resolved="$(resolve "$ROOT_SEC")"
  if [[ "$resolved" != "$root_resolved" ]]; then
    echo "ERROR: security/SECURITY.md symlink resolves to:"
    echo "  $resolved"
    echo "expected root SECURITY.md at:"
    echo "  $root_resolved"
    exit 1
  fi
  echo "OK: security/SECURITY.md is a symlink to root SECURITY.md"
  exit 0
fi

if ! diff -q "$SEC" "$ROOT_SEC" >/dev/null; then
  echo "ERROR: security/SECURITY.md is a regular file whose content does not match root SECURITY.md"
  exit 1
fi

echo "OK: security/SECURITY.md matches root SECURITY.md (copy fallback)"
