// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use std::borrow::Cow;

use anyhow::{Result, bail};
use futures_util::StreamExt as _;
use sqlx::{SqliteExecutor, SqlitePool};

use endjine::{
    AlbumArt, BatchOutcome, Historylist, HistorylistEntity, PerformanceData, Playlist,
    PlaylistEntity, PreparelistEntity, Smartlist, Track, batch,
};

const DEFAULT_DATABASE_PATH: &str = "m.db";

#[tokio::main]
async fn main() -> Result<()> {
    // In Windows, we must request a virtual terminal environment to display colors correctly. This
    // enables support for the ANSI escape sequences used by `colored`.
    //
    // <https://github.com/colored-rs/colored/issues/59#issuecomment-954355180>
    #[cfg(windows)]
    let _unused = colored::control::set_virtual_terminal(true);

    env_logger::init();

    let database_path = std::env::args()
        .nth(1)
        .map_or_else(|| DEFAULT_DATABASE_PATH.into(), Cow::Owned);

    let database_url = format!("sqlite:{database_path}");
    let pool = match SqlitePool::connect(&database_url).await {
        Ok(pool) => {
            log::info!("Opened database file: {database_path}");
            pool
        }
        Err(err) => {
            log::error!("Failed to open database file {database_path}: {err:#}");
            bail!("aborted");
        }
    };

    ///////////////////////////////////////////////////////////////////
    // Non-modifying operations.                                     //
    ///////////////////////////////////////////////////////////////////

    track_scan(&pool).await;

    playlist_scan(&pool).await;

    playlist_entity_scan(&pool).await;

    smartlist_scan(&pool).await;

    preparelist_entity_scan(&pool).await;

    if historylist_scan(&pool).await {
        historylist_entity_scan(&pool).await;
    }

    performance_data_scan(&pool).await;

    track_reset_unused_default_album_art(&pool).await;

    ///////////////////////////////////////////////////////////////////
    // Modifying operations.                                         //
    ///////////////////////////////////////////////////////////////////

    // let mut tx = pool.begin().await?;
    // batch::purge_album_art(&mut tx).await?;
    // tx.commit().await?;

    performance_data_delete_orphaned(&pool).await;

    album_art_delete_unused(&pool).await;

    album_art_shrink_images(&pool).await;

    optimize_database(&pool).await;

    log::info!("Finished housekeeping");

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
        log::warn!("Track: Scanned {count} rows: {err_count} unreadable");
    } else {
        log::info!("Track: Scanned {count} rows");
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
