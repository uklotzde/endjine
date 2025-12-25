// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

#![expect(rustdoc::invalid_rust_codeblocks)] // Do not interpret code blocks, e.g. license comments.
#![doc = include_str!("../README.md")]

mod album_art;

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
    PlaylistPath, PlaylistPathId, concat_playlist_path_segments_to_string,
    is_valid_playlist_path_segment,
};

mod preparelist;
pub use self::preparelist::{PreparelistEntity, PreparelistEntityId};

mod smartlist;
pub use self::smartlist::{
    Smartlist, SmartlistRules, SmartlistRulesItem, SmartlistRulesMatch, SmartlistUuid,
};

mod track;
pub use self::track::{Track, TrackId};

mod unix_timestamp;
pub use self::unix_timestamp::UnixTimestamp;

#[cfg(feature = "batch")]
pub mod batch;
#[cfg(feature = "batch")]
pub use self::batch::BatchOutcome;
