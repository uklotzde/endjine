// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

#![expect(rustdoc::invalid_rust_codeblocks)] // Do not interpret code blocks, e.g. license comments.
#![doc = include_str!("../README.md")]

mod album_art;
pub use self::album_art::{AlbumArt, AlbumArtId, AlbumArtImageQuality};

mod changelog;
pub use self::changelog::{ChangeLog, ChangeLogId};

mod db_id;

mod db_uuid;
pub use self::db_uuid::DbUuid;

mod historylist;
pub use self::historylist::{Historylist, HistorylistEntry, HistorylistEntryId, HistorylistId};

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
    Playlist, PlaylistAllChildren, PlaylistAllChildrenId, PlaylistAllParent, PlaylistAllParentId,
    PlaylistEntry, PlaylistEntryId, PlaylistId, PlaylistPath, PlaylistPathId,
};

mod preparelist;
pub use self::preparelist::{PreparelistEntry, PreparelistEntryId};

mod smartlist;
pub use self::smartlist::{
    Smartlist, SmartlistRules, SmartlistRulesItem, SmartlistRulesMatch, SmartlistUuid,
};

mod track;
pub use self::track::{Track, TrackId};

#[cfg(feature = "batch")]
pub mod batch;
#[cfg(feature = "batch")]
pub use self::batch::BatchOutcome;

mod util;
pub use self::util::optimize_database;
