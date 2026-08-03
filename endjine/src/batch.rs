// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use std::error::Error;

mod find_track_file_issues;
pub use self::find_track_file_issues::{
    TrackFileIssue, TrackFileIssueItem, find_track_file_issues,
};

mod purge_album_art;
pub use self::purge_album_art::purge_album_art;

#[derive(Debug, Default)]
pub struct BatchOutcome {
    /// Number of items that succeeded.
    pub succeeded: u64,

    /// Number of items that were skipped.
    pub skipped: u64,

    /// Failed items.
    pub failed: Vec<Box<dyn Error>>,

    /// Error that aborted the batch operation prematurely.
    ///
    /// If `None` the batch operation finished regularly.
    pub aborted_error: Option<Box<dyn Error>>,
}
