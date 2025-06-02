// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use std::{future, io, path::PathBuf};

use futures_util::{StreamExt as _, stream::BoxStream};
use sqlx::SqliteExecutor;
use tokio::task::block_in_place;

use crate::TrackId;

#[derive(Debug)]
pub enum TrackFileIssue {
    FileError(io::Error),
    FileMissing,
}

#[derive(Debug)]
pub struct TrackFileIssueItem {
    pub db_id: TrackId,
    pub db_path: String,

    /// The absolute file path.
    pub file_path: PathBuf,

    /// The
    pub file_issue: TrackFileIssue,
}

/// Finds track file issues.
///
/// Track file paths in the database are relative to the path of the
/// database file. The `base_path` refers to the "Engine Library"
/// directory.
#[must_use]
pub fn find_track_file_issues<'a>(
    executor: impl SqliteExecutor<'a> + 'a,
    base_path: PathBuf,
) -> BoxStream<'a, sqlx::Result<TrackFileIssueItem>> {
    sqlx::query_as::<_, (TrackId, String)>(
        r#"SELECT "id","path" FROM "Track" WHERE "path" IS NOT NULL"#,
    )
    .fetch(executor)
    .filter_map(move |next_result| {
        let (db_id, db_path) = match next_result {
            Ok(ok) => ok,
            Err(err) => {
                // Pass all errors through.
                return future::ready(Some(Err(err)));
            }
        };
        log::debug!("Checking path \"{db_path}\" of track {db_id}");
        let mut file_path = base_path.join(&db_path);
        let file_issue = block_in_place(||
                // Blocking file I/O operations.
                match check_file_exists(&mut file_path) {
                    Ok(true) => None,
                    Ok(false) => Some(TrackFileIssue::FileMissing),
                    Err(err) => Some(TrackFileIssue::FileError(err)),
                });
        future::ready(file_issue.map(|file_issue| {
            Ok(TrackFileIssueItem {
                db_id,
                db_path,
                file_path,
                file_issue,
            })
        }))
    })
    .boxed()
}

fn check_file_exists(file_path: &mut PathBuf) -> io::Result<bool> {
    if let (Some(parent_path), Some(file_name)) = (file_path.parent(), file_path.file_name()) {
        let parent_path = parent_path.canonicalize()?;
        *file_path = parent_path.join(file_name);
    }
    file_path.try_exists()
}
