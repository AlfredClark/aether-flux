use tauri::{Manager, Runtime};
use tauri_plugin_i18n::PluginI18nExt;

pub fn tr<R: Runtime, T: Manager<R>>(app: &T, key: &str, fallback: &str) -> String {
    app.i18n()
        .translate(key)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| fallback.to_string())
}

pub fn tr_args<R: Runtime, T: Manager<R>>(
    app: &T,
    key: &str,
    fallback: &str,
    args: &[(&str, String)],
) -> String {
    let mut translated = tr(app, key, fallback);
    for (name, value) in args {
        translated = translated.replace(&format!("{{{name}}}"), value);
    }
    translated
}

pub fn localize_error<R: Runtime, T: Manager<R>>(app: &T, message: &str) -> String {
    if message.trim().is_empty() {
        return String::new();
    }

    message
        .split('\n')
        .map(|line| {
            line.split(": ")
                .map(|segment| localize_segment(app, segment))
                .collect::<Vec<_>>()
                .join(": ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn localize_segment<R: Runtime, T: Manager<R>>(app: &T, segment: &str) -> String {
    match segment {
        "Failed to lock settings state" => tr(
            app,
            "backend.settings.lock_failed",
            "Failed to lock settings state",
        ),
        "Settings service was not initialized" => tr(
            app,
            "backend.settings.not_initialized",
            "Settings service was not initialized",
        ),
        "Failed to lock recorder state" => tr(
            app,
            "backend.recorder.lock_failed",
            "Failed to lock recorder state",
        ),
        "A recording session is already active" => tr(
            app,
            "backend.recorder.session_active",
            "A recording session is already active",
        ),
        "There is no active recording" => tr(
            app,
            "backend.recorder.no_active_recording",
            "There is no active recording",
        ),
        "Failed to lock WAV writer" => tr(
            app,
            "backend.recorder.wav_writer_lock_failed",
            "Failed to lock WAV writer",
        ),
        "Failed to lock ASR state" => {
            tr(app, "backend.asr.lock_failed", "Failed to lock ASR state")
        }
        "ASR model is not initialized" => tr(
            app,
            "backend.asr.model_not_initialized",
            "ASR model is not initialized",
        ),
        "ASR language is not initialized" => tr(
            app,
            "backend.asr.language_not_initialized",
            "ASR language is not initialized",
        ),
        "ASR model is not loaded" => tr(
            app,
            "backend.asr.model_not_loaded",
            "ASR model is not loaded",
        ),
        "ASR fitter is not loaded" => tr(
            app,
            "backend.asr.fitter_not_loaded",
            "ASR fitter is not loaded",
        ),
        "ASR decomposer is not loaded" => tr(
            app,
            "backend.asr.decomposer_not_loaded",
            "ASR decomposer is not loaded",
        ),
        "app_cache_dir was not available" => tr(
            app,
            "backend.path.app_cache_dir_unavailable",
            "app_cache_dir was not available",
        ),
        "app_local_data_dir was not available" => tr(
            app,
            "backend.path.app_local_data_dir_unavailable",
            "app_local_data_dir was not available",
        ),
        "Failed to lock wordbank state" => tr(
            app,
            "backend.wordbank.lock_failed",
            "Failed to lock wordbank state",
        ),
        "wordbank loader was not initialized" => tr(
            app,
            "backend.wordbank.loader_not_initialized",
            "wordbank loader was not initialized",
        ),
        "wordbank id cannot be empty" => tr(
            app,
            "backend.wordbank.id_empty",
            "wordbank id cannot be empty",
        ),
        "wordbank name cannot be empty" => tr(
            app,
            "backend.wordbank.name_empty",
            "wordbank name cannot be empty",
        ),
        "default wordbank cannot be disabled" => tr(
            app,
            "backend.wordbank.default_cannot_disable",
            "default wordbank cannot be disabled",
        ),
        "default wordbank cannot be reordered" => tr(
            app,
            "backend.wordbank.default_cannot_reorder",
            "default wordbank cannot be reordered",
        ),
        "default wordbank cannot be deleted" => tr(
            app,
            "backend.wordbank.default_cannot_delete",
            "default wordbank cannot be deleted",
        ),
        "reordered wordbanks must contain exactly all non-default wordbanks" => tr(
            app,
            "backend.wordbank.reorder_invalid",
            "reordered wordbanks must contain exactly all non-default wordbanks",
        ),
        "reordered values must contain exactly the existing group entries" => tr(
            app,
            "backend.wordbank.group_reorder_invalid",
            "reordered values must contain exactly the existing group entries",
        ),
        "reordered values must contain at least one entry" => tr(
            app,
            "backend.wordbank.group_reorder_empty",
            "reordered values must contain at least one entry",
        ),
        "at least one wordbank must be selected for export" => tr(
            app,
            "backend.wordbank.export_none_selected",
            "at least one wordbank must be selected for export",
        ),
        "some selected wordbanks were not found for export" => tr(
            app,
            "backend.wordbank.export_missing_selected",
            "some selected wordbanks were not found for export",
        ),
        "word bank input did not contain any Chinese entries" => tr(
            app,
            "backend.wordbank.input_no_chinese_entries",
            "word bank input did not contain any Chinese entries",
        ),
        "word entry did not produce a pinyin key" => tr(
            app,
            "backend.wordbank.entry_no_pinyin_key",
            "word entry did not produce a pinyin key",
        ),
        "wordbank database file was not found" => tr(
            app,
            "backend.wordbank.database_file_not_found",
            "wordbank database file was not found",
        ),
        _ => {
            if let Some(err) = segment.strip_prefix("Failed to update settings") {
                return localize_with_suffix(
                    app,
                    "backend.settings.update_failed",
                    "Failed to update settings",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("Failed to reset settings") {
                return localize_with_suffix(
                    app,
                    "backend.settings.reset_failed",
                    "Failed to reset settings",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("Failed to refresh tray locale") {
                return localize_with_suffix(
                    app,
                    "backend.tray.refresh_failed",
                    "Failed to refresh tray locale",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("Failed to enumerate input devices") {
                return localize_with_suffix(
                    app,
                    "backend.recorder.enumerate_devices_failed",
                    "Failed to enumerate input devices",
                    "err",
                    err,
                );
            }
            if let Some(device_id) = segment.strip_prefix("Input device not found") {
                return localize_with_suffix(
                    app,
                    "backend.recorder.input_device_not_found",
                    "Input device not found",
                    "device_id",
                    device_id,
                );
            }
            if let Some(err) = segment.strip_prefix("Failed to get device ID") {
                return localize_with_suffix(
                    app,
                    "backend.recorder.device_id_failed",
                    "Failed to get device ID",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("Failed to get default input config") {
                return localize_with_suffix(
                    app,
                    "backend.recorder.default_input_config_failed",
                    "Failed to get default input config",
                    "err",
                    err,
                );
            }
            if let Some(sample_format) = segment.strip_prefix("Unsupported sample format") {
                return localize_with_suffix(
                    app,
                    "backend.recorder.unsupported_sample_format",
                    "Unsupported sample format",
                    "sample_format",
                    sample_format,
                );
            }
            if let Some(err) = segment.strip_prefix("Failed to start recording stream") {
                return localize_with_suffix(
                    app,
                    "backend.recorder.start_stream_failed",
                    "Failed to start recording stream",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("Failed to finalize WAV file") {
                return localize_with_suffix(
                    app,
                    "backend.recorder.finalize_wav_failed",
                    "Failed to finalize WAV file",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("Failed to create recording directory") {
                return localize_with_suffix(
                    app,
                    "backend.recorder.create_recording_dir_failed",
                    "Failed to create recording directory",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("Failed to create WAV file") {
                return localize_with_suffix(
                    app,
                    "backend.recorder.create_wav_failed",
                    "Failed to create WAV file",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("Failed to open source WAV file") {
                return localize_with_suffix(
                    app,
                    "backend.recorder.open_source_wav_failed",
                    "Failed to open source WAV file",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("Failed to read source WAV samples") {
                return localize_with_suffix(
                    app,
                    "backend.recorder.read_source_wav_failed",
                    "Failed to read source WAV samples",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("Failed to write resampled WAV data") {
                return localize_with_suffix(
                    app,
                    "backend.recorder.write_resampled_wav_failed",
                    "Failed to write resampled WAV data",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("Failed to finalize resampled WAV file") {
                return localize_with_suffix(
                    app,
                    "backend.recorder.finalize_resampled_wav_failed",
                    "Failed to finalize resampled WAV file",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("Failed to remove original WAV file") {
                return localize_with_suffix(
                    app,
                    "backend.recorder.remove_original_wav_failed",
                    "Failed to remove original WAV file",
                    "err",
                    err,
                );
            }
            if let Some(err) =
                segment.strip_prefix("Failed to replace WAV file with resampled output")
            {
                return localize_with_suffix(
                    app,
                    "backend.recorder.replace_wav_failed",
                    "Failed to replace WAV file with resampled output",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("Failed to build input stream") {
                return localize_with_suffix(
                    app,
                    "backend.recorder.build_stream_failed",
                    "Failed to build input stream",
                    "err",
                    err,
                );
            }
            if segment == "Unknown Input Device" {
                return tr(
                    app,
                    "backend.recorder.unknown_input_device",
                    "Unknown Input Device",
                );
            }
            if let Some(err) = segment.strip_prefix("Failed to join ASR loader task") {
                return localize_with_suffix(
                    app,
                    "backend.asr.join_loader_task_failed",
                    "Failed to join ASR loader task",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("Failed to join ASR recognition task") {
                return localize_with_suffix(
                    app,
                    "backend.asr.join_recognition_task_failed",
                    "Failed to join ASR recognition task",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("failed to resolve recorder cache dir") {
                return localize_with_suffix(
                    app,
                    "backend.asr.resolve_recorder_cache_dir_failed",
                    "failed to resolve recorder cache dir",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("failed to collect cache stats") {
                return localize_with_suffix(
                    app,
                    "backend.asr.collect_cache_stats_failed",
                    "failed to collect cache stats",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("failed to clear recording cache") {
                return localize_with_suffix(
                    app,
                    "backend.asr.clear_recording_cache_failed",
                    "failed to clear recording cache",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("ASR fitting failed") {
                return localize_with_suffix(
                    app,
                    "backend.asr.fitting_failed",
                    "ASR fitting failed",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("ASR decomposition failed") {
                return localize_with_suffix(
                    app,
                    "backend.asr.decomposition_failed",
                    "ASR decomposition failed",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("failed to load Qwen3-ASR") {
                return localize_with_suffix(
                    app,
                    "backend.asr.load_qwen3_failed",
                    "failed to load Qwen3-ASR",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("failed to load SenseVoiceSmall") {
                return localize_with_suffix(
                    app,
                    "backend.asr.load_sensevoice_failed",
                    "failed to load SenseVoiceSmall",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("Qwen3-ASR recognition failed") {
                return localize_with_suffix(
                    app,
                    "backend.asr.qwen3_recognition_failed",
                    "Qwen3-ASR recognition failed",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("SenseVoiceSmall recognition failed") {
                return localize_with_suffix(
                    app,
                    "backend.asr.sensevoice_recognition_failed",
                    "SenseVoiceSmall recognition failed",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("Failed to join wordbank ") {
                return tr_args(
                    app,
                    "backend.wordbank.join_task_failed",
                    "Failed to join wordbank {task} task: {err}",
                    &split_wordbank_join_args(err),
                );
            }
            if let Some(err) = segment.strip_prefix("failed to resolve wordbank path") {
                return localize_with_suffix(
                    app,
                    "backend.wordbank.resolve_path_failed",
                    "failed to resolve wordbank path",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("failed to resolve wordbank backup dir") {
                return localize_with_suffix(
                    app,
                    "backend.wordbank.resolve_backup_dir_failed",
                    "failed to resolve wordbank backup dir",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("failed to initialize wordbank") {
                return localize_with_suffix(
                    app,
                    "backend.wordbank.initialize_failed",
                    "failed to initialize wordbank",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("wordbank operation failed") {
                return localize_with_suffix(
                    app,
                    "backend.wordbank.operation_failed",
                    "wordbank operation failed",
                    "err",
                    err,
                );
            }
            if let Some(err) = segment.strip_prefix("failed to backup wordbank") {
                return localize_with_suffix(
                    app,
                    "backend.wordbank.backup_failed",
                    "failed to backup wordbank",
                    "err",
                    err,
                );
            }
            if let Some(path) = segment.strip_prefix("failed to create backup dir ") {
                return tr_args(
                    app,
                    "backend.wordbank.create_backup_dir_failed",
                    "Failed to create backup directory {path}",
                    &[("path", path.to_string())],
                );
            }
            if let Some(rest) = segment.strip_prefix("failed to copy wordbank database from ") {
                if let Some((source, target)) = rest.split_once(" to ") {
                    return tr_args(
                        app,
                        "backend.wordbank.copy_database_failed",
                        "Failed to copy wordbank database from {source} to {target}",
                        &[
                            ("source", source.to_string()),
                            ("target", target.to_string()),
                        ],
                    );
                }
            }
            if let Some(path) = segment.strip_prefix("failed to remove wordbank file ") {
                return tr_args(
                    app,
                    "backend.wordbank.remove_file_failed",
                    "Failed to remove wordbank file {path}",
                    &[("path", path.to_string())],
                );
            }
            if let Some(wordbank_id) = capture_wrapped(segment, "wordbank '", "' was not found") {
                return tr_args(
                    app,
                    "backend.wordbank.not_found",
                    "Wordbank '{wordbank_id}' was not found",
                    &[("wordbank_id", wordbank_id)],
                );
            }
            if let Some(key) = capture_wrapped(segment, "wordbank entry group '", "' was not found")
            {
                return tr_args(
                    app,
                    "backend.wordbank.entry_group_not_found",
                    "Wordbank entry group '{key}' was not found",
                    &[("key", key)],
                );
            }
            if let Some(value) = capture_wrapped(segment, "word entry '", "' was not found") {
                return tr_args(
                    app,
                    "backend.wordbank.entry_not_found",
                    "Word entry '{value}' was not found",
                    &[("value", value)],
                );
            }
            if let Some(ch) = capture_wrapped(segment, "character '", "' did not have pinyin") {
                return tr_args(
                    app,
                    "backend.wordbank.character_missing_pinyin",
                    "Character '{character}' did not have pinyin",
                    &[("character", ch)],
                );
            }
            segment.to_string()
        }
    }
}

fn localize_with_suffix<R: Runtime, T: Manager<R>>(
    app: &T,
    key: &str,
    fallback: &str,
    arg_name: &str,
    suffix: &str,
) -> String {
    let value = suffix.trim_start_matches(':').trim();
    if value.is_empty() {
        tr(app, key, fallback)
    } else {
        tr_args(
            app,
            key,
            &format!("{fallback}: {{{arg_name}}}"),
            &[(arg_name, value.to_string())],
        )
    }
}

fn capture_wrapped(segment: &str, prefix: &str, suffix: &str) -> Option<String> {
    segment
        .strip_prefix(prefix)
        .and_then(|value| value.strip_suffix(suffix))
        .map(ToOwned::to_owned)
}

fn split_wordbank_join_args(rest: &str) -> [(&str, String); 2] {
    let rest = rest.trim();
    if let Some((task, err)) = rest.split_once(" task: ") {
        [("task", task.to_string()), ("err", err.to_string())]
    } else {
        [
            ("task", rest.trim_end_matches(" task").to_string()),
            ("err", String::new()),
        ]
    }
}
