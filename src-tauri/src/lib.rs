pub mod ai_client;
pub mod api;
pub mod config;
pub mod db;
pub mod serve;
pub mod util;

use ai_client::ClientHub;
use anyhow::anyhow;
use config::{
    impl_read_config_from_store,
    shortcut::{
        lookup_lexical_entry::LookupLexicalEntryShortcutHandle, ocr::OcrShortcutHandle,
        translate_text::TranslateTextShortcutHandle, ShortcutHandle,
    },
    OcrModel, TargetLangOfLexicalEntryLookup, TargetLangOfTranslation, TextToSpeechModel,
    TextToTextModel,
};
use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent};
use std::collections::HashMap;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager, RunEvent,
};
use util::{
    selected_context::{SelectedImage, SelectedText},
    window::{PendingCancelSignals, PendingInputs},
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if try_handle_shortcut::<LookupLexicalEntryShortcutHandle>(
                        app, &event, shortcut,
                    ) {
                        return;
                    }

                    if try_handle_shortcut::<TranslateTextShortcutHandle>(
                        app, &event, shortcut,
                    ) {
                        return;
                    }

                    try_handle_shortcut::<OcrShortcutHandle>(app, &event, shortcut);
                })
                .build(),
        )
        .plugin(tauri_plugin_opener::init());

    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());

    builder
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            let app_data_path = app.path().app_local_data_dir()?;
            let db_path = app_data_path.join("database.sqlite");

            let sqlite_interface =
                tauri::async_runtime::block_on(async { db::SqliteInterface::new(db_path).await })?;

            app.manage(sqlite_interface);

            let handle_of_le_lookup_shortcut = LookupLexicalEntryShortcutHandle::new(app.handle())?;
            let handle_of_text_translate_shortcut = TranslateTextShortcutHandle::new(app.handle())?;
            let handle_of_ocr_shortcut = OcrShortcutHandle::new(app.handle())?;

            app.manage(std::sync::RwLock::new(handle_of_ocr_shortcut));
            app.manage(std::sync::RwLock::new(handle_of_le_lookup_shortcut));
            app.manage(std::sync::RwLock::new(handle_of_text_translate_shortcut));

            let client_hub =
                tauri::async_runtime::block_on(async { ClientHub::new(app.handle()).await })?;

            let review_progresses = HashMap::<String, serve::review::ReviewProgress>::new();

            app.manage(tokio::sync::Mutex::new(review_progresses));
            app.manage(tokio::sync::RwLock::new(client_hub));

            app.manage(tokio::sync::Mutex::new(SelectedText {
                text: String::new(),
            }));

            app.manage(tokio::sync::Mutex::new(SelectedImage {
                bin: Vec::new(),
            }));

            app.manage(tokio::sync::Mutex::new(PendingInputs::new()));
            app.manage(tokio::sync::Mutex::new(PendingCancelSignals::new()));

            app.manage(tokio::sync::RwLock::new(TextToTextModel {
                id: impl_read_config_from_store(app.handle(), "textToTextModel")?,
            }));

            app.manage(tokio::sync::RwLock::new(TextToSpeechModel {
                id: impl_read_config_from_store(app.handle(), "textToSpeechModel")?,
            }));

            app.manage(tokio::sync::RwLock::new(OcrModel {
                id: impl_read_config_from_store(app.handle(), "ocrModel")?,
            }));

            app.manage(tokio::sync::RwLock::new(TargetLangOfLexicalEntryLookup {
                lang: impl_read_config_from_store(
                    app.handle(),
                    "targetLangOfLexicalEntryLookup",
                )?,
            }));

            app.manage(tokio::sync::RwLock::new(TargetLangOfTranslation {
                lang: impl_read_config_from_store(app.handle(), "targetLangOfTranslation")?,
            }));

            let show_i = MenuItem::with_id(app, "show", "Show LexiCog", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let _ = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        println!("quit menu item was clicked");
                        app.exit(0);
                    }
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {
                        println!("menu item {:?} not handled", event.id);
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            config::shortcut::reset_hotkey,
            config::reset_ttt_model,
            config::reset_tts_model,
            config::reset_ocr_model,
            config::reset_target_lang_of_lexical_entry_lookup,
            config::reset_target_lang_of_translation,
            config::read_config_from_store,
            config::shortcut::translate_text::mimic_trigger_translate_text,
            config::shortcut::lookup_lexical_entry::mimic_trigger_lookup_lexical_entry,
            serve::text_to_speech::serve_text_to_speech,
            serve::lookup_lexical_entry::lookup_lexical_entry,
            serve::lookup_lexical_entry::mark_lexical_entry,
            serve::lookup_lexical_entry::get_lookup_history,
            serve::lookup_lexical_entry::get_unique_source_languages_of_lexical_entries,
            serve::lookup_lexical_entry::remove_lexical_entry,
            serve::lookup_lexical_entry::domain::serve_representative_entries_by_discipline,
            serve::lookup_lexical_entry::domain::get_unique_disciplines_of_lexical_entries,
            serve::translate_text::serve_text_translation,
            serve::ocr::serve_ocr,
            serve::ocr::fetch_selected_image,
            serve::review::serve_session,
            serve::review::update_review_state,
            serve::review::get_review_history,
            serve::review::remove_review_session,
            util::window::deliver_single_message_from_window_to_backend,
            util::window::deliver_cancel_signal_from_window_to_backend,
            util::window::hide_window,
            api::add_vendor_api,
            api::remove_vendor,
            api::set_vendor_api,
            api::get_vendor_api
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app, event| match event {
            RunEvent::WindowEvent {
                label,
                event: tauri::WindowEvent::CloseRequested { api, .. },
                ..
            } => {
                if label == "main" {
                    api.prevent_close();

                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.hide();
                    }
                }
            }
            _ => {}
        });
}

fn try_handle_shortcut<T: ShortcutHandle + Sync + 'static>(
    app: &AppHandle,
    event: &GlobalHotKeyEvent,
    shortcut: &HotKey,
) -> bool {
    if let Some(state) = app.try_state::<std::sync::RwLock<T>>() {
        let handler = match state.read() {
            Ok(h) => h,
            Err(e) => {
                log::error!(
                    "{:#}",
                    anyhow!(e.to_string())
                        .context("dispatch global shortcut: read shortcut handler lock")
                );
                return false;
            }
        };

        if shortcut == handler.get_hotkey() {
            handler.callback(app, *event);
            true
        } else {
            false
        }
    } else {
        log::error!(
            "{:#}",
            anyhow!("missing tauri state: shortcut handle")
                .context("dispatch global shortcut: load shortcut handler state")
        );
        false
    }
}