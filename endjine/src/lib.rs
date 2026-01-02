// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

#![expect(rustdoc::invalid_rust_codeblocks)] // Do not interpret code blocks, e.g. license comments.
#![doc = include_str!("../README.md")]

mod album_art;

use std::borrow::Cow;
use std::path::{Component, Path, PathBuf};

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
    RELATIVE_TRACK_PATH_PREFIX, Track, TrackId, TrackRef, normalize_track_file_path,
};

mod unix_timestamp;
pub use self::unix_timestamp::UnixTimestamp;

#[cfg(feature = "batch")]
pub mod batch;
#[cfg(feature = "batch")]
pub use self::batch::BatchOutcome;

#[must_use]
pub fn split_and_normalize_file_path(file_path: &Path) -> (PathBuf, Cow<'_, RelativePath>) {
    if file_path.is_relative()
        && let Ok(relative_path) = RelativePath::from_path(file_path)
    {
        return (PathBuf::new(), Cow::Borrowed(relative_path));
    }
    debug_assert!(file_path.is_absolute());
    let mut root_components = Vec::with_capacity(2);
    let normalized_path = file_path
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
    let root_path = root_components.into_iter().collect::<PathBuf>();
    (root_path, normalized_path)
}
