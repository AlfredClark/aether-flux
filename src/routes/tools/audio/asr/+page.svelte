<script lang="ts">
  import { onMount } from "svelte";
  import { resolve } from "$app/paths";
  import { invoke } from "@tauri-apps/api/core";
  import { appCacheDir, join } from "@tauri-apps/api/path";
  import { readFile } from "@tauri-apps/plugin-fs";
  import { m } from "$lib/i18n/paraglide/messages";

  type InputDeviceInfo = {
    id: string;
    name: string;
    is_default: boolean;
  };

  type AsrModelKind = "qwen3_asr" | "sense_voice_small";
  type AsrExecutionMode = "auto" | "only_cpu";

  type AsrStatus = {
    isLoaded: boolean;
    currentModel: AsrModelKind | null;
    currentMode: AsrExecutionMode | null;
  };

  type StopRecordingResult = {
    file_path: string;
    sample_rate: number;
    channels: number;
    device_id: string;
    device_name: string;
  };

  type AsrRecognitionResult = {
    text: string;
    model: AsrModelKind;
    audioPath: string;
  };

  const modelOptions: Array<{ value: AsrModelKind; label: string }> = [
    { value: "sense_voice_small", label: m.tools_audio_asr_model_sense_voice_small() },
    { value: "qwen3_asr", label: m.tools_audio_asr_model_qwen3_asr() }
  ];
  const executionModeOptions: Array<{ value: AsrExecutionMode; label: string }> = [
    { value: "auto", label: m.tools_audio_asr_execution_mode_auto() },
    { value: "only_cpu", label: m.tools_audio_asr_execution_mode_only_cpu() }
  ];

  let devices: InputDeviceInfo[] = [];
  let selectedId = "";
  let selectedModel: AsrModelKind = "sense_voice_small";
  let selectedExecutionMode: AsrExecutionMode = "auto";
  let asrStatus: AsrStatus = { isLoaded: false, currentModel: null, currentMode: null };
  let recording = false;
  let loadingModel = false;
  let recognizing = false;
  let audioUrl = "";
  let recognitionText = "";
  let error = "";

  function modelLabel(model: AsrModelKind | null) {
    if (!model) return "";
    return modelOptions.find((item) => item.value === model)?.label ?? model;
  }

  function executionModeLabel(mode: AsrExecutionMode | null) {
    if (!mode) return "";
    return executionModeOptions.find((item) => item.value === mode)?.label ?? mode;
  }

  async function loadDevices() {
    devices = await invoke<InputDeviceInfo[]>("list_input_devices");
    selectedId = devices.find((device) => device.is_default)?.id ?? devices[0]?.id ?? "";
  }

  async function loadAsrModel(model: AsrModelKind, mode: AsrExecutionMode) {
    error = "";
    loadingModel = true;
    asrStatus = { isLoaded: false, currentModel: model, currentMode: mode };

    try {
      asrStatus = await invoke<AsrStatus>("load_asr_model", { model, mode });
    } catch (e) {
      asrStatus = { isLoaded: false, currentModel: null, currentMode: null };
      error = String(e);
    } finally {
      loadingModel = false;
    }
  }

  async function startRecording() {
    error = "";
    recognitionText = "";

    const dir = await appCacheDir();
    const outputPath = await join(dir, `asr-${Date.now()}`);

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
    error = "";
    recognizing = true;

    try {
      const result = await invoke<StopRecordingResult>("stop_recording");
      const bytes = await readFile(result.file_path);
      const blob = new Blob([bytes], { type: "audio/wav" });

      if (audioUrl) URL.revokeObjectURL(audioUrl);
      audioUrl = URL.createObjectURL(blob);

      const recognition = await invoke<AsrRecognitionResult>("recognize_audio", {
        wavPath: result.file_path
      });
      recognitionText = recognition.text || m.tools_audio_asr_recognition_empty_text();
    } catch (e) {
      error = String(e);
    } finally {
      recording = false;
      recognizing = false;
    }
  }

  async function reloadCurrentModel() {
    if (recording || recognizing) return;
    recognitionText = "";
    await loadAsrModel(selectedModel, selectedExecutionMode);
  }

  async function destroyAsrModel() {
    try {
      await invoke("destroy_asr_model");
    } catch {
      // 页面退出时只做兜底清理，不额外打断用户。
    }
  }

  onMount(() => {
    let disposed = false;

    void (async () => {
      try {
        await loadDevices();
      } catch (e) {
        if (!disposed) {
          error = String(e);
        }
      }
    })();

    return () => {
      disposed = true;
      if (audioUrl) URL.revokeObjectURL(audioUrl);
      void destroyAsrModel();
    };
  });
</script>

<div class="tabs-lift tabs flex h-full w-full flex-wrap content-start overflow-hidden">
  <input type="radio" name="asr_tabs" class="tab px-10" aria-label={m.tools_audio_asr_use()} checked />
  <div class="tab-content order-last min-h-0 flex-1 basis-full overflow-hidden border-base-300 bg-base-100 p-6">
    <div class="mx-auto flex h-full min-h-0 w-full max-w-6xl flex-col">
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
              on:click={() => void reloadCurrentModel()}
              disabled={loadingModel || recording || recognizing}>
              {asrStatus.isLoaded ? m.tools_audio_asr_reload_model() : m.tools_audio_asr_load_model()}
            </button>
          </div>

          <div class="flex gap-3">
            {#if !recording}
              <button
                class="btn btn-primary"
                on:click={() => void startRecording()}
                disabled={!selectedId || !asrStatus.isLoaded || loadingModel || recognizing}>
                {m.tools_audio_asr_start_recording()}
              </button>
            {:else}
              <button class="btn btn-error" on:click={() => void stopRecording()} disabled={recognizing}>
                {m.tools_audio_asr_stop_and_recognize()}
              </button>
            {/if}

            {#if recognizing}
              <span class="loading loading-md loading-spinner"></span>
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
            <div class="shrink-0 text-sm font-medium">{m.tools_audio_asr_result_label()}</div>
            <textarea
              class="textarea-bordered textarea h-full min-h-0 w-full flex-1 resize-none"
              bind:value={recognitionText}
              readonly
              placeholder={m.tools_audio_asr_result_placeholder()}></textarea>
          </div>
        </div>
      </div>
    </div>
  </div>

  <input type="radio" name="asr_tabs" class="tab px-10" aria-label={m.tools_audio_asr_word_bank()} />
  <div class="tab-content order-last min-h-0 flex-1 basis-full overflow-hidden border-base-300 bg-base-100 p-6">
    字库功能待实现
  </div>

  <input type="radio" name="asr_tabs" class="tab px-10" aria-label={m.tools_audio_asr_settings()} />
  <div class="tab-content order-last min-h-0 flex-1 basis-full overflow-hidden border-base-300 bg-base-100 p-6">
    设置功能待实现
  </div>

  <a class="btn ml-auto shrink-0 px-10 btn-error" href={resolve("/tools")}>{m.msg_back()}</a>
</div>
