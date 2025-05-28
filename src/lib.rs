// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

#![expect(rustdoc::invalid_rust_codeblocks)] // Do not interpret code blocks, e.g. license comments.
#![doc = include_str!("../README.md")]

mod album_art;
pub use self::album_art::{
    AlbumArt, AlbumArtId, album_art_delete_orphaned, album_art_fetch_all, album_art_try_load,
    album_art_update_image,
};

mod db_id;

mod changelog;
pub use self::changelog::{ChangeLog, ChangeLogId};

mod information;
pub use self::information::{
    Information, InformationId, SCHEMA_VERSION_MAJOR, SCHEMA_VERSION_MINOR, information_fetch_all,
    information_try_load,
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

mod smartlist;
pub use self::smartlist::{Smartlist, smartlist_fetch_all, smartlist_try_load};

mod track;
pub use self::track::{Track, TrackId};

#[cfg(feature = "batch")]
mod batch;
#[cfg(feature = "batch")]
pub use self::batch::{BatchOutcome, shrink_album_art};

mod util;
pub use self::util::optimize_database;
