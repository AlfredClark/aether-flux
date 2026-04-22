<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  type RecordingStatus = {
    is_recording: boolean;
    started_at_millis: number | null;
  };

  let elapsedLabel = $state("00:00");

  function formatElapsed(elapsedMilliseconds: number) {
    const totalSeconds = Math.max(0, Math.floor(elapsedMilliseconds / 1000));
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
  }

  onMount(() => {
    let timerId: number | undefined;

    const updateElapsed = async () => {
      try {
        const status = await invoke<RecordingStatus>("get_recording_status");
        if (!status.is_recording || !status.started_at_millis) {
          elapsedLabel = "00:00";
          return;
        }

        elapsedLabel = formatElapsed(Date.now() - status.started_at_millis);
      } catch {
        elapsedLabel = "00:00";
      }
    };

    void updateElapsed();
    timerId = window.setInterval(() => {
      void updateElapsed();
    }, 250);

    return () => {
      if (timerId) {
        window.clearInterval(timerId);
      }
    };
  });
</script>

<div class="status-shell">
  <div class="status-window">
    <div class="status-time">{elapsedLabel}</div>
  </div>
</div>

<style>
  :global(html),
  :global(body) {
    margin: 0;
    background: transparent;
    overflow: hidden;
  }

  .status-shell {
    display: flex;
    height: 100vh;
    width: 100vw;
    align-items: center;
    justify-content: center;
    background: transparent;
    pointer-events: none;
    box-sizing: border-box;
  }

  .status-window {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 160px;
    height: 40px;
    border: 1px solid rgb(255 255 255 / 0.14);
    border-radius: 999px;
    background: rgb(15 23 42 / 0.9);
    box-shadow:
      0 8px 18px rgb(15 23 42 / 0.16),
      inset 0 1px 0 rgb(255 255 255 / 0.08);
    box-sizing: border-box;
    color: rgb(248 250 252 / 0.96);
  }

  .status-time {
    color: rgb(248 250 252 / 0.96);
    font-family: "JetBrains Mono", "Fira Code", "IBM Plex Mono", "SFMono-Regular", "Roboto Mono", monospace;
    font-size: 1.2rem;
    font-weight: 700;
    line-height: 1;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.04em;
    white-space: nowrap;
  }
</style>
