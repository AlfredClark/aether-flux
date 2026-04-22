<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { save } from "@tauri-apps/plugin-dialog";
  import { writeTextFile } from "@tauri-apps/plugin-fs";
  import { m } from "$lib/i18n/paraglide/messages";
  import { openModal } from "$lib/stores/modal";

  type AsrRecordingCacheStats = {
    fileCount: number;
    totalBytes: number;
  };

  type WordbankSummary = {
    id: string;
    name: string;
    description: string | null;
    prefix: string | null;
    suffix: string | null;
    sortOrder: number;
    isDefault: boolean;
    isEnabled: boolean;
    entryTotal: number;
  };

  type WordbankBackupResult = {
    backupPath: string;
  };

  let cacheStats: AsrRecordingCacheStats = {
    fileCount: 0,
    totalBytes: 0
  };
  let wordBanks: WordbankSummary[] = [];
  let selectedExportIds: string[] = [];
  let loadingStats = false;
  let loadingWordbanks = false;
  let clearingCache = false;
  let exportingWordbanks = false;
  let importingWordbanks = false;
  let backingUpWordbank = false;
  let resettingWordbank = false;
  let importInputElement: HTMLInputElement | undefined;

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

  function showStatusModal(options: { title: string; message: string; type: "success" | "error" | "info" }) {
    openModal({
      title: options.title,
      message: options.message,
      type: options.type,
      backdrop: true,
      confirmText: m.msg_confirm()
    });
  }

  function formatExportFilename() {
    const now = new Date();
    const parts = [
      now.getFullYear(),
      String(now.getMonth() + 1).padStart(2, "0"),
      String(now.getDate()).padStart(2, "0"),
      String(now.getHours()).padStart(2, "0"),
      String(now.getMinutes()).padStart(2, "0"),
      String(now.getSeconds()).padStart(2, "0")
    ];
    return `wordbanks-${parts[0]}${parts[1]}${parts[2]}-${parts[3]}${parts[4]}${parts[5]}.json`;
  }

  async function saveExportPayload(payload: string) {
    const targetPath = await save({
      defaultPath: formatExportFilename(),
      filters: [
        {
          name: "JSON",
          extensions: ["json"]
        }
      ]
    });

    if (!targetPath) {
      return false;
    }

    await writeTextFile(targetPath, payload);
    return true;
  }

  async function loadCacheStats() {
    loadingStats = true;
    try {
      cacheStats = await invoke<AsrRecordingCacheStats>("get_asr_recording_cache_stats");
    } catch (e) {
      showStatusModal({
        title: m.msg_error(),
        message: String(e),
        type: "error"
      });
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
    try {
      cacheStats = await invoke<AsrRecordingCacheStats>("clear_asr_recording_cache");
    } catch (e) {
      showStatusModal({
        title: m.msg_error(),
        message: String(e),
        type: "error"
      });
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
    try {
      await invoke("reset_wordbank_database");
      await loadWordbanks();
      window.dispatchEvent(new CustomEvent("asr:wordbank-reset"));
    } catch (e) {
      showStatusModal({
        title: m.msg_error(),
        message: String(e),
        type: "error"
      });
    } finally {
      resettingWordbank = false;
    }
  }

  function renderWordbankName(bank: WordbankSummary) {
    return `${bank.prefix ?? ""}${bank.name}${bank.suffix ?? ""}`;
  }

  async function loadWordbanks() {
    loadingWordbanks = true;
    try {
      wordBanks = await invoke<WordbankSummary[]>("list_wordbanks");
      const existingIds = new Set(wordBanks.map((bank) => bank.id));
      selectedExportIds = selectedExportIds.filter((id) => existingIds.has(id));
    } catch (e) {
      wordBanks = [];
      selectedExportIds = [];
      showStatusModal({
        title: m.msg_error(),
        message: String(e),
        type: "error"
      });
    } finally {
      loadingWordbanks = false;
    }
  }

  function toggleExportWordbank(wordbankId: string) {
    if (selectedExportIds.includes(wordbankId)) {
      selectedExportIds = selectedExportIds.filter((id) => id !== wordbankId);
    } else {
      selectedExportIds = [...selectedExportIds, wordbankId];
    }
  }

  async function exportSelectedWordbanks() {
    if (selectedExportIds.length === 0) return;

    exportingWordbanks = true;
    try {
      const payload = await invoke<string>("export_wordbanks", {
        wordbankIds: selectedExportIds
      });
      const saved = await saveExportPayload(payload);
      if (!saved) {
        return;
      }
      showStatusModal({
        title: m.msg_success(),
        message: m.tools_audio_asr_settings_wordbank_export_success(),
        type: "success"
      });
    } catch (e) {
      showStatusModal({
        title: m.msg_error(),
        message: String(e),
        type: "error"
      });
    } finally {
      exportingWordbanks = false;
    }
  }

  async function handleImportFileChange(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;

    importingWordbanks = true;
    try {
      const payload = await file.text();
      await invoke("import_wordbanks", { payload });
      await loadWordbanks();
      window.dispatchEvent(new CustomEvent("asr:wordbank-reset"));
      showStatusModal({
        title: m.msg_success(),
        message: m.tools_audio_asr_settings_wordbank_import_success(),
        type: "success"
      });
    } catch (e) {
      showStatusModal({
        title: m.msg_error(),
        message: String(e),
        type: "error"
      });
    } finally {
      importingWordbanks = false;
      input.value = "";
    }
  }

  async function backupWordbank() {
    backingUpWordbank = true;
    try {
      const result = await invoke<WordbankBackupResult>("backup_wordbank_database");
      showStatusModal({
        title: m.msg_success(),
        message: m.tools_audio_asr_settings_wordbank_backup_success({ path: result.backupPath }),
        type: "success"
      });
    } catch (e) {
      showStatusModal({
        title: m.msg_error(),
        message: String(e),
        type: "error"
      });
    } finally {
      backingUpWordbank = false;
    }
  }

  onMount(() => {
    void loadCacheStats();
    void loadWordbanks();
  });
</script>

<div class="mx-auto flex h-full min-h-0 w-full max-w-5xl flex-col pr-1 pb-10">
  <div class="flex flex-col gap-4 pb-15">
    <div class="card border border-base-300 bg-base-100 shadow-md">
      <div class="card-body gap-4">
        <div class="flex items-center justify-between gap-3">
          <div class="text-base font-semibold">{m.tools_audio_asr_settings_cache_title()}</div>
          <button
            class="btn btn-sm btn-secondary"
            type="button"
            onclick={() => void loadCacheStats()}
            disabled={loadingStats ||
              clearingCache ||
              exportingWordbanks ||
              importingWordbanks ||
              backingUpWordbank ||
              resettingWordbank}>
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
          onclick={() => void clearRecordingCache()}
          disabled={loadingStats ||
            clearingCache ||
            exportingWordbanks ||
            importingWordbanks ||
            backingUpWordbank ||
            resettingWordbank}>
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
        <div class="space-y-3 rounded-xl border border-base-300 bg-base-100 p-4">
          <div class="flex items-center justify-between gap-3">
            <div class="text-sm font-medium">{m.tools_audio_asr_settings_wordbank_export_title()}</div>
            <button
              class="btn btn-sm btn-secondary"
              type="button"
              onclick={() => void loadWordbanks()}
              disabled={loadingWordbanks || exportingWordbanks || importingWordbanks || backingUpWordbank || resettingWordbank}>
              {m.tools_audio_asr_settings_refresh_action()}
            </button>
          </div>
          <div class="text-sm text-base-content/60">
            {m.tools_audio_asr_settings_wordbank_export_description()}
          </div>
          <div class="max-h-48 overflow-auto rounded-box border border-base-300 bg-base-100 p-3">
            {#if loadingWordbanks}
              <div class="text-sm text-base-content/60">{m.tools_audio_asr_word_bank_management_loading()}</div>
            {:else if wordBanks.length === 0}
              <div class="text-sm text-base-content/60">{m.tools_audio_asr_word_bank_management_list_empty()}</div>
            {:else}
              <div class="space-y-2">
                {#each wordBanks as bank (bank.id)}
                  <label
                    class="flex cursor-pointer items-start gap-3 rounded-lg border border-base-300 px-3 py-2 hover:bg-base-200">
                    <input
                      type="checkbox"
                      class="checkbox mt-0.5 checkbox-sm"
                      checked={selectedExportIds.includes(bank.id)}
                      onchange={() => toggleExportWordbank(bank.id)}
                      disabled={exportingWordbanks || importingWordbanks || backingUpWordbank || resettingWordbank} />
                    <span class="min-w-0 flex-1">
                      <span class="block font-medium">{renderWordbankName(bank)}</span>
                      <span class="block truncate text-xs text-base-content/60">
                        {bank.description || m.tools_audio_asr_word_bank_management_description_empty()}
                      </span>
                    </span>
                  </label>
                {/each}
              </div>
            {/if}
          </div>
          <div class="flex flex-wrap gap-3">
            <button
              class="btn btn-primary"
              type="button"
              onclick={() => void exportSelectedWordbanks()}
              disabled={selectedExportIds.length === 0 ||
                loadingWordbanks ||
                exportingWordbanks ||
                importingWordbanks ||
                backingUpWordbank ||
                resettingWordbank}>
              {m.tools_audio_asr_settings_wordbank_export_action()}
            </button>
            <button
              class="btn btn-outline"
              type="button"
              onclick={() => importInputElement?.click()}
              disabled={loadingWordbanks || exportingWordbanks || importingWordbanks || backingUpWordbank || resettingWordbank}>
              {m.tools_audio_asr_settings_wordbank_import_action()}
            </button>
            <input
              bind:this={importInputElement}
              class="hidden"
              type="file"
              accept="application/json,.json"
              onchange={handleImportFileChange}
              disabled={loadingWordbanks ||
                exportingWordbanks ||
                importingWordbanks ||
                backingUpWordbank ||
                resettingWordbank} />
          </div>
        </div>
        <div class="space-y-3 rounded-xl border border-base-300 bg-base-100 p-4">
          <div class="text-sm font-medium">{m.tools_audio_asr_settings_wordbank_backup_title()}</div>
          <div class="text-sm text-base-content/60">
            {m.tools_audio_asr_settings_wordbank_backup_description()}
          </div>
          <button
            class="btn btn-outline"
            type="button"
            onclick={() => void backupWordbank()}
            disabled={loadingWordbanks || exportingWordbanks || importingWordbanks || backingUpWordbank || resettingWordbank}>
            {m.tools_audio_asr_settings_wordbank_backup_action()}
          </button>
        </div>
        <button
          class="btn mt-auto btn-error"
          type="button"
          onclick={() => void resetWordbank()}
          disabled={loadingStats ||
            clearingCache ||
            loadingWordbanks ||
            exportingWordbanks ||
            importingWordbanks ||
            backingUpWordbank ||
            resettingWordbank}>
          {m.tools_audio_asr_settings_wordbank_reset_action()}
        </button>
      </div>
    </div>
  </div>
</div>
