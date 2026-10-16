#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

set -eu
script_dir=$(CDPATH='' cd -- "$(dirname "$0")" && pwd)
exec python3 -B "$script_dir/verify-native-init.py" "$@"
