#!/bin/sh
set -e

if [ -z "$NIX_CONF_CONTENT" ]; then
    echo "ERROR: NIX_CONF_CONTENT not set" >&2
    exit 1
fi

if [ "$(id -u)" != "0" ]; then
    echo "ERROR: Must run as root" >&2
    exit 1
fi

# Remove existing config (often a symlink in the base image)
rm -f /etc/nix/nix.conf

# Write new config from environment variable
printf "%s" "$NIX_CONF_CONTENT" > /etc/nix/nix.conf
chmod 644 /etc/nix/nix.conf

# Hand off to nix-daemon (or whatever CMD is specified)
exec "$@"
