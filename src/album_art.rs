// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use std::io::Cursor;

use futures_util::stream::BoxStream;
use image::{DynamicImage, ImageFormat, ImageReader, ImageResult};
use sqlx::{FromRow, SqliteExecutor, sqlite::SqliteQueryResult};

crate::db_id!(AlbumArtId);

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct AlbumArt {
    id: AlbumArtId,
    hash: Option<String>,
    #[sqlx(rename = "albumArt")]
    image_data: Option<Vec<u8>>,
}

impl AlbumArt {
    #[must_use]
    pub const fn id(&self) -> AlbumArtId {
        self.id
    }

    #[must_use]
    pub const fn hash(&self) -> Option<&str> {
        if let Some(hash) = &self.hash {
            return Some(hash.as_str());
        }
        None
    }

    #[must_use]
    pub const fn image_data(&self) -> Option<&[u8]> {
        if let Some(image_data) = &self.image_data {
            return Some(image_data.as_slice());
        }
        None
    }

    pub fn guess_image_format(&self) -> ImageResult<Option<ImageFormat>> {
        let Some(image_data) = self.image_data() else {
            return Ok(None);
        };
        guess_image_format(image_data)
    }

    pub fn decode_image(&self) -> ImageResult<(Option<ImageFormat>, Option<DynamicImage>)> {
        let Some(image_data) = self.image_data() else {
            return Ok((None, None));
        };
        let (image_format, image) = decode_image(image_data)?;
        Ok((image_format, Some(image)))
    }

    /// Fetches all album art asynchronously.
    ///
    /// Unfiltered and in no particular order.
    #[must_use]
    pub fn fetch_all<'a>(
        executor: impl SqliteExecutor<'a> + 'a,
    ) -> BoxStream<'a, sqlx::Result<AlbumArt>> {
        sqlx::query_as(r"SELECT * FROM AlbumArt").fetch(executor)
    }

    /// Loads a single album art by id.
    ///
    /// Returns `Ok(None)` if the requested album art has not been found.
    pub async fn try_load(
        executor: impl SqliteExecutor<'_>,
        id: AlbumArtId,
    ) -> sqlx::Result<Option<AlbumArt>> {
        sqlx::query_as(r"SELECT * FROM AlbumArt WHERE id=?1")
            .bind(id)
            .fetch_optional(executor)
            .await
    }

    pub async fn update_image(
        executor: impl SqliteExecutor<'_>,
        id: AlbumArtId,
        image_data: impl AsRef<[u8]>,
    ) -> sqlx::Result<SqliteQueryResult> {
        sqlx::query(r"UPDATE AlbumArt SET albumArt=?2 WHERE id=?1")
            .bind(id)
            .bind(image_data.as_ref())
            .execute(executor)
            .await
    }

    pub async fn delete_unused(executor: impl SqliteExecutor<'_>) -> sqlx::Result<u64> {
        let result =
            sqlx::query(r"DELETE FROM AlbumArt WHERE id NOT IN (SELECT albumArtId FROM Track WHERE albumArtId IS NOT NULL)")
                .execute(executor)
                .await?;
        Ok(result.rows_affected())
    }
}

fn guess_image_format(image_data: &[u8]) -> ImageResult<Option<ImageFormat>> {
    let reader = ImageReader::new(Cursor::new(image_data)).with_guessed_format()?;
    Ok(reader.format())
}

fn decode_image(image_data: &[u8]) -> ImageResult<(Option<ImageFormat>, DynamicImage)> {
    let reader = ImageReader::new(Cursor::new(image_data)).with_guessed_format()?;
    let image_format = reader.format();
    reader.decode().map(|image| (image_format, image))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AlbumArtImageQuality {
    Low,
    Medium,
    High,
}
