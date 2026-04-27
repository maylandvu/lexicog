#[cfg(target_os = "macos")]
mod macos_builtin_ocr;

mod prompt_template;

use crate::{
    ai_client::ClientHub,
    config::OcrModel,
    util::{selected_context::SelectedImage, window::PendingCancelSignals},
};
use anyhow::{anyhow, Context, Error, Result};
use base64::engine::general_purpose::STANDARD;
use base64::{engine::general_purpose, Engine as _};
use tauri::{ipc::Channel, AppHandle, Emitter};
use tokio::sync::{Mutex, RwLock};

use tokio::sync::oneshot;

use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[tauri::command]
pub async fn fetch_selected_image(
    selected_image_state: tauri::State<'_, Mutex<SelectedImage>>,
) -> Result<String, String> {
    Ok(STANDARD.encode(selected_image_state.lock().await.bin.clone()))
}

#[tauri::command]
pub async fn serve_ocr(
    app: AppHandle,
    pending_cancel_signals_state: tauri::State<'_, Mutex<PendingCancelSignals>>,
    client_hub_state: tauri::State<'_, RwLock<ClientHub>>,
    ocr_model_state: tauri::State<'_, RwLock<OcrModel>>,
    selected_image_state: tauri::State<'_, Mutex<SelectedImage>>,
    channel: Channel<Option<String>>,
    languages: Vec<String>,
    offset_x: u32,
    offset_y: u32,
    width: u32,
    height: u32,
) -> Result<(), String> {
    let original_image: Vec<u8> = selected_image_state.lock().await.bin.clone();

    let cropped_image = crop_and_convert(original_image, offset_x, offset_y, width, height)
        .map_err(|e| {
            format!(
                "{:#}",
                e.context("serve OCR: crop screenshot to selected region")
            )
        })?;

    /*
     * macOS support:
     * Use Apple's built-in OCR first.
     *
     * Important:
     * Do NOT use `if cfg!(target_os = "macos")`.
     * That still compiles the macOS code on Windows.
     *
     * Use `#[cfg(target_os = "macos")]` so Windows completely ignores
     * this block at compile time.
     */
    #[cfg(target_os = "macos")]
    {
        let cropped_image_copy = cropped_image.clone();
        let (tx, rx) = oneshot::channel::<Result<String>>();
        let languages_for_macos = languages.clone();

        app.run_on_main_thread(move || {
            let refs: Vec<&str> = languages_for_macos.iter().map(|s| s.as_str()).collect();

            if tx
                .send(macos_builtin_ocr::recognize(
                    &cropped_image_copy,
                    Some(&refs),
                ))
                .is_err()
            {
                log::error!(
                    "{:#}",
                    anyhow!(
                        "failed to send macOS built-in OCR result from main thread to backend thread"
                    )
                    .context("serve OCR")
                );
            }
        })
        .map_err(|e| {
            format!(
                "{:#}",
                Error::from(e).context("serve OCR: run built-in macOS OCR call on main thread")
            )
        })?;

        match rx.await {
            Ok(Ok(text)) => {
                if text.trim().is_empty() {
                    log::warn!(
                        "macOS built-in OCR returned empty text, fallback to configured OCR model"
                    );
                } else {
                    channel.send(Some(text.clone())).map_err(|e| {
                        format!(
                            "{:#}",
                            Error::from(e).context("serve OCR: send built-in OCR text to frontend")
                        )
                    })?;

                    channel.send(None).map_err(|e| {
                        format!(
                            "{:#}",
                            Error::from(e)
                                .context("serve OCR: send completion signal for built-in OCR")
                        )
                    })?;

                    return Ok(());
                }
            }

            Ok(Err(e)) => {
                log::error!("{:#}", e.context("serve OCR: run built-in macOS OCR"));
            }

            Err(e) => {
                log::error!(
                    "{:#}",
                    Error::from(e).context("serve OCR: receive built-in macOS OCR result")
                );
            }
        }
    }


    let model_id = ocr_model_state.read().await.id.clone().ok_or(format!(
        "{:#}",
        anyhow!("no OCR model is currently configured").context("serve OCR: resolve active model")
    ))?;

    let prompt = prompt_template::OCR_PROMPT_TEMPLATE.to_string();

    let task_id = Uuid::new_v4().to_string();
    let cancel_token = CancellationToken::new();

    {
        let mut pending_cancel_signals_guard = pending_cancel_signals_state.lock().await;
        pending_cancel_signals_guard.insert(task_id.clone(), cancel_token.clone());
    }

    app.emit("ocr-task-started", task_id.clone()).map_err(|e| {
        format!(
            "{:#}",
            Error::from(e).context("serve OCR: emit ocr-task-started event")
        )
    })?;

    let result: Result<()> = tokio::select! {
        _ = cancel_token.cancelled() => {
            Err(anyhow!("ocr generation cancelled by user").context("serve OCR"))
        }

        res = async move {
            let client_hub_guard = client_hub_state.read().await;

            if let Some(client) = &client_hub_guard.ocr_client {
                let _ = client
                    .execute_ocr_task(
                        channel,
                        prompt,
                        general_purpose::STANDARD.encode(&cropped_image),
                        model_id,
                    )
                    .await
                    .context("serve OCR: execute model-based OCR streaming task")?;

                Ok(())
            } else {
                Err(anyhow!("no OCR client is available in ClientHub").context("serve OCR"))
            }
        } => res
    };

    let mut pending_cancel_signals_guard = pending_cancel_signals_state.lock().await;
    pending_cancel_signals_guard.remove(&task_id);

    result.map_err(|e| format!("{:#}", e))
}

fn crop_and_convert(
    bin_img: Vec<u8>,
    offset_x: u32,
    offset_y: u32,
    width: u32,
    height: u32,
) -> Result<Vec<u8>> {
    let img = image::load_from_memory(&bin_img).context("crop OCR image: decode source image")?;

    let img_width = img.width();
    let img_height = img.height();

    if width == 0 || height == 0 {
        return Err(
            anyhow!("crop region is empty: width or height is zero").context("crop OCR image")
        );
    }

    if offset_x >= img_width || offset_y >= img_height {
        return Err(anyhow!(
            "crop origin out of bounds: ({}, {}) for image {}x{}",
            offset_x,
            offset_y,
            img_width,
            img_height
        )
        .context("crop OCR image"));
    }

    let crop_width = width.min(img_width.saturating_sub(offset_x));
    let crop_height = height.min(img_height.saturating_sub(offset_y));

    let cropped_img = img.crop_imm(offset_x, offset_y, crop_width, crop_height);

    let mut png_buf = std::io::Cursor::new(Vec::new());

    cropped_img
        .write_to(&mut png_buf, image::ImageFormat::Png)
        .context("crop OCR image: encode cropped region as PNG")?;

    Ok(png_buf.into_inner())
}