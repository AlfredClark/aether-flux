<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { appCacheDir, join } from "@tauri-apps/api/path";
  import { readFile } from "@tauri-apps/plugin-fs";

  type InputDeviceInfo = {
    id: string;
    name: string;
    is_default: boolean;
  };

  let devices: InputDeviceInfo[] = [];
  let selectedId = "";
  let recording = false;
  let audioUrl = "";
  let error = "";

  async function loadDevices() {
    error = "";
    devices = await invoke<InputDeviceInfo[]>("list_input_devices");
    selectedId = devices.find((d) => d.is_default)?.id ?? devices[0]?.id ?? "";
    console.log(devices);
  }

  async function startRecording() {
    error = "";
    audioUrl = "";

    const dir = await appCacheDir();
    const outputPath = await join(dir, `custom-mic-${Date.now()}`);

    try {
      await invoke("start_recording", {
        deviceId: selectedId,
        outputPath,
        sampleRate: 16000
      });
      recording = true;
    } catch (e) {
      error = String(e);
    }
  }

  async function stopRecording() {
    error = "";

    try {
      const result = await invoke<{ file_path: string }>("stop_recording");

      // Linux/Tauri 下本地音频资源更稳的回放方式：直接读文件转 Blob
      const bytes = await readFile(result.file_path);
      const blob = new Blob([bytes], { type: "audio/wav" });

      if (audioUrl) URL.revokeObjectURL(audioUrl);
      audioUrl = URL.createObjectURL(blob);

      recording = false;
    } catch (e) {
      error = String(e);
      recording = false;
    }
  }

  loadDevices();
</script>

<div class="card max-w-lg border border-base-300 bg-base-100 shadow-md">
  <div class="card-body gap-4">
    <h2 class="card-title">自定义麦克风录音测试</h2>

    <label class="form-control w-full">
      <div class="label">
        <span class="label-text">录音设备</span>
      </div>
      <select class="select-bordered select w-full" bind:value={selectedId} disabled={recording}>
        {#each devices as device (device.id)}
          <option value={device.id}>
            {device.name}{device.is_default ? "（默认）" : ""}
          </option>
        {/each}
      </select>
    </label>

    <div class="flex gap-2">
      {#if !recording}
        <button class="btn btn-primary" on:click={startRecording} disabled={!selectedId}> 开始录音 </button>
      {:else}
        <button class="btn btn-error" on:click={stopRecording}> 停止录音 </button>
      {/if}
    </div>

    {#if error}
      <div class="alert alert-error">
        <span>{error}</span>
      </div>
    {/if}

    {#if audioUrl}
      <audio class="w-full" controls src={audioUrl}></audio>
    {/if}
  </div>
</div>
