# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

$(eval $(call native-rust-app,svc/fs-erofs,hyper-fs-erofs-system,fs-erofs-system,svc/fs-erofs,system development))
