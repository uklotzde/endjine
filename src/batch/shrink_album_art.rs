// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use std::io::Cursor;

use futures_util::StreamExt;
use image::{DynamicImage, ImageFormat, ImageReader, codecs::jpeg::JpegEncoder};
use sqlx::SqlitePool;
use tokio::task::block_in_place;

use crate::{AlbumArt, BatchOutcome};

const BATCH_UPDATE_SIZE: u16 = 128;

const JPEG_QUALITY: u8 = 70;

const MAX_RATIO: f64 = 0.75;

#[expect(clippy::too_many_lines, reason = "TODO")]
pub async fn shrink_album_art(pool: &SqlitePool) -> BatchOutcome {
    let mut outcome = BatchOutcome::default();
    let mut last_id = -1;
    let mut batch_update: Vec<(i64, ImageFormat, f64, Vec<u8>)> =
        Vec::with_capacity(BATCH_UPDATE_SIZE.into());
    loop {
        if !batch_update.is_empty() {
            log::debug!(
                "Updating {batch_size} album art image(s)",
                batch_size = batch_update.len()
            );
            for (id, format, ratio, album_art) in &batch_update {
                match sqlx::query(r"UPDATE AlbumArt SET albumArt=?2 WHERE id=?1")
                    .bind(id)
                    .bind(album_art)
                    .execute(pool)
                    .await
                {
                    Ok(result) => {
                        debug_assert_eq!(result.rows_affected(), 1);
                    }
                    Err(err) => {
                        log::warn!("Failed to update album art {id}: {err}");
                        outcome.failed.push(Box::new(err));
                        continue;
                    }
                }
                log::info!(
                    "Converted album art {id} from {format} to JPEG: {percent:.1}%",
                    format = format!("{format:?}").to_uppercase(),
                    percent = ratio * 100.0,
                );
                outcome.succeeded += 1;
            }
            batch_update.clear();
        }
        let mut rows = sqlx::query_as(r"SELECT * FROM AlbumArt WHERE id>?1 ORDER BY id LIMIT ?2")
            .bind(last_id)
            .bind(i64::from(BATCH_UPDATE_SIZE))
            .fetch(pool);
        let mut row_fetch_count = 0;
        while let Some(row) = rows.next().await {
            row_fetch_count += 1;
            let (id, format, image, old_size) = match row {
                Ok(row) => {
                    let AlbumArt {
                        id,
                        hash,
                        album_art,
                    } = &row;
                    let id = *id;
                    debug_assert!(id > last_id);
                    last_id = id;
                    let Some(album_art) = album_art else {
                        log::debug!("Skipping missing album art {id}");
                        debug_assert!(hash.is_none());
                        outcome.skipped += 1;
                        continue;
                    };
                    debug_assert!(hash.is_some());
                    let (format, image) = match block_in_place(|| decode_image(album_art)) {
                        Ok(ok) => ok,
                        Err(err) => {
                            log::warn!("Failed to decode image data of album art {id}: {err}");
                            outcome.failed.push(Box::new(err));
                            continue;
                        }
                    };
                    match format {
                        None => {
                            log::info!("Skipping album art {id} with unknown image format");
                            outcome.skipped += 1;
                            continue;
                        }
                        Some(format @ (ImageFormat::Png | ImageFormat::Bmp | ImageFormat::Tga)) => {
                            (id, format, image, album_art.len())
                        }
                        Some(ImageFormat::Jpeg) => {
                            log::debug!("Skipping album art {id} with JPEG image format");
                            outcome.skipped += 1;
                            continue;
                        }
                        Some(unsupported_format) => {
                            log::info!(
                                "Skipping album art {id} with unsupported image format {unsupported_format:?}"
                            );
                            outcome.skipped += 1;
                            continue;
                        }
                    }
                }
                Err(fetch_error) => {
                    log::warn!("Failed to fetch row: {fetch_error}");
                    return outcome.abort(Box::new(fetch_error));
                }
            };
            // We replace the image data but leave the original hash as is. This ensures
            // that Engine DJ will reuse album art when adding tracks with the same
            // image.
            let mut album_art_jpeg = Vec::with_capacity(256_000);
            let encoder = JpegEncoder::new_with_quality(&mut album_art_jpeg, JPEG_QUALITY);
            if let Err(err) = block_in_place(|| image.write_with_encoder(encoder)) {
                log::warn!("Failed to re-encode album art {id} as JPEG: {err}");
                outcome.failed.push(Box::new(err));
                continue;
            }
            let new_size = album_art_jpeg.len();
            if new_size < old_size && new_size > 0 {
                #[expect(clippy::cast_precision_loss)]
                let ratio = new_size as f64 / old_size as f64;
                if ratio <= MAX_RATIO {
                    debug_assert!(batch_update.len() < BATCH_UPDATE_SIZE.into());
                    batch_update.push((id, format, ratio, album_art_jpeg));
                    continue;
                }
            }
            log::info!("Keeping album art {id}: old size = {old_size}, new size = {new_size}");
            outcome.skipped += 1;
        }
        if row_fetch_count > 0 {
            continue;
        }
        return outcome;
    }
}

fn decode_image(
    bytes: impl AsRef<[u8]>,
) -> image::ImageResult<(Option<ImageFormat>, DynamicImage)> {
    let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let image_format = reader.format();
    reader.decode().map(|image| (image_format, image))
}
