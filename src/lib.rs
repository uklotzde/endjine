// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

#![expect(rustdoc::invalid_rust_codeblocks)] // Do not interpret code blocks, e.g. license comments.
#![doc = include_str!("../README.md")]

mod album_art;
pub use self::album_art::{AlbumArt, AlbumArtId};

mod db_id;

mod db_uuid;
pub use self::db_uuid::DbUuid;

mod changelog;
pub use self::changelog::{ChangeLog, ChangeLogId};

mod information;
pub use self::information::{
    Information, InformationId, SCHEMA_VERSION_MAJOR, SCHEMA_VERSION_MINOR,
};

mod pack;
pub use self::pack::{Pack, PackId};

mod performance;
pub use self::performance::{PerformanceData, PerformanceDataId};

mod playlist;
pub use self::playlist::{
    Playlist, PlaylistAllChildren, PlaylistAllChildrenId, PlaylistAllParent, PlaylistAllParentId,
    PlaylistEntity, PlaylistEntityId, PlaylistId, PlaylistPath, PlaylistPathId,
};

mod preparelist;
pub use self::preparelist::{PreparelistEntity, PreparelistEntityId};

pub mod smartlist;
pub use self::smartlist::Smartlist;

mod track;
pub use self::track::{Track, TrackId};

#[cfg(feature = "batch")]
pub mod batch;
#[cfg(feature = "batch")]
pub use self::batch::BatchOutcome;

mod util;
pub use self::util::optimize_database;
