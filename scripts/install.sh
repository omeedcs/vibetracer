#!/usr/bin/env bash
set -euo pipefail

REPO="omeedcs/vibetracer"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

detect_platform() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os" in
    Darwin) os="apple-darwin" ;;
    Linux)  os="unknown-linux-gnu" ;;
    *)      echo "Unsupported OS: $os" >&2; exit 1 ;;
  esac
  case "$arch" in
    x86_64)  arch="x86_64" ;;
    arm64|aarch64) arch="aarch64" ;;
    *)       echo "Unsupported arch: $arch" >&2; exit 1 ;;
  esac
  echo "${arch}-${os}"
}

main() {
  local platform version url tmp
  platform="$(detect_platform)"
  version="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep tag_name | cut -d'"' -f4)"
  echo "Installing vibetracer ${version} for ${platform}..."
  url="https://github.com/${REPO}/releases/download/${version}/vibetracer-${platform}.tar.gz"
  tmp="$(mktemp -d)"
  curl -fsSL "$url" | tar xz -C "$tmp"
  mkdir -p "$INSTALL_DIR"
  mv "$tmp/vibetracer" "$INSTALL_DIR/vibetracer"
  chmod +x "$INSTALL_DIR/vibetracer"
  rm -rf "$tmp"
  echo "Installed to ${INSTALL_DIR}/vibetracer"
  if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
    echo "Add ${INSTALL_DIR} to your PATH:"
    echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
  fi
}

main
