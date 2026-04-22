<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { appCacheDir, join } from "@tauri-apps/api/path";
  import { readFile } from "@tauri-apps/plugin-fs";
  import { m } from "$lib/i18n/paraglide/messages";
  import settings from "$lib/stores/settings";

  type InputDeviceInfo = {
    id: string;
    name: string;
    is_default: boolean;
  };

  type AsrModelKind = "qwen3_asr" | "sense_voice_small";
  type AsrExecutionMode = "auto" | "only_cpu";
  type AsrLanguage = "auto" | "zh" | "en" | "yue" | "ja" | "ko";

  type AsrStatus = {
    isLoaded: boolean;
    currentModel: AsrModelKind | null;
    currentMode: AsrExecutionMode | null;
    currentLanguage: AsrLanguage | null;
  };

  type StopRecordingResult = {
    file_path: string;
    sample_rate: number;
    channels: number;
    device_id: string;
    device_name: string;
  };

  type AsrRecognitionResult = {
    text: string | string[];
    model: AsrModelKind;
    audioPath: string;
  };

  type WordbankTokenHomophoneOptions = {
    token: string;
    options: string[];
  };

  type ResultToken = {
    id: string;
    value: string;
    options: string[];
  };

  type AsrHotkeyEventPayload = {
    shortcut: string;
    state: "pressed" | "released";
  };

  const modelOptions: Array<{ value: AsrModelKind; label: string }> = [
    { value: "sense_voice_small", label: m.tools_audio_asr_model_sense_voice_small() },
    { value: "qwen3_asr", label: m.tools_audio_asr_model_qwen3_asr() }
  ];
  const executionModeOptions: Array<{ value: AsrExecutionMode; label: string }> = [
    { value: "auto", label: m.tools_audio_asr_execution_mode_auto() },
    { value: "only_cpu", label: m.tools_audio_asr_execution_mode_only_cpu() }
  ];
  const languageOptions: Array<{ value: AsrLanguage; label: string }> = [
    { value: "auto", label: m.tools_audio_asr_language_auto() },
    { value: "zh", label: m.tools_audio_asr_language_zh() },
    { value: "en", label: m.tools_audio_asr_language_en() },
    { value: "yue", label: m.tools_audio_asr_language_yue() },
    { value: "ja", label: m.tools_audio_asr_language_ja() },
    { value: "ko", label: m.tools_audio_asr_language_ko() }
  ];

  let devices: InputDeviceInfo[] = $state([]);
  let selectedId = $state("");
  let selectedModel: AsrModelKind = $state("sense_voice_small");
  let selectedExecutionMode: AsrExecutionMode = $state("auto");
  let selectedLanguage: AsrLanguage = $state("auto");
  let enableFitting = $state(false);
  let enableDecomposition = $state(false);
  let asrStatus: AsrStatus = $state({ isLoaded: false, currentModel: null, currentMode: null, currentLanguage: null });
  let recording = $state(false);
  let loadingModel = $state(false);
  let rebuildingFitter = $state(false);
  let rebuildingDecomposer = $state(false);
  let recognizing = $state(false);
  let copyingResult = $state(false);
  let audioUrl = $state("");
  let editableResultText = $state("");
  let resultTokens: ResultToken[] = $state([]);
  let resultEditorEnabled = $state(true);
  let error = $state("");
  let hotkeyEnabled = $state(settings.asr_hotkey_enabled.get());
  let hotkeyShortcut = $state(settings.asr_hotkey_shortcut.get());
  let hotkeyTriggerMode = $state(settings.asr_hotkey_trigger_mode.get());
  let recordingTriggeredByShortcut = $state(false);

  function modelLabel(model: AsrModelKind | null) {
    if (!model) return "";
    return modelOptions.find((item) => item.value === model)?.label ?? model;
  }

  function executionModeLabel(mode: AsrExecutionMode | null) {
    if (!mode) return "";
    return executionModeOptions.find((item) => item.value === mode)?.label ?? mode;
  }

  function supportsDecomposition(language: AsrLanguage) {
    return language === "auto" || language === "zh";
  }

  function currentLanguageForDecomposition() {
    return asrStatus.isLoaded ? (asrStatus.currentLanguage ?? selectedLanguage) : selectedLanguage;
  }

  function resetRecognitionResult() {
    resultTokens = [];
    editableResultText = "";
    resultEditorEnabled = true;
  }

  function currentResultText() {
    if (resultTokens.length > 0 && !resultEditorEnabled) {
      return resultTokens.map((token) => token.value).join("");
    }
    return editableResultText;
  }

  function syncEditableResultTextFromTokens() {
    editableResultText = resultTokens.map((token) => token.value).join("");
  }

  function hasRecognitionResult() {
    return currentResultText().trim().length > 0 || resultTokens.length > 0;
  }

  function showTokenResultView() {
    return resultTokens.length > 0 && !resultEditorEnabled;
  }

  function shouldShowHotkeyHint() {
    return hotkeyEnabled && hotkeyShortcut.trim().length > 0;
  }

  $effect(() => {
    if (!supportsDecomposition(currentLanguageForDecomposition())) {
      enableFitting = false;
      enableDecomposition = false;
    }
  });

  async function loadDevices() {
    devices = await invoke<InputDeviceInfo[]>("list_input_devices");
    selectedId = devices.find((device) => device.is_default)?.id ?? devices[0]?.id ?? "";
  }

  async function loadAsrModel(model: AsrModelKind, mode: AsrExecutionMode, language: AsrLanguage) {
    error = "";
    loadingModel = true;
    asrStatus = { isLoaded: false, currentModel: model, currentMode: mode, currentLanguage: language };

    try {
      asrStatus = await invoke<AsrStatus>("load_asr_model", { model, mode, language });
    } catch (e) {
      asrStatus = { isLoaded: false, currentModel: null, currentMode: null, currentLanguage: null };
      error = String(e);
    } finally {
      loadingModel = false;
    }
  }

  async function startRecording() {
    if (recording || recognizing || loadingModel || !selectedId || !asrStatus.isLoaded) return;

    error = "";
    resetRecognitionResult();

    const dir = await appCacheDir();
    const recorderDir = await join(dir, "recorder");
    const outputPath = await join(recorderDir, `asr-${Date.now()}`);

    try {
      await invoke("start_recording", {
        deviceId: selectedId,
        outputPath,
        sampleRate: 16000
      });
      recording = true;
    } catch (e) {
      error = String(e);
      recording = false;
    }
  }

  async function stopRecording() {
    if (!recording || recognizing) return;

    error = "";
    recognizing = true;

    try {
      const result = await invoke<StopRecordingResult>("stop_recording");
      const bytes = await readFile(result.file_path);
      const blob = new Blob([bytes], { type: "audio/wav" });

      if (audioUrl) URL.revokeObjectURL(audioUrl);
      audioUrl = URL.createObjectURL(blob);

      const recognition = await invoke<AsrRecognitionResult>("recognize_audio", {
        wavPath: result.file_path,
        enableFitting,
        enableDecomposition
      });
      if (Array.isArray(recognition.text) && recognition.text.length > 0) {
        const homophoneOptions = await invoke<WordbankTokenHomophoneOptions[]>("list_enabled_wordbank_homophones", {
          tokens: recognition.text
        });
        const resultId = Date.now();
        resultTokens = recognition.text.map((token, index) => {
          const options = homophoneOptions[index]?.options ?? [];
          return {
            id: `${resultId}-${index}`,
            value: token,
            options
          };
        });
        syncEditableResultTextFromTokens();
        resultEditorEnabled = false;
      } else {
        resultTokens = [];
        editableResultText =
          (typeof recognition.text === "string" ? recognition.text : "") || m.tools_audio_asr_recognition_empty_text();
        resultEditorEnabled = true;
      }
    } catch (e) {
      error = String(e);
    } finally {
      recording = false;
      recognizing = false;
    }
  }

  async function reloadCurrentModel() {
    if (recording || recognizing) return;
    resetRecognitionResult();
    await loadAsrModel(selectedModel, selectedExecutionMode, selectedLanguage);
  }

  async function rebuildCurrentDecomposer() {
    if (recording || recognizing || loadingModel || !asrStatus.isLoaded || !enableDecomposition) return;
    error = "";
    rebuildingDecomposer = true;
    try {
      await invoke("rebuild_asr_decomposer");
    } catch (e) {
      error = String(e);
    } finally {
      rebuildingDecomposer = false;
    }
  }

  async function rebuildCurrentFitter() {
    if (recording || recognizing || loadingModel || !asrStatus.isLoaded || !enableFitting) return;
    error = "";
    rebuildingFitter = true;
    try {
      await invoke("rebuild_asr_fitter");
    } catch (e) {
      error = String(e);
    } finally {
      rebuildingFitter = false;
    }
  }

  async function copyRecognitionResult() {
    const text = currentResultText().trim();
    if (!text) return;

    error = "";
    copyingResult = true;
    try {
      await navigator.clipboard.writeText(text);
    } catch (e) {
      error = String(e);
    } finally {
      copyingResult = false;
    }
  }

  function enableResultEditor() {
    if (resultTokens.length === 0) {
      resultEditorEnabled = true;
      return;
    }

    syncEditableResultTextFromTokens();
    resultEditorEnabled = true;
  }

  function updateResultToken(index: number, nextValue: string) {
    if (!resultTokens[index]) return;
    resultTokens[index].value = nextValue;
    syncEditableResultTextFromTokens();
  }

  function selectResultTokenOption(index: number, nextValue: string, dropdown?: HTMLDetailsElement | null) {
    updateResultToken(index, nextValue);
    if (dropdown) {
      dropdown.open = false;
    }
  }

  async function destroyAsrModel() {
    try {
      await invoke("destroy_asr_model");
    } catch {
      // 页面退出时只做兜底清理，不额外打断用户。
    }
  }

  function handleShortcutPressed() {
    if (!hotkeyEnabled || !hotkeyShortcut || !asrStatus.isLoaded) return;
    if (recognizing || loadingModel) return;
    if (hotkeyTriggerMode === "press_press") {
      if (recording) {
        void stopRecording();
      } else {
        void startRecording();
      }
      return;
    }

    recordingTriggeredByShortcut = true;
    void startRecording();
  }

  function handleShortcutReleased() {
    if (hotkeyTriggerMode !== "press_release") return;
    if (!recordingTriggeredByShortcut || !hotkeyEnabled || !hotkeyShortcut) return;
    recordingTriggeredByShortcut = false;
    void stopRecording();
  }

  onMount(() => {
    let disposed = false;
    let unlistenHotkey: (() => void) | undefined;
    const unsubscribeHotkeyEnabled = settings.asr_hotkey_enabled.subscribe((value) => {
      hotkeyEnabled = value;
    });
    const unsubscribeHotkeyShortcut = settings.asr_hotkey_shortcut.subscribe((value) => {
      hotkeyShortcut = value;
    });
    const unsubscribeHotkeyTriggerMode = settings.asr_hotkey_trigger_mode.subscribe((value) => {
      hotkeyTriggerMode = value;
    });

    void (async () => {
      try {
        await loadDevices();
      } catch (e) {
        if (!disposed) {
          error = String(e);
        }
      }
    })();

    void listen<AsrHotkeyEventPayload>("asr://hotkey", (event) => {
      if (!hotkeyEnabled) return;

      if (event.payload.state === "pressed") {
        handleShortcutPressed();
        return;
      }

      handleShortcutReleased();
    }).then((cleanup) => {
      unlistenHotkey = cleanup;
    });

    return () => {
      disposed = true;
      if (audioUrl) URL.revokeObjectURL(audioUrl);
      recordingTriggeredByShortcut = false;
      unlistenHotkey?.();
      unsubscribeHotkeyEnabled();
      unsubscribeHotkeyShortcut();
      unsubscribeHotkeyTriggerMode();
      void destroyAsrModel();
    };
  });
</script>

<div class="mx-auto flex h-full min-h-0 w-full max-w-6xl flex-col pb-10">
  <div class="card h-full min-h-0 border border-base-300 bg-base-100 shadow-md">
    <div class="card-body flex min-h-0 gap-4 overflow-hidden">
      <div class="max-h-6xl flex flex-col gap-4 lg:flex-row">
        <div class="form-control w-full">
          <div class="label">
            <span class="label-text">{m.tools_audio_asr_model_label()}</span>
          </div>
          <select
            class="select-bordered select w-full"
            bind:value={selectedModel}
            disabled={loadingModel || recording || recognizing}>
            {#each modelOptions as model (model.value)}
              <option value={model.value}>{model.label}</option>
            {/each}
          </select>
        </div>

        <div class="form-control w-full">
          <div class="label">
            <span class="label-text">{m.tools_audio_asr_execution_mode_label()}</span>
          </div>
          <select
            class="select-bordered select w-full"
            bind:value={selectedExecutionMode}
            disabled={loadingModel || recording || recognizing}>
            {#each executionModeOptions as mode (mode.value)}
              <option value={mode.value}>{mode.label}</option>
            {/each}
          </select>
        </div>

        <div class="form-control w-full">
          <div class="label">
            <span class="label-text">{m.tools_audio_asr_language_label()}</span>
          </div>
          <select
            class="select-bordered select w-full"
            bind:value={selectedLanguage}
            disabled={loadingModel || recording || recognizing}>
            {#each languageOptions as language (language.value)}
              <option value={language.value}>{language.label}</option>
            {/each}
          </select>
        </div>

        <div class="form-control w-full">
          <div class="label">
            <span class="label-text">{m.tools_audio_asr_device_label()}</span>
          </div>
          <select
            class="select-bordered select w-full"
            bind:value={selectedId}
            disabled={recording || loadingModel || recognizing}>
            {#each devices as device (device.id)}
              <option value={device.id}>
                {device.name}{device.is_default ? ` (${m.tools_audio_asr_device_default()})` : ""}
              </option>
            {/each}
          </select>
        </div>
      </div>

      <div class="flex flex-wrap items-center gap-3">
        <div class="badge badge-outline p-4">
          {#if loadingModel}
            {m.tools_audio_asr_status_loading({
              model: modelLabel(selectedModel),
              mode: executionModeLabel(selectedExecutionMode)
            })}
          {:else if asrStatus.isLoaded}
            {m.tools_audio_asr_status_ready({
              model: modelLabel(asrStatus.currentModel),
              mode: executionModeLabel(asrStatus.currentMode)
            })}
          {:else}
            {m.tools_audio_asr_status_unloaded()}
          {/if}
        </div>

        <button
          class="btn btn-secondary"
          onclick={() => void reloadCurrentModel()}
          disabled={loadingModel || recording || recognizing}>
          {asrStatus.isLoaded ? m.tools_audio_asr_reload_model() : m.tools_audio_asr_load_model()}
        </button>

        {#if supportsDecomposition(currentLanguageForDecomposition())}
          <label class="label cursor-pointer gap-3 rounded-lg border border-base-300 px-4 py-2">
            <span class="label-text">{m.tools_audio_asr_fitting_toggle()}</span>
            <input
              type="checkbox"
              class="checkbox checkbox-sm"
              bind:checked={enableFitting}
              disabled={loadingModel || recording || recognizing || !asrStatus.isLoaded} />
          </label>

          {#if enableFitting}
            <button
              class="btn btn-outline"
              onclick={() => void rebuildCurrentFitter()}
              disabled={loadingModel || rebuildingFitter || recording || recognizing || !asrStatus.isLoaded}>
              {m.tools_audio_asr_rebuild_fitter()}
            </button>
          {/if}

          <label class="label cursor-pointer gap-3 rounded-lg border border-base-300 px-4 py-2">
            <span class="label-text">{m.tools_audio_asr_decomposition_toggle()}</span>
            <input
              type="checkbox"
              class="checkbox checkbox-sm"
              bind:checked={enableDecomposition}
              disabled={loadingModel || recording || recognizing || !asrStatus.isLoaded} />
          </label>

          {#if enableDecomposition}
            <button
              class="btn btn-outline"
              onclick={() => void rebuildCurrentDecomposer()}
              disabled={loadingModel || rebuildingDecomposer || recording || recognizing || !asrStatus.isLoaded}>
              {m.tools_audio_asr_rebuild_decomposer()}
            </button>
          {/if}
        {/if}
      </div>

      <div class="flex flex-wrap items-center gap-3">
        {#if !recording}
          <button
            class="btn btn-primary"
            onclick={() => void startRecording()}
            disabled={!selectedId || !asrStatus.isLoaded || loadingModel || recognizing}>
            {m.tools_audio_asr_start_recording()}
          </button>
        {:else}
          <button class="btn btn-error" onclick={() => void stopRecording()} disabled={recognizing}>
            {m.tools_audio_asr_stop_and_recognize()}
          </button>
        {/if}

        {#if recognizing}
          <span class="loading loading-md loading-spinner"></span>
        {/if}

        {#if shouldShowHotkeyHint()}
          <div class="ml-auto alert min-h-0 py-2 alert-info">
            <span class="text-sm">
              {m.tools_audio_asr_hotkey_hint({
                shortcut: hotkeyShortcut,
                mode:
                  hotkeyTriggerMode === "press_release"
                    ? m.tools_audio_asr_hotkey_mode_press_release()
                    : m.tools_audio_asr_hotkey_mode_press_press()
              })}
            </span>
          </div>
        {/if}
      </div>

      {#if error}
        <div class="alert shrink-0 alert-error">
          <span>{error}</span>
        </div>
      {/if}

      {#if audioUrl}
        <div class="shrink-0 space-y-2">
          <div class="text-sm font-medium">{m.tools_audio_asr_audio_playback()}</div>
          <audio class="w-full" controls src={audioUrl}></audio>
        </div>
      {/if}

      <div class="flex min-h-0 flex-1 flex-col space-y-2">
        <div class="flex shrink-0 flex-wrap items-center justify-between gap-3">
          <div class="text-sm font-medium">{m.tools_audio_asr_result_label()}</div>

          <div class="flex flex-wrap items-center gap-2">
            {#if showTokenResultView()}
              <button class="btn btn-outline btn-sm" onclick={enableResultEditor}>
                {m.tools_audio_asr_result_edit_text_action()}
              </button>
            {/if}

            <button
              class="btn btn-outline btn-sm"
              onclick={() => void copyRecognitionResult()}
              disabled={copyingResult || !hasRecognitionResult()}>
              {m.tools_audio_asr_result_copy_action()}
            </button>
          </div>
        </div>

        <div class="min-h-0 flex-1 overflow-auto rounded-box border border-base-300 bg-base-100 p-4">
          {#if !hasRecognitionResult()}
            <div class="whitespace-pre-wrap text-base-content/50">
              {m.tools_audio_asr_result_placeholder()}
            </div>
          {:else if showTokenResultView()}
            <div class="flex flex-wrap gap-2">
              {#each resultTokens as token, index (token.id)}
                {#if token.options.length > 1}
                  <details class="dropdown">
                    <summary
                      class="btn h-auto min-h-0 max-w-52 justify-between rounded-xl border-base-300 bg-base-200 px-3 py-2 text-sm font-normal shadow-sm btn-sm">
                      <span class="truncate">{token.value}</span>
                      <svg
                        xmlns="http://www.w3.org/2000/svg"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        class="size-4 shrink-0 opacity-60">
                        <path stroke-linecap="round" stroke-linejoin="round" d="m6 9 6 6 6-6" />
                      </svg>
                    </summary>
                    <ul
                      class="dropdown-content menu z-10 mt-2 w-44 rounded-box border border-base-300 bg-base-100 p-2 shadow-xl">
                      {#each token.options as option (`${token.id}-${option}`)}
                        <li>
                          <button
                            type="button"
                            class:active={option === token.value}
                            onclick={(event) => selectResultTokenOption(index, option, event.currentTarget.closest("details"))}>
                            {option}
                          </button>
                        </li>
                      {/each}
                    </ul>
                  </details>
                {:else}
                  <div class="rounded-lg border border-base-300 bg-base-200 px-3 py-1.5 text-sm shadow-sm">
                    {token.value}
                  </div>
                {/if}
              {/each}
            </div>
          {:else}
            <textarea
              class="textarea-bordered textarea h-full w-full resize-none bg-base-100 leading-7"
              bind:value={editableResultText}
              spellcheck={false}></textarea>
          {/if}
        </div>
      </div>
    </div>
  </div>
</div>
