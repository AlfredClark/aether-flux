<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { m } from "$lib/i18n/paraglide/messages";
  import { openModal } from "$lib/stores/modal";

  type AsrRecordingCacheStats = {
    fileCount: number;
    totalBytes: number;
  };

  let cacheStats: AsrRecordingCacheStats = {
    fileCount: 0,
    totalBytes: 0
  };
  let loadingStats = false;
  let clearingCache = false;
  let resettingWordbank = false;
  let error = "";

  function formatBytes(bytes: number) {
    if (bytes <= 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    let value = bytes;
    let index = 0;
    while (value >= 1024 && index < units.length - 1) {
      value /= 1024;
      index += 1;
    }
    return `${value >= 10 || index === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[index]}`;
  }

  function confirmByModal(options: { title: string; message: string }) {
    return new Promise<boolean>((resolve) => {
      openModal({
        title: options.title,
        message: options.message,
        type: "warning",
        backdrop: true,
        cancelText: m.msg_cancel(),
        confirmText: m.msg_confirm(),
        onConfirm: () => resolve(true),
        onCancel: () => resolve(false)
      });
    });
  }

  async function loadCacheStats() {
    loadingStats = true;
    error = "";
    try {
      cacheStats = await invoke<AsrRecordingCacheStats>("get_asr_recording_cache_stats");
    } catch (e) {
      error = String(e);
    } finally {
      loadingStats = false;
    }
  }

  async function clearRecordingCache() {
    const confirmed = await confirmByModal({
      title: m.tools_audio_asr_settings_cache_clear_confirm_title(),
      message: m.tools_audio_asr_settings_cache_clear_confirm_message()
    });
    if (!confirmed) return;

    clearingCache = true;
    error = "";
    try {
      cacheStats = await invoke<AsrRecordingCacheStats>("clear_asr_recording_cache");
    } catch (e) {
      error = String(e);
    } finally {
      clearingCache = false;
    }
  }

  async function resetWordbank() {
    const confirmed = await confirmByModal({
      title: m.tools_audio_asr_settings_wordbank_reset_confirm_title(),
      message: m.tools_audio_asr_settings_wordbank_reset_confirm_message()
    });
    if (!confirmed) return;

    resettingWordbank = true;
    error = "";
    try {
      await invoke("reset_wordbank_database");
      window.dispatchEvent(new CustomEvent("asr:wordbank-reset"));
    } catch (e) {
      error = String(e);
    } finally {
      resettingWordbank = false;
    }
  }

  onMount(() => {
    void loadCacheStats();
  });
</script>

<div class="mx-auto flex h-full min-h-0 w-full max-w-5xl flex-col">
  <div class="grid gap-4 lg:grid-cols-2">
    <div class="card border border-base-300 bg-base-100 shadow-md">
      <div class="card-body gap-4">
        <div class="flex items-center justify-between gap-3">
          <div class="text-base font-semibold">{m.tools_audio_asr_settings_cache_title()}</div>
          <button
            class="btn btn-sm btn-secondary"
            type="button"
            on:click={() => void loadCacheStats()}
            disabled={loadingStats || clearingCache || resettingWordbank}>
            {m.tools_audio_asr_settings_refresh_action()}
          </button>
        </div>

        <div class="stats stats-vertical border border-base-300 bg-base-100 lg:stats-horizontal">
          <div class="stat">
            <div class="stat-title">{m.tools_audio_asr_settings_cache_file_count_label()}</div>
            <div class="stat-value text-2xl">{cacheStats.fileCount}</div>
          </div>
          <div class="stat">
            <div class="stat-title">{m.tools_audio_asr_settings_cache_total_size_label()}</div>
            <div class="stat-value text-2xl">{formatBytes(cacheStats.totalBytes)}</div>
          </div>
        </div>

        <div class="text-sm text-base-content/60">
          {m.tools_audio_asr_settings_cache_description()}
        </div>

        <button
          class="btn btn-warning"
          type="button"
          on:click={() => void clearRecordingCache()}
          disabled={loadingStats || clearingCache || resettingWordbank}>
          {m.tools_audio_asr_settings_cache_clear_action()}
        </button>
      </div>
    </div>

    <div class="card border border-base-300 bg-base-100 shadow-md">
      <div class="card-body gap-4">
        <div class="text-base font-semibold">{m.tools_audio_asr_settings_wordbank_title()}</div>
        <div class="text-sm text-base-content/60">
          {m.tools_audio_asr_settings_wordbank_description()}
        </div>
        <button
          class="btn btn-error"
          type="button"
          on:click={() => void resetWordbank()}
          disabled={loadingStats || clearingCache || resettingWordbank}>
          {m.tools_audio_asr_settings_wordbank_reset_action()}
        </button>
      </div>
    </div>
  </div>

  {#if error}
    <div class="mt-4 alert alert-error">
      <span>{error}</span>
    </div>
  {/if}
</div>
