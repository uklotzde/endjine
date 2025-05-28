// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use futures_util::StreamExt;
use image::{ImageFormat, codecs::jpeg::JpegEncoder};
use sqlx::SqlitePool;
use tokio::task::block_in_place;

use crate::{AlbumArt, AlbumArtId, BatchOutcome};

const BATCH_UPDATE_SIZE: u16 = 128;

const JPEG_QUALITY: u8 = 70;

const MAX_RATIO: f64 = 0.75;

#[derive(Debug)]
struct BatchUpdateItem {
    id: AlbumArtId,
    format: ImageFormat,
    ratio: f64,
    image_data: Vec<u8>,
}

#[expect(clippy::too_many_lines, reason = "TODO")]
pub async fn shrink_album_art(pool: &SqlitePool) -> BatchOutcome {
    let mut outcome = BatchOutcome::default();
    // All ids in the database are strictly positive.
    let mut last_id = AlbumArtId::INVALID_MIN_EXCLUSIVE;
    let mut batch_update_items: Vec<BatchUpdateItem> = Vec::with_capacity(BATCH_UPDATE_SIZE.into());
    loop {
        if !batch_update_items.is_empty() {
            log::debug!(
                "Updating {batch_size} album art image(s)",
                batch_size = batch_update_items.len()
            );
            for BatchUpdateItem {
                id,
                format,
                ratio,
                image_data,
            } in batch_update_items.drain(..)
            {
                match AlbumArt::update_image(pool, id, image_data).await {
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
            debug_assert!(batch_update_items.is_empty());
        }
        let mut rows = sqlx::query_as(r"SELECT * FROM AlbumArt WHERE id>?1 ORDER BY id")
            .bind(last_id)
            .fetch(pool);
        let mut row_fetch_count = 0;
        while let Some(row) = rows.next().await {
            row_fetch_count += 1;
            let (id, format, image, old_size) = match row {
                Ok(row) => {
                    let album_art: AlbumArt = row;
                    let id = album_art.id();
                    debug_assert!(id > last_id);
                    last_id = id;
                    match block_in_place(|| album_art.decode_image()) {
                        Ok((_, None)) => {
                            log::debug!("Skipping missing album art {id}");
                            debug_assert!(album_art.hash().is_none());
                            outcome.skipped += 1;
                            continue;
                        }
                        Ok((None, _)) => {
                            log::info!("Skipping album art {id} with unknown image format");
                            debug_assert!(album_art.hash().is_some());
                            outcome.skipped += 1;
                            continue;
                        }
                        Ok((Some(format), Some(image))) => {
                            debug_assert!(album_art.hash().is_some());
                            match format {
                                format @ (ImageFormat::Png
                                | ImageFormat::Bmp
                                | ImageFormat::Tga) => (
                                    id,
                                    format,
                                    image,
                                    album_art.image_data().map_or(0, <[u8]>::len),
                                ),
                                ImageFormat::Jpeg => {
                                    log::debug!("Skipping album art {id} with JPEG image format");
                                    outcome.skipped += 1;
                                    continue;
                                }
                                unsupported_format => {
                                    log::info!(
                                        "Skipping album art {id} with unsupported image format {unsupported_format:?}"
                                    );
                                    outcome.skipped += 1;
                                    continue;
                                }
                            }
                        }
                        Err(err) => {
                            log::warn!("Failed to decode image data of album art {id}: {err}");
                            outcome.failed.push(Box::new(err));
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
            let mut image_data_jpeg = Vec::with_capacity(256_000);
            let encoder = JpegEncoder::new_with_quality(&mut image_data_jpeg, JPEG_QUALITY);
            if let Err(err) = block_in_place(|| image.write_with_encoder(encoder)) {
                log::warn!("Failed to re-encode album art {id} as JPEG: {err}");
                outcome.failed.push(Box::new(err));
                continue;
            }
            let new_size = image_data_jpeg.len();
            if new_size < old_size && new_size > 0 {
                #[expect(clippy::cast_precision_loss)]
                let ratio = new_size as f64 / old_size as f64;
                if ratio <= MAX_RATIO {
                    debug_assert!(batch_update_items.len() < BATCH_UPDATE_SIZE.into());
                    batch_update_items.push(BatchUpdateItem {
                        id,
                        format,
                        ratio,
                        image_data: image_data_jpeg,
                    });
                    if batch_update_items.len() >= BATCH_UPDATE_SIZE.into() {
                        // Abort scanning and update the album art collected during the current batch.
                        break;
                    }
                    continue;
                }
            }
            log::info!("Keeping album art {id}: old size = {old_size}, new size = {new_size}");
            outcome.skipped += 1;
        }
        if row_fetch_count > 0 {
            continue;
        }
        debug_assert!(batch_update_items.is_empty());
        return outcome;
    }
}
