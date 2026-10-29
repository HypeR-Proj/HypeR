#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Populate a workspace using repo; all source layout is defined by the manifest.
set -eu
usage() {
    echo "Usage: $0 [--branch BRANCH] [DIRECTORY]"
    echo "Default: main manifest, workspace/ beside this script."
}
branch=main
if [ "${1:-}" = --help ] || [ "${1:-}" = -h ]; then
    usage
    exit 0
fi
if [ "${1:-}" = --branch ]; then
    [ "$#" -ge 2 ] || { usage >&2; exit 2; }
    branch=$2
    shift 2
fi
[ "$#" -le 1 ] || { usage >&2; exit 2; }
case "$branch" in ''|-*) echo 'Invalid manifest branch' >&2; exit 2 ;; esac
script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
directory=${1:-$script_dir/workspace}
case "$directory" in /*) ;; *) directory=$PWD/$directory ;; esac
command -v git >/dev/null 2>&1 || { echo 'Install git first.' >&2; exit 1; }
command -v repo >/dev/null 2>&1 || { echo 'Install the Android repo tool first (see README.md).' >&2; exit 1; }
if [ -e "$directory" ] && [ ! -d "$directory/.repo" ]; then
    [ -d "$directory" ] || { echo "Not a directory: $directory" >&2; exit 1; }
    if [ -n "$(ls -A "$directory")" ]; then
        echo "Refusing a nonempty directory not managed by repo: $directory" >&2
        exit 1
    fi
fi
mkdir -p "$directory"
cd "$directory"
repo init -u https://github.com/HypeR-Proj/manifest -b "$branch"
repo sync
printf '\nWorkspace ready: %s\n' "$directory"
printf 'Build from that directory; see README.md and docs/learn/getting-started.md.\n'
