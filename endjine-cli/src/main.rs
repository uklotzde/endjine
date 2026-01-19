// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

#![allow(unreachable_code)]

use std::{
    borrow::Cow,
    env, io,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, bail};
use clap::{Parser, Subcommand, ValueEnum};
use futures_util::StreamExt as _;
use log::LevelFilter;
use sqlx::{SqliteExecutor, SqlitePool};

use endjine::{
    AlbumArt, BatchOutcome, DbUuid, FilePath, Historylist, HistorylistEntity, Information,
    LibraryPath, PerformanceData, Playlist, PlaylistEntity, PreparelistEntity, Smartlist, Track,
    batch, open_database, resolve_playlist_track_refs_from_file_paths,
};

/// Default log level for debug builds.
#[cfg(debug_assertions)]
const DEFAULT_LOG_FILTER_LEVEL: LevelFilter = LevelFilter::Debug;

/// Reduce log verbosity for release builds.
#[cfg(not(debug_assertions))]
const DEFAULT_LOG_FILTER_LEVEL: LevelFilter = LevelFilter::Info;

const DEFAULT_DB_FILE: &str = "m.db";

#[derive(Debug, Subcommand)]
enum Command {
    /// Scan database for consistency and missing or inaccessible track files (read-only).
    Analyze,
    /// Find missing or inaccessible track files (read-only).
    FindMissingTracks,
    /// Import playlist from M3U file.
    ImportPlaylist(ImportPlaylistArgs),
    /// Delete all empty playlists.
    DeleteEmptyPlaylists,
    /// Convert album art images from PNG to JPG to save space.
    ShrinkAlbumArt,
    /// Purge all album art for re-import.
    PurgeAlbumArt,
    /// Purge cruft from the database.
    Housekeeping,
    /// Optimize the database.
    Optimize,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum ImportPlaylistMode {
    /// Appends tracks to a playlist.
    #[default]
    Append,
    /// Replaces all tracks of a playlist.
    Replace,
}

#[derive(Debug, Parser)]
struct ImportPlaylistArgs {
    /// M3U file path.
    ///
    /// Optional. Defaults to reading from stdin instead of a file.
    #[arg(long)]
    m3u_file: Option<PathBuf>,

    /// Absolute base path for resolving relative M3U file paths.
    ///
    /// Optional. Defaults to the parent directory of the M3U file.
    #[arg(long)]
    m3u_base_path: Option<PathBuf>,

    /// Path in the playlist hierarchy.
    ///
    /// Optional. Defaults to the M3U file name without extension.
    ///
    /// The playlist path in Engine DJ is composed from the playlist titles
    /// in the library hierarchy. Path segments are separated by semicolons (';').
    /// A trailing semicolon is allowed.
    ///
    /// Example: "Parent Title;Child Title" or "Parent Title;Child Title;"
    #[arg(long)]
    playlist_path: Option<String>,

    /// Controls how tracks are added to the playlist.
    ///
    /// Optional. Defaults to "append" (non-destructive).
    #[arg(long)]
    mode: Option<ImportPlaylistMode>,
}

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    db_file: Option<PathBuf>,

    #[clap(subcommand)]
    command: Command,
}

#[expect(clippy::too_many_lines, reason = "TODO")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::new()
        .filter_level(DEFAULT_LOG_FILTER_LEVEL)
        // Parse environment variables after configuring all default option(s).
        .parse_default_env()
        .init();

    let Args { db_file, command } = Args::parse();

    let mut db_file_path = db_file.map_or(Cow::Borrowed(Path::new(DEFAULT_DB_FILE)), Cow::Owned);
    if db_file_path.is_relative() {
        let current_dir = env::current_dir()?;
        debug_assert!(current_dir.is_absolute());
        db_file_path = Cow::Owned(current_dir.join(db_file_path));
    }
    debug_assert!(db_file_path.is_absolute());

    let (pool, _info) = match open_database(&db_file_path, None).await {
        Ok(pool) => {
            log::info!(
                "Opened database file \"{db_file_path}\"",
                db_file_path = db_file_path.display()
            );
            pool
        }
        Err(err) => {
            log::error!(
                "Failed to open database file \"{db_file_path}\": {err:#}",
                db_file_path = db_file_path.display()
            );
            bail!("aborted");
        }
    };

    let db_file_path = FilePath::import_path(&db_file_path);
    let library_path = match LibraryPath::new(&db_file_path) {
        Ok(library_path) => library_path,
        Err(err) => {
            log::warn!(
                "Failed to determine library directory from database file path \"{db_file_path}\": {err:#}"
            );
            return Ok(());
        }
    };
    log::info!("Library directory: {library_path}");

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
        }
        Command::FindMissingTracks => {
            find_track_file_issues(&pool, library_path.to_path()).await;
        }
        Command::DeleteEmptyPlaylists => {
            playlist_delete_empty(&pool).await;
        }
        Command::ShrinkAlbumArt => {
            album_art_shrink_images(&pool).await;
        }
        Command::PurgeAlbumArt => {
            album_art_purge_images(&pool).await;
        }
        Command::ImportPlaylist(ImportPlaylistArgs {
            playlist_path,
            mode,
            m3u_file,
            m3u_base_path,
        }) => {
            let mode = mode.unwrap_or_default();
            let Some(playlist_path) = playlist_path.map(Cow::Owned).or_else(|| {
                m3u_file
                    .as_deref()
                    .and_then(Path::file_prefix)
                    .and_then(|file_name| file_name.to_str().map(Cow::Borrowed))
            }) else {
                bail!("Missing playlist path");
            };
            log::info!("Playlist path: {playlist_path}");
            let source = if let Some(m3u_file) = &m3u_file {
                Cow::Owned(format!("file \"{}\"", m3u_file.display()))
            } else {
                Cow::Borrowed("stdin")
            };
            log::info!("Importing M3U playlist from {source}");
            match import_playlist_from_m3u_file(
                &pool,
                *info.uuid(),
                &library_path,
                &playlist_path,
                mode,
                m3u_file.as_deref(),
                m3u_base_path.as_deref(),
            )
            .await
            {
                Ok(()) => (),
                Err(err) => {
                    bail!("Failed to import M3U playlist from {source}: {err:#}");
                }
            }
        }
        Command::Housekeeping => {
            performance_data_delete_orphaned(&pool).await;
            track_reset_unused_default_album_art(&pool).await;
            album_art_delete_unused(&pool).await;
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

async fn find_track_file_issues(pool: &SqlitePool, library_path: PathBuf) {
    log::info!("Track: Scanning for file issues...");
    batch::find_track_file_issues(pool, library_path)
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

async fn playlist_delete_empty(pool: &SqlitePool) {
    log::info!("Playlist: Deleting empty...");
    loop {
        match Playlist::delete_all_empty_without_children(pool).await {
            Ok(playlist_count) => {
                if playlist_count > 0 {
                    log::info!("Playlist: Deleted {playlist_count} empty");
                } else {
                    break;
                }
            }
            Err(err) => {
                log::warn!("Playlist: Failed to delete empty: {err}");
            }
        }
    }
}

fn import_track_file_paths_from_m3u_file(
    file_path: Option<&Path>,
    entry_base_path: Option<&Path>,
) -> anyhow::Result<Vec<FilePath<'static>>> {
    if let Some(file_path) = file_path {
        let mut reader = m3u::Reader::open(file_path).context("open M3U file")?;
        import_m3u_entries(reader.entries(), entry_base_path)
    } else {
        let mut reader = m3u::Reader::new(io::stdin().lock());
        import_m3u_entries(reader.entries(), entry_base_path)
    }
}

fn import_m3u_entries<T>(
    entries: m3u::Entries<'_, T>,
    entry_base_path: Option<&Path>,
) -> anyhow::Result<Vec<FilePath<'static>>>
where
    T: io::BufRead,
{
    entries
        .map(|entry_result| {
            entry_result
                .map_err(Into::into)
                .and_then(|entry| import_m3u_entry(&entry, entry_base_path))
        })
        .collect::<anyhow::Result<Vec<_>>>()
}

fn import_m3u_entry(
    entry: &m3u::Entry,
    entry_base_path: Option<&Path>,
) -> anyhow::Result<FilePath<'static>> {
    let mut file_path = m3u_entry_to_file_path(entry).context("M3U entry file path")?;
    if file_path.is_relative() {
        let Some(entry_base_path) = entry_base_path else {
            bail!(
                "unresolved relative file path \"{file_path}\"",
                file_path = file_path.display()
            );
        };
        file_path = Cow::Owned(entry_base_path.join(file_path));
    }
    Ok(FilePath::import_path(&file_path))
}

fn m3u_entry_to_file_path(entry: &m3u::Entry) -> anyhow::Result<Cow<'_, Path>> {
    match entry {
        m3u::Entry::Path(file_path) => Ok(Cow::Borrowed(file_path)),
        m3u::Entry::Url(url) => match url.to_file_path() {
            Ok(file_path) => Ok(Cow::Owned(file_path)),
            Err(()) => {
                bail!("URL \"{url}\" is not a (local) file path");
            }
        },
    }
}

async fn import_playlist_from_m3u_file(
    pool: &SqlitePool,
    local_db_uuid: DbUuid,
    library_path: &LibraryPath,
    playlist_path: &str,
    mode: ImportPlaylistMode,
    m3u_file_path: Option<&Path>,
    m3u_base_path: Option<&Path>,
) -> anyhow::Result<()> {
    let m3u_base_path = m3u_base_path.or_else(|| m3u_file_path.and_then(Path::parent));
    if let Some(m3u_base_path) = m3u_base_path {
        log::info!(
            "M3U base path: {m3u_base_path}",
            m3u_base_path = m3u_base_path.display()
        );
    }

    let track_file_paths = import_track_file_paths_from_m3u_file(m3u_file_path, m3u_base_path)
        .context("import track file paths")?;
    log::info!(
        "Imported {count} track file path(s) from M3U playlist",
        count = track_file_paths.len()
    );

    import_playlist_from_track_file_paths(
        pool,
        local_db_uuid,
        library_path,
        playlist_path,
        mode,
        track_file_paths,
    )
    .await
}

async fn import_playlist_from_track_file_paths(
    pool: &SqlitePool,
    local_db_uuid: DbUuid,
    library_path: &LibraryPath,
    playlist_path: &str,
    mode: ImportPlaylistMode,
    track_file_paths: impl IntoIterator<Item = FilePath<'_>>,
) -> anyhow::Result<()> {
    let track_refs = resolve_playlist_track_refs_from_file_paths(
        pool,
        local_db_uuid,
        library_path,
        track_file_paths,
    )
    .await
    .context("resolve track refs from file paths")?;

    let Some(playlist_id) = Playlist::find_id_by_path(pool, playlist_path)
        .await
        .context("find playlist by path")?
    else {
        // TODO: Create new playlist.
        bail!("playlist \"{playlist_path}\" not found");
    };

    // Modify playlist within a transaction.
    let tx = pool.begin().await?;
    let ignored_track_refs = match mode {
        ImportPlaylistMode::Append => {
            log::info!(
                "Appending {track_count} track(s) to playlist \"{playlist_path}\"",
                track_count = track_refs.len()
            );
            Playlist::append_tracks(|| pool, playlist_id, track_refs)
                .await
                .context("append tracks to playlist")?
        }
        ImportPlaylistMode::Replace => {
            log::info!(
                "Replacing playlist \"{playlist_path}\" with {track_count} track(s)",
                track_count = track_refs.len()
            );
            Playlist::replace_tracks(|| pool, playlist_id, track_refs)
                .await
                .context("replace tracks of playlist")?
        }
    };
    if !ignored_track_refs.is_empty() {
        log::warn!(
            "Ignored {ignored_count} duplicate track(s) in playlist \"{playlist_path}\"",
            ignored_count = ignored_track_refs.len()
        );
    }
    tx.commit().await.map_err(Into::into)
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
