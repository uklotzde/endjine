// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

#![allow(unreachable_code)]

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, bail};
use clap::{Parser, Subcommand};
use futures_util::{StreamExt as _, TryStreamExt, stream::FuturesOrdered};
use sqlx::{SqliteExecutor, SqlitePool};

use endjine::{
    AlbumArt, BatchOutcome, DbUuid, Historylist, HistorylistEntity, Information, PerformanceData,
    Playlist, PlaylistEntity, PlaylistTrackRef, PreparelistEntity, RELATIVE_TRACK_PATH_PREFIX,
    Smartlist, Track, batch, open_database, resolve_track_path,
};

const DEFAULT_DB_FILE: &str = "m.db";

#[derive(Debug, Subcommand)]
enum Command {
    /// Scan database for consistency and missing or inaccessible track files (read-only).
    Analyze,
    /// Import playlist from M3U file.
    ImportPlaylist(ImportPlaylistArgs),
    /// Convert album art images from PNG to JPG to save space.
    ShrinkAlbumArt,
    /// Purge all album art for re-import.
    PurgeAlbumArt,
    /// Purge cruft from the database.
    Housekeeping,
    /// Optimize the database.
    Optimize,
}

#[derive(Debug, Parser)]
struct ImportPlaylistArgs {
    /// Path in the playlist hierarchy.
    ///
    /// Composed from the playlist titles. Path segments are separated by semicolons (';').
    ///
    /// Example: "Parent Playlist Title;Child Playlist Title"
    #[arg(long)]
    playlist_path: String,

    /// M3U file path.
    #[arg(long)]
    m3u_file: PathBuf,

    /// Absolute base path for resolving relative paths in the M3U file.
    #[arg(long)]
    m3u_base_path: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    db_file: Option<PathBuf>,

    #[clap(subcommand)]
    command: Command,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // In Windows, we must request a virtual terminal environment to display colors correctly.
    // This enables support for the ANSI escape sequences used by `colored`.
    //
    // <https://github.com/colored-rs/colored/issues/59#issuecomment-954355180>
    #[cfg(windows)]
    let _unused = colored::control::set_virtual_terminal(true);

    env_logger::init();

    let Args { db_file, command } = Args::parse();

    let db_file = db_file.map_or(Cow::Borrowed(Path::new(DEFAULT_DB_FILE)), Cow::Owned);

    let (pool, _info) = match open_database(&db_file, None).await {
        Ok(pool) => {
            log::info!(
                "Opened database file \"{db_file}\"",
                db_file = db_file.display()
            );
            pool
        }
        Err(err) => {
            log::error!(
                "Failed to open database file \"{db_file}\": {err:#}",
                db_file = db_file.display()
            );
            bail!("aborted");
        }
    };

    let info = Information::load(|| &pool).await?;
    log::info!("Database UUID: {uuid}", uuid = info.uuid());

    match command {
        Command::Analyze => {
            track_scan(&pool).await;
            playlist_scan(&pool).await;
            playlist_entity_scan(&pool).await;
            smartlist_scan(&pool).await;
            preparelist_entity_scan(&pool).await;
            if historylist_scan(&pool).await {
                historylist_entity_scan(&pool).await;
            }
            performance_data_scan(&pool).await;
            // Find missing or inaccessible track files.
            if let Some(base_path) = Track::base_path(&db_file) {
                find_track_file_issues(&pool, base_path.to_path_buf()).await;
            } else {
                log::warn!("Cannot resolve base path from database path");
            }
        }
        Command::ShrinkAlbumArt => {
            album_art_shrink_images(&pool).await;
        }
        Command::PurgeAlbumArt => {
            album_art_purge_images(&pool).await;
        }
        Command::ImportPlaylist(ImportPlaylistArgs {
            playlist_path,
            m3u_file,
            m3u_base_path,
        }) => {
            if let Some(track_base_path) = Track::base_path(&db_file) {
                log::info!("Track base path: {}", track_base_path.display());
                match import_m3u_playlist(
                    &pool,
                    *info.uuid(),
                    track_base_path,
                    &playlist_path,
                    &m3u_file,
                    m3u_base_path.as_deref(),
                )
                .await
                {
                    Ok(()) => (),
                    Err(err) => {
                        log::error!(
                            "Failed to import M3U playlist from \"{m3u_file}\": {err:#}",
                            m3u_file = m3u_file.display()
                        );
                    }
                }
            } else {
                log::warn!("Cannot resolve base path from database path");
            }
        }
        Command::Housekeeping => {
            performance_data_delete_orphaned(&pool).await;
            track_reset_unused_default_album_art(&pool).await;
            album_art_delete_unused(&pool).await;
            // TODO
            let playlist_entry_count = PlaylistEntity::delete_all_external(&pool).await?;
            if playlist_entry_count > 0 {
                log::info!("Deleted {playlist_entry_count} playlist entry(-ies)");
            }
            let playlist_count = Playlist::delete_all_empty_without_children(&pool).await?;
            if playlist_count > 0 {
                log::info!("Deleted {playlist_count} playlist(s)");
            }
        }
        Command::Optimize => {
            optimize_database(&pool).await;
        }
    }

    Ok(())
}

async fn track_scan(pool: &SqlitePool) {
    log::info!("Track: Scanning...");
    let (ok_count, err_count) = Track::fetch_all(pool)
        .fold((0, 0), |(ok_count, err_count), result| {
            let counts = match result {
                Ok(_) => (ok_count + 1, err_count),
                Err(err) => {
                    log::warn!("Track: Failed to read row: {err:#}");
                    (ok_count, err_count + 1)
                }
            };
            std::future::ready(counts)
        })
        .await;
    let count = ok_count + err_count;
    if err_count > 0 {
        log::warn!("Track: Scanned {count} row(s): {err_count} unreadable");
    } else {
        log::info!("Track: Scanned {count} row(s)");
    }
}

async fn playlist_scan(pool: &SqlitePool) {
    log::info!("Playlist: Scanning...");
    let (ok_count, err_count) = Playlist::fetch_all(pool)
        .fold((0, 0), |(ok_count, err_count), result| {
            let counts = match result {
                Ok(_) => (ok_count + 1, err_count),
                Err(err) => {
                    log::warn!("Playlist: Failed to read row: {err:#}");
                    (ok_count, err_count + 1)
                }
            };
            std::future::ready(counts)
        })
        .await;
    let count = ok_count + err_count;
    if err_count > 0 {
        log::warn!("Playlist: Scanned {count} row(s): {err_count} unreadable");
    } else {
        log::info!("Playlist: Scanned {count} row(s)");
    }
}

async fn playlist_entity_scan(pool: &SqlitePool) {
    log::info!("PlaylistEntity: Scanning...");
    let (ok_count, err_count) = PlaylistEntity::fetch_all(pool)
        .fold((0, 0), |(ok_count, err_count), result| {
            let counts = match result {
                Ok(_) => (ok_count + 1, err_count),
                Err(err) => {
                    log::warn!("PlaylistEntity: Failed to read row: {err:#}");
                    (ok_count, err_count + 1)
                }
            };
            std::future::ready(counts)
        })
        .await;
    let count = ok_count + err_count;
    if err_count > 0 {
        log::warn!("PlaylistEntity: Scanned {count} row(s): {err_count} unreadable");
    } else {
        log::info!("PlaylistEntity: Scanned {count} row(s)");
    }
}

async fn smartlist_scan(pool: &SqlitePool) -> bool {
    if !matches!(Smartlist::is_available(pool).await, Ok(true)) {
        log::info!("Smartlist: Not available in database");
        return false;
    }
    log::info!("Smartlist: Scanning...");
    let (ok_count, err_count) = Smartlist::fetch_all(pool)
        .fold((0, 0), |(ok_count, err_count), result| {
            let counts = match result {
                Ok(_) => (ok_count + 1, err_count),
                Err(err) => {
                    log::warn!("Smartlist: Failed to read row: {err:#}");
                    (ok_count, err_count + 1)
                }
            };
            std::future::ready(counts)
        })
        .await;
    let count = ok_count + err_count;
    if err_count > 0 {
        log::warn!("Smartlist: Scanned {count} row(s): {err_count} unreadable");
    } else {
        log::info!("Smartlist: Scanned {count} row(s)");
    }
    true
}

async fn preparelist_entity_scan(pool: &SqlitePool) -> bool {
    if !matches!(PreparelistEntity::is_available(pool).await, Ok(true)) {
        log::info!("PreparelistEntity: Not available in database");
        return false;
    }
    log::info!("PreparelistEntity: Scanning...");
    // Try to load all PreparelistEntity(s) from the database to verify the schema definition.
    let (ok_count, err_count) = PreparelistEntity::fetch_all(pool)
        .fold((0, 0), |(ok_count, err_count), result| {
            let counts = match result {
                Ok(_) => (ok_count + 1, err_count),
                Err(err) => {
                    log::warn!("PreparelistEntity: Failed to read row: {err:#}");
                    (ok_count, err_count + 1)
                }
            };
            std::future::ready(counts)
        })
        .await;
    let count = ok_count + err_count;
    if err_count > 0 {
        log::warn!("PreparelistEntity: Scanned {count} row(s): {err_count} unreadable");
    } else {
        log::info!("PreparelistEntity: Scanned {count} row(s)");
    }
    true
}

async fn historylist_scan(pool: &SqlitePool) -> bool {
    if !matches!(Historylist::is_available(pool).await, Ok(true)) {
        log::info!("Historylist: Not available in database");
        return false;
    }
    log::info!("Historylist: Scanning...");
    // Try to load all Historylist(s) from the database to verify the schema definition.
    let (ok_count, err_count) = Historylist::fetch_all(pool)
        .fold((0, 0), |(ok_count, err_count), result| {
            let counts = match result {
                Ok(_) => (ok_count + 1, err_count),
                Err(err) => {
                    log::warn!("Historylist: Failed to read row: {err:#}");
                    (ok_count, err_count + 1)
                }
            };
            std::future::ready(counts)
        })
        .await;
    let count = ok_count + err_count;
    if err_count > 0 {
        log::warn!("Historylist: Scanned {count} row(s): {err_count} unreadable");
    } else {
        log::info!("Historylist: Scanned {count} row(s)");
    }
    true
}

async fn historylist_entity_scan(pool: &SqlitePool) {
    log::info!("HistorylistEntity: Scanning...");
    // Try to load all HistorylistEntity(s) from the database to verify the schema definition.
    let (ok_count, err_count) = HistorylistEntity::fetch_all(pool)
        .fold((0, 0), |(ok_count, err_count), result| {
            let counts = match result {
                Ok(_) => (ok_count + 1, err_count),
                Err(err) => {
                    log::warn!("HistorylistEntity: Failed to read row: {err:#}");
                    (ok_count, err_count + 1)
                }
            };
            std::future::ready(counts)
        })
        .await;
    let count = ok_count + err_count;
    if err_count > 0 {
        log::warn!("HistorylistEntity: Scanned {count} rows(s): {err_count} unreadable");
    } else {
        log::info!("HistorylistEntity: Scanned {count} rows(s)");
    }
}

async fn performance_data_scan(pool: &SqlitePool) {
    log::info!("PerformanceData: Scanning...");
    // Try to load all PerformanceData from the database to verify the schema definition.
    let (ok_count, err_count) = PerformanceData::fetch_all(pool)
        .fold((0, 0), |(ok_count, err_count), result| {
            let counts = match result {
                Ok(_) => (ok_count + 1, err_count),
                Err(err) => {
                    log::warn!("PerformanceData: Failed to read row: {err:#}");
                    (ok_count, err_count + 1)
                }
            };
            std::future::ready(counts)
        })
        .await;
    let count = ok_count + err_count;
    if err_count > 0 {
        log::warn!("PerformanceData: Scanned {count} rows: {err_count} unreadable");
    } else {
        log::info!("PerformanceData: Scanned {count} rows(s)");
    }
}

async fn find_track_file_issues(pool: &SqlitePool, base_path: PathBuf) {
    log::info!("Track: Scanning for file issues...");
    batch::find_track_file_issues(pool, base_path)
        .for_each(|next_result| {
            match next_result {
                Ok(batch::TrackFileIssueItem { db_id, db_path, file_path, file_issue }) => match file_issue {
                    batch::TrackFileIssue::FileMissing => {
                        log::warn!(
                            "Track: File \"{file_path}\" of track {db_id} with path \"{db_path}\" is missing",
                            file_path = file_path.display()
                        );
                    }
                    batch::TrackFileIssue::FileError(err) => {
                        log::warn!(
                            "Track: File \"{file_path}\" of track {db_id} with path \"{db_path}\" is inaccessible: {err:#}",
                            file_path = file_path.display()
                        );
                    }
                },
                Err(err) => {
                    // Should not occur.
                    log::error!("Database error: {err:#}");
                }
            }
            std::future::ready(())
        })
        .await;
}

async fn performance_data_delete_orphaned(pool: &SqlitePool) {
    log::info!("PerformanceData: Deleting orphaned...");
    match PerformanceData::delete_orphaned(pool).await {
        Ok(rows_affected) => {
            if rows_affected > 0 {
                log::info!("PerformanceData: Deleted {rows_affected} orphaned row(s)");
            } else {
                log::info!("PerformanceData: No orphaned rows found");
            }
        }
        Err(err) => {
            log::warn!("PerformanceData: Failed to delete orphaned: {err}");
        }
    }
}

async fn track_reset_unused_default_album_art(executor: impl SqliteExecutor<'_>) {
    log::info!("Track: Resetting unused default album art...");
    match Track::reset_unused_default_album_art(executor).await {
        Ok(rows_affected) => {
            if rows_affected > 0 {
                log::info!(
                    "Track: Reset {rows_affected} row(s) with unused default album art \"{}\"",
                    Track::DEFAULT_ALBUM_ART
                );
            } else {
                log::info!("Track: No unused default album art found");
            }
        }
        Err(err) => {
            log::warn!("Track: Failed to reset unused default album art: {err}");
        }
    }
}

async fn album_art_delete_unused(pool: &SqlitePool) {
    log::info!("AlbumArt: Deleting unused...");
    match AlbumArt::delete_unused(pool).await {
        Ok(rows_affected) => {
            if rows_affected > 0 {
                log::info!("AlbumArt: Deleted {rows_affected} unused row(s)");
            } else {
                log::info!("AlbumArt: No unused found");
            }
        }
        Err(err) => {
            log::warn!("AlbumArt: Failed to delete unused: {err}");
        }
    }
}

async fn album_art_shrink_images(pool: &SqlitePool) {
    log::info!("AlbumArt: Shrinking images...");
    {
        let BatchOutcome {
            succeeded,
            skipped,
            failed,
            aborted_error,
        } = batch::shrink_album_art_images(pool, endjine::AlbumArtImageQuality::Low).await;
        log::info!(
            "AlbumArt: Shrinking of images finished: succeeded = {succeeded}, skipped = {skipped}, failed = {failed}",
            failed = failed.len()
        );
        if let Some(err) = aborted_error {
            log::warn!("AlbumArt: Shrinking of images aborted with error: {err}");
        }
    }
}

async fn album_art_purge_images(pool: &SqlitePool) {
    log::info!("AlbumArt: Purging images...");
    {
        // TODO: Extract function.
        let purge_album_art_result: anyhow::Result<u64> = async move {
            let mut tx = pool.begin().await?;
            let purged_count = batch::purge_album_art(&mut tx).await?;
            tx.commit().await?;
            Ok(purged_count)
        }
        .await;
        match purge_album_art_result {
            Ok(purged_count) => {
                log::info!("AlbumArt: Purged {purged_count} image(s)");
            }
            Err(err) => {
                log::warn!("AlbumArt: Purging of images aborted with error: {err}");
            }
        }
    }
}

fn import_m3u_playlist_track_paths(
    file_path: &Path,
    base_path: Option<&Path>,
) -> anyhow::Result<Vec<PathBuf>> {
    let reader = m3u::Reader::open(file_path)?;
    let base_path = base_path.unwrap_or_else(|| Path::new(RELATIVE_TRACK_PATH_PREFIX));
    let mut reader = reader;
    reader
        .entries()
        .map(|entry_result| {
            entry_result.map_err(Into::into).and_then(|entry| {
                let mut path = match entry {
                    m3u::Entry::Path(path) => path,
                    m3u::Entry::Url(url) => match url.to_file_path() {
                        Ok(path) => path,
                        Err(()) => {
                            bail!("URL \"{url}\" is not a local file path");
                        }
                    },
                };
                if path.is_relative() {
                    path = base_path.join(path);
                }
                Ok(path)
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()
}

async fn import_m3u_playlist(
    pool: &SqlitePool,
    local_database_uuid: DbUuid,
    track_base_path: &Path,
    playlist_path: &str,
    m3u_file: &Path,
    m3u_base_path: Option<&Path>,
) -> anyhow::Result<()> {
    let imported_track_paths = import_m3u_playlist_track_paths(m3u_file, m3u_base_path)?;

    log::info!(
        "Resolving id(s) of {track_count} track(s)",
        track_count = imported_track_paths.len()
    );

    let track_refs_fut = imported_track_paths
        .into_iter()
        .map(|track_path: PathBuf| {
            resolve_track_path(track_base_path, &track_path)
                .map(Cow::into_owned)
                .map(|track_path| async move {
                    let track_ref = Track::find_ref_by_path(pool, &track_path)
                        .await
                        .with_context(|| {
                            format!(
                                "find id of track path \"{track_path}\"",
                                track_path = track_path.display()
                            )
                        })?;
                    let Some(track_ref) = track_ref else {
                        bail!(
                            "unknown track path \"{track_path}\"",
                            track_path = track_path.display()
                        );
                    };
                    PlaylistTrackRef::new(track_ref, local_database_uuid)
                })
                .with_context(|| {
                    format!(
                        "resolve track path \"{track_path}\"",
                        track_path = track_path.display(),
                    )
                })
        })
        .collect::<anyhow::Result<FuturesOrdered<_>>>()?;
    let track_refs = track_refs_fut.try_collect::<Vec<_>>().await?;
    let Some(playlist_id) = Playlist::find_id_by_path(pool, playlist_path).await? else {
        // TODO: Create new playlist.
        log::warn!("Playlist \"{playlist_path}\" not found");
        return Ok(());
    };
    Playlist::replace_tracks(|| pool, playlist_id, track_refs).await
}

async fn optimize_database(pool: &SqlitePool) {
    log::info!("Optimizing database...");
    match endjine::optimize_database(pool).await {
        Ok(()) => {
            log::info!("Optimized database");
        }
        Err(err) => {
            log::warn!("Failed to optimize database: {err}");
        }
    }
}
