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

  type AsrStatus = {
    isLoaded: boolean;
    currentModel: AsrModelKind | null;
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
    { value: "sense_voice_small", label: "SenseVoiceSmall" },
    { value: "qwen3_asr", label: "Qwen3-ASR" }
  ];

  let devices: InputDeviceInfo[] = [];
  let selectedId = "";
  let selectedModel: AsrModelKind = "sense_voice_small";
  let asrStatus: AsrStatus = { isLoaded: false, currentModel: null };
  let recording = false;
  let loadingModel = false;
  let recognizing = false;
  let audioUrl = "";
  let recognitionText = "";
  let error = "";

  function currentModelLabel() {
    return modelOptions.find((item) => item.value === selectedModel)?.label ?? selectedModel;
  }

  async function loadDevices() {
    devices = await invoke<InputDeviceInfo[]>("list_input_devices");
    selectedId = devices.find((device) => device.is_default)?.id ?? devices[0]?.id ?? "";
  }

  // 页面进入时自动加载默认模型，录音按钮会在加载完成后解锁。
  async function loadAsrModel(model: AsrModelKind) {
    error = "";
    loadingModel = true;
    asrStatus = { isLoaded: false, currentModel: model };

    try {
      asrStatus = await invoke<AsrStatus>("load_asr_model", { model });
    } catch (e) {
      asrStatus = { isLoaded: false, currentModel: null };
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
      recognitionText = recognition.text || "识别完成，但模型返回了空文本。";
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
    await loadAsrModel(selectedModel);
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
        if (!disposed) {
          await loadAsrModel(selectedModel);
        }
      } catch (e) {
        error = String(e);
      }
    })();

    return () => {
      disposed = true;
      if (audioUrl) URL.revokeObjectURL(audioUrl);
      void destroyAsrModel();
    };
  });
</script>

<div class="tabs-lift tabs h-full w-full">
  <input type="radio" name="asr_tabs" class="tab px-10" aria-label={m.tools_audio_asr_use()} checked />
  <div class="tab-content border-base-300 bg-base-100 p-6">
    <div class="mx-auto flex max-w-4xl flex-col gap-6">
      <div class="card border border-base-300 bg-base-100 shadow-md">
        <div class="card-body gap-4">
          <div class="flex flex-col gap-4 lg:flex-row">
            <label class="form-control w-full">
              <div class="label">
                <span class="label-text">ASR 模型</span>
              </div>
              <select
                class="select-bordered select w-full"
                bind:value={selectedModel}
                disabled={loadingModel || recording || recognizing}
                on:change={() => void reloadCurrentModel()}>
                {#each modelOptions as model (model.value)}
                  <option value={model.value}>{model.label}</option>
                {/each}
              </select>
            </label>

            <label class="form-control w-full">
              <div class="label">
                <span class="label-text">录音设备</span>
              </div>
              <select
                class="select-bordered select w-full"
                bind:value={selectedId}
                disabled={recording || loadingModel || recognizing}>
                {#each devices as device (device.id)}
                  <option value={device.id}>
                    {device.name}{device.is_default ? "（默认）" : ""}
                  </option>
                {/each}
              </select>
            </label>
          </div>

          <div class="flex flex-wrap items-center gap-3">
            <div class="badge badge-outline p-4">
              {#if loadingModel}
                正在加载 {currentModelLabel()}
              {:else if asrStatus.isLoaded}
                当前模型已就绪：{currentModelLabel()}
              {:else}
                模型未加载
              {/if}
            </div>

            <button
              class="btn btn-secondary"
              on:click={() => void reloadCurrentModel()}
              disabled={loadingModel || recording || recognizing}>
              重新加载模型
            </button>
          </div>

          <div class="flex gap-3">
            {#if !recording}
              <button
                class="btn btn-primary"
                on:click={() => void startRecording()}
                disabled={!selectedId || !asrStatus.isLoaded || loadingModel || recognizing}>
                开始录音
              </button>
            {:else}
              <button class="btn btn-error" on:click={() => void stopRecording()} disabled={recognizing}>
                停止录音并识别
              </button>
            {/if}

            {#if recognizing}
              <span class="loading loading-md loading-spinner"></span>
            {/if}
          </div>

          {#if error}
            <div class="alert alert-error">
              <span>{error}</span>
            </div>
          {/if}

          {#if audioUrl}
            <div class="space-y-2">
              <div class="text-sm font-medium">录音回放</div>
              <audio class="w-full" controls src={audioUrl}></audio>
            </div>
          {/if}

          <div class="space-y-2">
            <div class="text-sm font-medium">识别结果</div>
            <textarea
              class="textarea-bordered textarea min-h-40 w-full"
              bind:value={recognitionText}
              readonly
              placeholder="完成录音后，这里会显示识别文本。"></textarea>
          </div>
        </div>
      </div>
    </div>
  </div>

  <input type="radio" name="asr_tabs" class="tab px-10" aria-label={m.tools_audio_asr_word_bank()} />
  <div class="tab-content border-base-300 bg-base-100 p-6">词库功能待补充</div>

  <input type="radio" name="asr_tabs" class="tab px-10" aria-label={m.tools_audio_asr_settings()} />
  <div class="tab-content border-base-300 bg-base-100 p-6">当前页面设置待补充</div>

  <a class="btn ml-auto px-10 btn-error" href={resolve("/tools")}>{m.msg_back()}</a>
</div>
