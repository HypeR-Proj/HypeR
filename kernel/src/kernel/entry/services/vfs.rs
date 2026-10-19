// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Filesystem calls need Process authority and the calling Thread cancellation state.

use super::ProcessServices;
use crate::kernel::abi::native::VfsServices;
use crate::kernel::capability::{HandleValue, InvocationReceiver, Rights};
use crate::kernel::mm::user_space::UserSlice;
use crate::kernel::process::UserThreadPhase;
use crate::kernel::vfs::VfsServiceError;

impl VfsServices for ProcessServices<'_> {
    fn mount_ramfs(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
    ) -> Result<(), VfsServiceError> {
        super::vfs_namespace::mount_ramfs(self.process, directory, path)
    }
    fn switch_root(
        &self,
        directory: &InvocationReceiver,
        request: UserSlice,
    ) -> Result<(), VfsServiceError> {
        super::vfs_namespace::switch_root(self.process, directory, request)
    }

    fn begin_root_switch(
        &self,
        directory: &InvocationReceiver,
        request: UserSlice,
    ) -> Result<HandleValue, VfsServiceError> {
        super::vfs_namespace::begin_root_switch(self.process, directory, request)
    }
    fn finish_root_switch(
        &self,
        token: &InvocationReceiver,
        commit: bool,
    ) -> Result<(), VfsServiceError> {
        super::vfs_namespace::finish_root_switch(token, commit)
    }

    fn directory_scope_create(
        &self,
        root: &InvocationReceiver,
        start: HandleValue,
        rights: Rights,
    ) -> Result<HandleValue, VfsServiceError> {
        crate::kernel::vfs::service::directory_scope_create(self.process, root, start, rights)
    }
    fn directory_get_metadata(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
        follow: bool,
    ) -> Result<crate::kernel::vfs::Metadata, VfsServiceError> {
        crate::kernel::vfs::service::directory_get_metadata(self.process, directory, path, follow)
    }
    fn file_get_metadata(
        &self,
        file: &InvocationReceiver,
    ) -> Result<crate::kernel::vfs::Metadata, VfsServiceError> {
        crate::kernel::vfs::service::file_get_metadata(file)
    }
    fn directory_get_self_metadata(
        &self,
        directory: &InvocationReceiver,
    ) -> Result<crate::kernel::vfs::Metadata, VfsServiceError> {
        crate::kernel::vfs::service::directory_get_self_metadata(directory)
    }
    fn directory_set_metadata(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
        follow: bool,
        update: crate::kernel::vfs::MetadataUpdate,
    ) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::service::directory_set_metadata(
            self.process,
            directory,
            path,
            follow,
            update,
        )
    }
    fn file_set_metadata(
        &self,
        file: &InvocationReceiver,
        update: crate::kernel::vfs::MetadataUpdate,
    ) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::service::file_set_metadata(file, update)
    }
    fn directory_rename(
        &self,
        source: &InvocationReceiver,
        path: UserSlice,
        destination: HandleValue,
        new_path: UserSlice,
    ) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::service::directory_rename(
            self.process,
            source,
            path,
            destination,
            new_path,
        )
    }
    fn directory_link(
        &self,
        source: &InvocationReceiver,
        path: UserSlice,
        destination: HandleValue,
        new_path: UserSlice,
    ) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::service::directory_link(
            self.process,
            source,
            path,
            destination,
            new_path,
        )
    }
    fn directory_symlink(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
        target: UserSlice,
    ) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::service::directory_symlink(self.process, directory, path, target)
    }
    fn directory_read_link(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
    ) -> Result<crate::kernel::vfs::ScratchVec<u8>, VfsServiceError> {
        crate::kernel::vfs::service::directory_read_link(self.process, directory, path)
    }
    fn directory_canonicalize(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
    ) -> Result<crate::kernel::vfs::ScratchString, VfsServiceError> {
        crate::kernel::vfs::service::directory_canonicalize(self.process, directory, path)
    }
    fn directory_remove_if(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
        is_directory: bool,
        expected: u64,
    ) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::service::directory_remove_if(
            self.process,
            directory,
            path,
            is_directory,
            expected,
        )
    }
    fn directory_open_directory_nofollow(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
        rights: Rights,
    ) -> Result<HandleValue, VfsServiceError> {
        crate::kernel::vfs::service::directory_open_directory_nofollow(
            self.process,
            directory,
            path,
            rights,
        )
    }
    fn file_sync(&self, file: &InvocationReceiver, scope: u64) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::service::file_sync(file, scope)
    }
    fn file_lock(
        &self,
        file: &InvocationReceiver,
        mode: crate::kernel::vfs::locks::LockMode,
        deadline: u64,
    ) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::service::file_lock(self.process, file, mode, deadline, || {
            self.thread.snapshot().phase == UserThreadPhase::StopRequested
        })
    }
    fn file_unlock(&self, file: &InvocationReceiver) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::service::file_unlock(file)
    }
    fn directory_open_file_with_options(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
        rights: Rights,
        options: u64,
        mode: u32,
    ) -> Result<HandleValue, VfsServiceError> {
        crate::kernel::vfs::service::directory_open_file_with_options(
            self.process,
            directory,
            path,
            rights,
            options,
            mode,
        )
    }

    fn create_file(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
        rights: Rights,
        mode: u32,
    ) -> Result<HandleValue, VfsServiceError> {
        crate::kernel::vfs::create_file(self.process, directory, path, rights, mode)
    }

    fn create_directory(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
        mode: u32,
    ) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::create_directory(self.process, directory, path, mode)
    }

    fn remove_entry(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
        is_directory: bool,
    ) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::remove_entry(self.process, directory, path, is_directory)
    }

    fn resize_file(&self, file: &InvocationReceiver, length: u64) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::resize_file(file, length)
    }

    fn write_file_at(
        &self,
        file: &InvocationReceiver,
        offset: Option<u64>,
        input: Option<UserSlice>,
    ) -> Result<(u64, u64), VfsServiceError> {
        crate::kernel::vfs::write_file_at(self.process, file, offset, input)
    }

    fn open_file(
        &self,
        root: &InvocationReceiver,
        path: UserSlice,
        rights: Rights,
    ) -> Result<HandleValue, VfsServiceError> {
        crate::kernel::vfs::open_file(self.process, root, path, rights)
    }

    fn open_directory(
        &self,
        root: &InvocationReceiver,
        path: UserSlice,
        rights: Rights,
    ) -> Result<HandleValue, VfsServiceError> {
        crate::kernel::vfs::open_directory(self.process, root, path, rights)
    }

    fn begin_directory_read(
        &self,
        directory: &InvocationReceiver,
    ) -> Result<HandleValue, VfsServiceError> {
        crate::kernel::vfs::begin_directory_read(self.process, directory)
    }

    fn read_directory(
        &self,
        directory: &InvocationReceiver,
        cookie: u64,
        page: &mut crate::kernel::vfs::DirectoryPage,
    ) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::read_directory(directory, cookie, page)
    }

    fn read_file_at(
        &self,
        file: &InvocationReceiver,
        offset: u64,
        output: Option<UserSlice>,
    ) -> Result<(u64, u64), VfsServiceError> {
        crate::kernel::vfs::read_file_at(self.process, file, offset, output)
    }

    fn protect_file(
        &self,
        file: &InvocationReceiver,
    ) -> Result<(HandleValue, u64), VfsServiceError> {
        crate::kernel::vfs::protect_file(self.process, file)
    }
    fn file_info(
        &self,
        file: &InvocationReceiver,
    ) -> Result<crate::kernel::vfs::FileInfo, VfsServiceError> {
        crate::kernel::vfs::file_info(file)
    }

    fn directory_info(
        &self,
        directory: &InvocationReceiver,
    ) -> Result<crate::kernel::vfs::DirectoryInfo, VfsServiceError> {
        crate::kernel::vfs::directory_info(directory)
    }
}
