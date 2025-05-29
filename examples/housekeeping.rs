// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use std::borrow::Cow;

use anyhow::{Result, bail};
use futures_util::StreamExt as _;
use sqlx::SqlitePool;

use endjine::{
    AlbumArt, BatchOutcome, Historylist, HistorylistEntity, PerformanceData, Playlist,
    PlaylistEntity, Smartlist, Track, batch,
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

    scan_tracks(&pool).await;

    scan_playlists(&pool).await;

    scan_playlist_entities(&pool).await;

    scan_smartlists(&pool).await;

    if scan_historylists(&pool).await {
        scan_historylist_entities(&pool).await;
    }

    scan_performance_data(&pool).await;

    delete_orphaned_performance_data(&pool).await;

    delete_unused_album_art(&pool).await;

    shrink_album_art_images(&pool).await;

    optimize_database(&pool).await;

    log::info!("Finished housekeeping");

    Ok(())
}

async fn scan_tracks(pool: &SqlitePool) {
    log::info!("Scanning Track...");
    // Try to load all Playlists from the database to verify the schema definition.
    let (ok_count, err_count) = Track::fetch_all(pool)
        .fold((0, 0), |(ok_count, err_count), result| {
            let counts = match result {
                Ok(_) => (ok_count + 1, err_count),
                Err(err) => {
                    log::warn!("Failed to fetch Track: {err:#}");
                    (ok_count, err_count + 1)
                }
            };
            std::future::ready(counts)
        })
        .await;
    let count = ok_count + err_count;
    if err_count > 0 {
        log::warn!("Found {count} Tracks(s): {err_count} unreadable");
    } else {
        log::info!("Found {count} Tracks(s)");
    }
}

async fn scan_playlists(pool: &SqlitePool) {
    log::info!("Scanning Playlist...");
    // Try to load all Playlists from the database to verify the schema definition.
    let (ok_count, err_count) = Playlist::fetch_all(pool)
        .fold((0, 0), |(ok_count, err_count), result| {
            let counts = match result {
                Ok(_) => (ok_count + 1, err_count),
                Err(err) => {
                    log::warn!("Failed to fetch Playlist: {err:#}");
                    (ok_count, err_count + 1)
                }
            };
            std::future::ready(counts)
        })
        .await;
    let count = ok_count + err_count;
    if err_count > 0 {
        log::warn!("Found {count} Playlists(s): {err_count} unreadable");
    } else {
        log::info!("Found {count} Playlists(s)");
    }
}

async fn scan_playlist_entities(pool: &SqlitePool) {
    log::info!("Scanning PlaylistEntity...");
    // Try to load all Playlists from the database to verify the schema definition.
    let (ok_count, err_count) = PlaylistEntity::fetch_all(pool)
        .fold((0, 0), |(ok_count, err_count), result| {
            let counts = match result {
                Ok(_) => (ok_count + 1, err_count),
                Err(err) => {
                    log::warn!("Failed to fetch PlaylistEntity: {err:#}");
                    (ok_count, err_count + 1)
                }
            };
            std::future::ready(counts)
        })
        .await;
    let count = ok_count + err_count;
    if err_count > 0 {
        log::warn!("Found {count} PlaylistEntity(s): {err_count} unreadable");
    } else {
        log::info!("Found {count} PlaylistEntity(s)");
    }
}

async fn scan_smartlists(pool: &SqlitePool) -> bool {
    if !matches!(Smartlist::is_available(pool).await, Ok(true)) {
        log::info!("Smartlist not available in database");
        return false;
    }
    log::info!("Scanning Smartlist...");
    // Try to load all Smartlists from the database to verify the schema definition.
    let (ok_count, err_count) = Smartlist::fetch_all(pool)
        .fold((0, 0), |(ok_count, err_count), result| {
            let counts = match result {
                Ok(_) => (ok_count + 1, err_count),
                Err(err) => {
                    log::warn!("Failed to fetch Smartlist: {err:#}");
                    (ok_count, err_count + 1)
                }
            };
            std::future::ready(counts)
        })
        .await;
    let count = ok_count + err_count;
    if err_count > 0 {
        log::warn!("Found {count} Smartlist(s): {err_count} unreadable");
    } else {
        log::info!("Found {count} Smartlist(s)");
    }
    true
}

async fn scan_historylists(pool: &SqlitePool) -> bool {
    if !matches!(Historylist::is_available(pool).await, Ok(true)) {
        log::info!("Historylist not available in database");
        return false;
    }
    log::info!("Scanning Historylist...");
    // Try to load all Historylists from the database to verify the schema definition.
    let (ok_count, err_count) = Historylist::fetch_all(pool)
        .fold((0, 0), |(ok_count, err_count), result| {
            let counts = match result {
                Ok(_) => (ok_count + 1, err_count),
                Err(err) => {
                    log::warn!("Failed to fetch Historylist: {err:#}");
                    (ok_count, err_count + 1)
                }
            };
            std::future::ready(counts)
        })
        .await;
    let count = ok_count + err_count;
    if err_count > 0 {
        log::warn!("Found {count} Historylist(s): {err_count} unreadable");
    } else {
        log::info!("Found {count} Historylist(s)");
    }
    true
}

async fn scan_historylist_entities(pool: &SqlitePool) {
    log::info!("Scanning HistorylistEntity...");
    // Try to load all Historylists from the database to verify the schema definition.
    let (ok_count, err_count) = HistorylistEntity::fetch_all(pool)
        .fold((0, 0), |(ok_count, err_count), result| {
            let counts = match result {
                Ok(_) => (ok_count + 1, err_count),
                Err(err) => {
                    log::warn!("Failed to fetch HistorylistEntity: {err:#}");
                    (ok_count, err_count + 1)
                }
            };
            std::future::ready(counts)
        })
        .await;
    let count = ok_count + err_count;
    if err_count > 0 {
        log::warn!("Found {count} HistorylistEntity(s): {err_count} unreadable");
    } else {
        log::info!("Found {count} HistorylistEntity(s)");
    }
}

async fn scan_performance_data(pool: &SqlitePool) {
    log::info!("Scanning PerformanceData...");
    // Try to load all PerformanceData from the database to verify the schema definition.
    let (ok_count, err_count) = PerformanceData::fetch_all(pool)
        .fold((0, 0), |(ok_count, err_count), result| {
            let counts = match result {
                Ok(_) => (ok_count + 1, err_count),
                Err(err) => {
                    log::warn!("Failed to fetch PerformanceData: {err:#}");
                    (ok_count, err_count + 1)
                }
            };
            std::future::ready(counts)
        })
        .await;
    let count = ok_count + err_count;
    if err_count > 0 {
        log::warn!("Found {count} PerformanceData(s): {err_count} unreadable");
    } else {
        log::info!("Found {count} PerformanceData(s)");
    }
}

async fn delete_orphaned_performance_data(pool: &SqlitePool) {
    log::info!("Deleting orphaned PerformanceData...");
    match PerformanceData::delete_orphaned(pool).await {
        Ok(rows_affected) => {
            if rows_affected > 0 {
                log::info!("Deleted {rows_affected} row(s) of orphaned PerformanceData");
            } else {
                log::info!("No orphaned PerformanceData found");
            }
        }
        Err(err) => {
            log::warn!("Failed to delete orphaned PerformanceData: {err}");
        }
    }
}

async fn delete_unused_album_art(pool: &SqlitePool) {
    log::info!("Deleting unused AlbumArt...");
    match AlbumArt::delete_unused(pool).await {
        Ok(rows_affected) => {
            if rows_affected > 0 {
                log::info!("Deleted {rows_affected} row(s) of unused AlbumArt");
            } else {
                log::info!("No unused AlbumArt found");
            }
        }
        Err(err) => {
            log::warn!("Failed to delete unused AlbumArt: {err}");
        }
    }
}

async fn shrink_album_art_images(pool: &SqlitePool) {
    log::info!("Shrinking AlbumArt images...");
    {
        let BatchOutcome {
            succeeded,
            skipped,
            failed,
            aborted_error,
        } = batch::shrink_album_art(pool, endjine::AlbumArtImageQuality::Low).await;
        log::info!(
            "Shrinking of AlbumArt images finished: succeeded = {succeeded}, skipped = {skipped}, failed = {failed}",
            failed = failed.len()
        );
        if let Some(err) = aborted_error {
            log::warn!("Shrinking of AlbumArt images aborted with error: {err}");
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
