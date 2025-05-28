// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use std::borrow::Cow;

use anyhow::Result;
use futures_util::StreamExt as _;
use sqlx::SqlitePool;

use endjine::{
    BatchOutcome, album_art_delete_orphaned, optimize_database, shrink_album_art,
    smartlist_fetch_all,
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
    let pool = SqlitePool::connect(&database_url).await?;

    log::info!("Opened database: {database_path}");

    log::info!("Scanning Smartlists...");
    // Try to load all Smartlists from the database to verify the schema definition.
    let (smartlist_ok_count, smartlist_err_count) = smartlist_fetch_all(&pool)
        .fold((0, 0), |(ok_count, err_count), result| {
            let counts = match result {
                Ok(_) => (ok_count + 1, err_count),
                Err(err) => {
                    log::warn!("Failed to load smartlist: {err}");
                    (ok_count, err_count + 1)
                }
            };
            std::future::ready(counts)
        })
        .await;
    let smartlist_count = smartlist_ok_count + smartlist_err_count;
    if smartlist_err_count > 0 {
        log::warn!("Found {smartlist_count} Smartlist(s): {smartlist_err_count} unreadable");
    } else {
        log::info!("Found {smartlist_count} Smartlist(s)");
    }

    log::info!("Deleting orphaned album art...");
    match album_art_delete_orphaned(&pool).await {
        Ok(rows_affected) => {
            if rows_affected > 0 {
                log::info!("Deleted {rows_affected} row(s) of orphaned album art");
            } else {
                log::info!("No orphaned album art found");
            }
        }
        Err(err) => {
            log::warn!("Failed to delete orphaned album art: {err}");
        }
    }

    log::info!("Shrinking album art...");
    {
        let BatchOutcome {
            succeeded,
            skipped,
            failed,
            aborted_error,
        } = shrink_album_art(&pool).await;
        log::info!(
            "Shrinking of album art finished: succeeded = {succeeded}, skipped = {skipped}, failed = {failed}",
            failed = failed.len()
        );
        if let Some(err) = aborted_error {
            log::warn!("Shrinking of album art aborted with error: {err}");
        }
    }

    log::info!("Optimizing database...");
    if let Err(err) = optimize_database(&pool).await {
        log::warn!("Failed to optimize database: {err}");
    }

    log::info!("Finished housekeeping");

    Ok(())
}
