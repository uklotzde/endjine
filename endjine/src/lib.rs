// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

#![expect(rustdoc::invalid_rust_codeblocks)] // Do not interpret code blocks, e.g. license comments.
#![doc = include_str!("../README.md")]

mod album_art;

use std::{
    borrow::Cow,
    fmt,
    path::{Component, Path, PathBuf},
};

use relative_path::{RelativePath, RelativePathBuf};

pub use self::album_art::{AlbumArt, AlbumArtId, AlbumArtImageQuality};

mod changelog;
pub use self::changelog::{ChangeLog, ChangeLogId};

mod database;
pub use self::database::{open_database, optimize_database};

mod db_id;

mod db_uuid;
pub use self::db_uuid::DbUuid;

mod historylist;
pub use self::historylist::{Historylist, HistorylistEntity, HistorylistEntityId, HistorylistId};

mod information;
pub use self::information::{
    Information, InformationId, SCHEMA_VERSION_MAJOR, SCHEMA_VERSION_MINOR,
};

mod pack;
pub use self::pack::{Pack, PackId, PackUuid};

mod performance;
pub use self::performance::{PerformanceData, PerformanceDataId};

mod playlist;
pub use self::playlist::{
    PLAYLIST_PATH_SEGMENT_SEPARATOR, Playlist, PlaylistAllChildren, PlaylistAllChildrenId,
    PlaylistAllParent, PlaylistAllParentId, PlaylistEntity, PlaylistEntityId, PlaylistId,
    PlaylistPath, PlaylistPathId, PlaylistTrackRef, concat_playlist_path_segments_to_string,
    is_valid_playlist_path_segment, resolve_playlist_track_refs_from_file_paths,
};

mod preparelist;
pub use self::preparelist::{PreparelistEntity, PreparelistEntityId};

mod smartlist;
pub use self::smartlist::{
    Smartlist, SmartlistRules, SmartlistRulesItem, SmartlistRulesMatch, SmartlistUuid,
};

mod track;
pub use self::track::{
    RELATIVE_TRACK_PATH_PREFIX, Track, TrackId, TrackRef, import_track_file_path,
};

mod unix_timestamp;
pub use self::unix_timestamp::UnixTimestamp;

#[cfg(feature = "batch")]
pub mod batch;
#[cfg(feature = "batch")]
pub use self::batch::BatchOutcome;

/// Portable file path.
///
/// Decomposed into root base path and (normalized) relative path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePath<'a> {
    root: Cow<'a, Path>,
    relative: Cow<'a, RelativePath>,
}

impl<'a> FilePath<'a> {
    /// Root base path.
    ///
    /// Empty for relative file paths.
    #[must_use]
    pub const fn root(&self) -> &Cow<'_, Path> {
        &self.root
    }

    /// Relative path part.
    ///
    /// Already normalized.
    #[must_use]
    pub const fn relative(&self) -> &Cow<'_, RelativePath> {
        &self.relative
    }

    /// Relative path part.
    ///
    /// Already normalized.
    #[must_use]
    pub fn into_relative(self) -> Cow<'a, RelativePath> {
        let Self { root: _, relative } = self;
        relative
    }

    #[must_use]
    pub(crate) fn to_parent_path(&'a self) -> Option<Self> {
        let Self { root, relative } = self;
        let relative = relative.parent()?;
        Some(Self {
            root: root.clone(),
            relative: relative.into(),
        })
    }
}

impl FilePath<'_> {
    #[must_use]
    pub fn is_relative(&self) -> bool {
        let Self { root, relative: _ } = self;
        root.is_relative()
    }

    /// Imports a file system path.
    pub fn import_path<P>(path: &P) -> anyhow::Result<FilePath<'static>>
    where
        P: ?Sized + AsRef<Path>,
    {
        // Monomorphization: Use a single, shared implementation for all generic arg types.
        Self::import_path_impl(path.as_ref())
    }

    fn import_path_impl(path: &Path) -> anyhow::Result<FilePath<'static>> {
        if path.is_relative() {
            let relative = RelativePath::from_path(path)?;
            let root = Path::new("").into();
            let relative = relative.normalize().into();
            let file_path = FilePath { root, relative };
            debug_assert!(file_path.is_relative());
            return Ok(file_path);
        }
        debug_assert!(path.is_absolute());
        let mut root_components = Vec::with_capacity(2);
        let relative = path
            .components()
            .filter_map(|component| match component {
                root_component @ (Component::Prefix(_) | Component::RootDir) => {
                    root_components.push(root_component);
                    None
                }
                Component::CurDir => None,
                Component::ParentDir => Some(relative_path::Component::ParentDir),
                Component::Normal(normal) => normal.to_str().map(relative_path::Component::Normal),
            })
            .collect::<RelativePathBuf>()
            // TODO: How to avoid duplicate allocation by collect + normalize?
            .normalize()
            .into();
        let root = root_components.into_iter().collect::<PathBuf>().into();
        let file_path = FilePath { root, relative };
        debug_assert!(!file_path.is_relative());
        Ok(file_path)
    }

    #[must_use]
    pub(crate) fn into_owned(self) -> FilePath<'static> {
        let Self { root, relative } = self;
        let root = root.into_owned();
        let relative = relative.into_owned();
        FilePath {
            root: Cow::Owned(root),
            relative: Cow::Owned(relative),
        }
    }

    pub(crate) fn add_relative_prefix<P>(&mut self, prefix: &P)
    where
        P: AsRef<RelativePath> + ?Sized,
    {
        let Self { root: _, relative } = self;
        *relative = prefix.as_ref().join_normalized(&relative).into();
    }

    #[must_use]
    pub(crate) fn strip_relative_prefix<P>(&mut self, prefix: &P) -> bool
    where
        P: AsRef<RelativePath> + ?Sized,
    {
        let Self { root: _, relative } = self;
        let Ok(stripped) = relative.strip_prefix(prefix) else {
            // Prefix mismatch.
            return false;
        };
        debug_assert!(stripped.as_str().len() <= relative.as_str().len());
        if stripped != relative {
            // We have to re-allocate and cannot reuse the suffix part.
            *relative = Cow::Owned(stripped.to_relative_path_buf());
        }
        true
    }

    /// Reconstructs the file system path.
    #[must_use]
    pub fn to_path(&self) -> PathBuf {
        let Self { root, relative } = self;
        relative.to_path(root)
    }
}

impl fmt::Display for FilePath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.to_path().display().fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use relative_path::RelativePath;

    use crate::FilePath;

    #[test]
    fn import_file_path() {
        let empty_root_path = Path::new("");

        #[cfg(target_os = "windows")]
        let root_path = Path::new("C:\\");
        #[cfg(not(target_os = "windows"))]
        let root_path = Path::new("/");
        assert!(root_path.is_absolute());

        //
        // 1 path segment without a separator (relative/absolute).
        //

        for path_segment in ["..", "foo"] {
            let file_path = FilePath::import_path(path_segment).unwrap();
            assert!(file_path.is_relative());
            assert_eq!(file_path.root(), empty_root_path);
            assert_eq!(file_path.relative(), RelativePath::new(path_segment));

            let file_path = FilePath::import_path(&root_path.join(path_segment)).unwrap();
            assert!(!file_path.is_relative());
            assert_eq!(file_path.root(), root_path);
            assert_eq!(file_path.relative(), RelativePath::new(path_segment));
        }

        //
        // Multiple path segments with a separator (relative/absolute).
        //

        let file_path =
            FilePath::import_path(&Path::new("..").join("foo").join("bar").join("..")).unwrap();
        assert!(file_path.is_relative());
        assert_eq!(file_path.root(), empty_root_path);
        assert_eq!(file_path.relative(), RelativePath::new("../foo"));

        let file_path =
            FilePath::import_path(&root_path.join("..").join("foo").join("bar").join(".."))
                .unwrap();
        assert!(!file_path.is_relative());
        assert_eq!(file_path.root(), root_path);
        assert_eq!(file_path.relative(), RelativePath::new("../foo"));
    }
}
