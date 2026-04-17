<script lang="ts">
  import { checkPermission, requestPermission, startRecording, stopRecording } from "tauri-plugin-audio-recorder-api";
  import { appCacheDir, join } from "@tauri-apps/api/path";
  import { readFile } from "@tauri-apps/plugin-fs";

  let recording = false;
  let audioUrl = "";
  let error = "";

  async function ensurePermission() {
    const status = await checkPermission();
    if (status.granted) return true;
    const requested = await requestPermission();
    return requested.granted;
  }

  async function startTest() {
    error = "";
    audioUrl = "";

    const granted = await ensurePermission();
    if (!granted) {
      error = "未获得麦克风权限";
      return;
    }

    const dir = await appCacheDir();
    const outputPath = await join(dir, `mic-test-${Date.now()}`);

    await startRecording({
      outputPath,
      quality: "medium",
      maxDuration: 0
    });

    recording = true;
  }

  async function stopTest() {
    try {
      const result = await stopRecording();

      // 关键：直接读取本地文件字节，不走 convertFileSrc
      const bytes = await readFile(result.filePath);
      const blob = new Blob([bytes], { type: "audio/wav" });

      // 释放旧 URL，避免内存泄漏
      if (audioUrl) URL.revokeObjectURL(audioUrl);
      audioUrl = URL.createObjectURL(blob);

      recording = false;
    } catch (e) {
      error = e instanceof Error ? e.message : "停止录音失败";
      console.error(e);
      recording = false;
    }
  }
</script>

<div class="card max-w-md border border-base-300 bg-base-100 shadow-md">
  <div class="card-body gap-4">
    <h2 class="card-title">麦克风录音测试</h2>

    <div class="flex gap-2">
      {#if !recording}
        <button class="btn btn-primary" on:click={startTest}> 开始录音 </button>
      {:else}
        <button class="btn btn-error" on:click={stopTest}> 停止录音 </button>
      {/if}
    </div>

    {#if error}
      <div class="alert alert-error">
        <span>{error}</span>
      </div>
    {/if}

    {#if audioUrl}
      <audio controls src={audioUrl}></audio>
    {/if}
  </div>
</div>
