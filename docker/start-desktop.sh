#!/bin/sh
set -eu
attempts=0
until xdpyinfo >/dev/null 2>&1; do
    attempts=$((attempts + 1))
    if [ "$attempts" -ge 100 ]; then
        echo 'X display failed to start' >&2
        exit 1
    fi
    sleep 0.1
done
exec dbus-run-session -- rustapi "$@"
