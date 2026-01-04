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

use anyhow::bail;
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
pub use self::track::{Track, TrackId, TrackRef, import_track_file_path};

mod unix_timestamp;
pub use self::unix_timestamp::UnixTimestamp;

#[cfg(feature = "batch")]
pub mod batch;
#[cfg(feature = "batch")]
pub use self::batch::BatchOutcome;

/// Portable file path.
///
/// Decomposed into minimal base path and (normalized) relative path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePath<'a> {
    base: Cow<'a, Path>,
    relative: Cow<'a, RelativePath>,
}

impl<'a> FilePath<'a> {
    /// Base path.
    ///
    /// Empty for relative file paths.
    #[must_use]
    pub const fn base(&self) -> &Cow<'_, Path> {
        &self.base
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
        let Self { base: _, relative } = self;
        relative
    }

    #[must_use]
    pub(crate) fn to_parent_path(&'a self) -> Option<Self> {
        let Self { base, relative } = self;
        let relative = relative.parent()?;
        Some(Self {
            base: base.clone(),
            relative: relative.into(),
        })
    }
}

impl FilePath<'_> {
    #[must_use]
    pub fn is_relative(&self) -> bool {
        let Self { base, relative: _ } = self;
        base.is_relative()
    }

    /// Imports a file system path.
    #[must_use]
    pub fn import_path<P>(path: &P) -> FilePath<'static>
    where
        P: ?Sized + AsRef<Path>,
    {
        // Monomorphization: Use a single, shared implementation for all generic arg types.
        Self::import_path_impl(path.as_ref())
    }

    #[must_use]
    fn import_path_impl(path: &Path) -> FilePath<'static> {
        if path.is_relative()
            && let Ok(relative) = RelativePath::from_path(path)
        {
            let base = Path::new("").into();
            let relative = relative.normalize().into();
            let file_path = FilePath { base, relative };
            debug_assert!(file_path.is_relative());
            return file_path;
        }
        let mut base_prefix = None;
        let mut base_root_dir = None;
        let relative = path
            .components()
            .filter_map(|component| match component {
                base_component @ Component::Prefix(_) => {
                    debug_assert!(base_root_dir.is_none());
                    base_prefix = Some(base_component);
                    None
                }
                base_component @ Component::RootDir => {
                    base_root_dir = Some(base_component);
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
        let base = base_prefix
            .into_iter()
            .chain(base_root_dir)
            .collect::<PathBuf>()
            .into();
        FilePath { base, relative }
    }

    #[must_use]
    pub(crate) fn into_owned(self) -> FilePath<'static> {
        let Self { base, relative } = self;
        let base = base.into_owned();
        let relative = relative.into_owned();
        FilePath {
            base: Cow::Owned(base),
            relative: Cow::Owned(relative),
        }
    }

    pub(crate) fn add_relative_prefix<P>(&mut self, prefix: &P)
    where
        P: AsRef<RelativePath> + ?Sized,
    {
        let Self { base: _, relative } = self;
        *relative = prefix.as_ref().join_normalized(&relative).into();
    }

    #[must_use]
    pub(crate) fn strip_relative_prefix<P>(&mut self, prefix: &P) -> bool
    where
        P: AsRef<RelativePath> + ?Sized,
    {
        let Self { base: _, relative } = self;
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
        let Self { base, relative } = self;
        relative.to_path(base)
    }
}

impl fmt::Display for FilePath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.to_path().display().fmt(f)
    }
}

pub(crate) const LIBRARY_DIRECTORY_NAME: &str = "Engine Library";

/// Determines the directory that contains the _Engine Library_.
pub fn database_file_to_library_path(
    db_file_path: &FilePath<'_>,
) -> anyhow::Result<FilePath<'static>> {
    let Some(library_path) = grandparent_file_path(db_file_path) else {
        bail!("invalid database file path");
    };
    let Some(dir_name) = library_path.relative().file_name() else {
        // The (relative) library path must not be empty.
        debug_assert!(library_path.relative().as_str().is_empty());
        bail!("invalid library directory");
    };
    if !dir_name.eq_ignore_ascii_case(LIBRARY_DIRECTORY_NAME) {
        bail!("invalid library directory name \"{dir_name}\"");
    }
    Ok(library_path)
}

#[must_use]
fn grandparent_file_path<'a>(file_path: &'a FilePath<'a>) -> Option<FilePath<'static>> {
    let parent_path = file_path.to_parent_path()?;
    parent_path.to_parent_path().map(FilePath::into_owned)
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
            let file_path = FilePath::import_path(path_segment);
            assert!(file_path.is_relative());
            assert_eq!(file_path.base(), empty_root_path);
            assert_eq!(file_path.relative(), RelativePath::new(path_segment));

            let file_path = FilePath::import_path(&root_path.join(path_segment));
            assert!(!file_path.is_relative());
            assert_eq!(file_path.base(), root_path);
            assert_eq!(file_path.relative(), RelativePath::new(path_segment));
        }

        //
        // Multiple path segments with a separator (relative/absolute).
        //

        let file_path = FilePath::import_path(&Path::new("..").join("foo").join("bar").join(".."));
        assert!(file_path.is_relative());
        assert_eq!(file_path.base(), empty_root_path);
        assert_eq!(file_path.relative(), RelativePath::new("../foo"));

        let file_path =
            FilePath::import_path(&root_path.join("..").join("foo").join("bar").join(".."));
        assert!(!file_path.is_relative());
        assert_eq!(file_path.base(), root_path);
        assert_eq!(file_path.relative(), RelativePath::new("../foo"));
    }
}
