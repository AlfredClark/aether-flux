<script lang="ts">
  import {
    estimatedPcmBytes,
    fileNameFromPath,
    formatBitRate,
    formatBytes,
    formatDate,
    formatDuration,
    formatSampleFormat
  } from "./format";
  import type { RecordedFile, StopRecordResult } from "./types";
  import { m } from "$lib/i18n/paraglide/messages";

  type Props = {
    error: string;
    recordedFiles: RecordedFile[];
    selectedRecordingId: string;
    selectedRecordedFile: RecordedFile | null;
    audioUrl: string;
    lastResult: StopRecordResult | null;
    playRecordedFile: (file: RecordedFile) => void | Promise<void>;
    deleteRecordedFile: (file: RecordedFile) => void | Promise<void>;
    clearRecordedFiles: () => void | Promise<void>;
    openRecorderFolder: () => void | Promise<void>;
  };

  let {
    error,
    recordedFiles,
    selectedRecordingId = $bindable(),
    selectedRecordedFile,
    audioUrl,
    lastResult,
    playRecordedFile,
    deleteRecordedFile,
    clearRecordedFiles,
    openRecorderFolder
  }: Props = $props();
</script>

<div class="mx-auto grid h-full min-h-0 w-full max-w-7xl gap-4 xl:grid-cols-[0.95fr_1.05fr]">
  {#if error}
    <div class="alert alert-error xl:col-span-2">
      <span>{error}</span>
    </div>
  {/if}

  <div class="card h-full min-h-0 border border-base-300 bg-base-100 shadow-md">
    <div class="card-body min-h-0 gap-4">
      <div class="flex flex-wrap items-center justify-between gap-3">
        <div>
          <div class="text-base font-semibold">{m.tools_audio_recorder_files_title()}</div>
          <div class="text-xs text-base-content/60">{m.tools_audio_recorder_files_description()}</div>
        </div>
        <div class="flex flex-wrap justify-end gap-2">
          <button class="btn btn-outline btn-sm" type="button" onclick={() => void openRecorderFolder()}>
            {m.tools_audio_recorder_files_open_folder()}
          </button>
          <button
            class="btn btn-outline btn-sm btn-error"
            type="button"
            onclick={() => void clearRecordedFiles()}
            disabled={recordedFiles.length === 0}>
            {m.tools_audio_recorder_files_clear_files()}
          </button>
        </div>
      </div>

      <div class="min-h-0 flex-1 overflow-auto rounded-box border border-base-300 p-3">
        {#if recordedFiles.length === 0}
          <div class="py-10 text-center text-sm text-base-content/60">{m.tools_audio_recorder_files_empty()}</div>
        {:else}
          <div class="space-y-2">
            {#each recordedFiles as file (file.id)}
              <button
                class={`w-full rounded-xl border p-4 text-left transition hover:border-primary/60 hover:bg-base-200 ${
                  selectedRecordingId === file.id ? "border-primary bg-primary/10" : "border-base-300 bg-base-100"
                }`}
                type="button"
                onclick={() => void playRecordedFile(file)}>
                <span class="flex items-start justify-between gap-3">
                  <span class="min-w-0">
                    <span class="truncate font-medium">{file.name}</span>
                    <span class="mt-1 text-xs text-base-content/60">{formatDate(file.createdAt)}</span>
                    <span class="mt-1 truncate font-mono text-xs text-base-content/50">{file.filePath}</span>
                  </span>
                  <span class="badge shrink-0 badge-outline">{formatDuration(file.durationMs)}</span>
                </span>
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  </div>

  <div class="card h-full min-h-0 border border-base-300 bg-base-100 shadow-md">
    <div class="card-body min-h-0 gap-4 overflow-hidden">
      <div class="text-base font-semibold">{m.tools_audio_recorder_files_playback_title()}</div>

      {#if audioUrl && lastResult}
        <audio class="w-full shrink-0" controls src={audioUrl}></audio>
        <div class="min-h-0 flex-1 overflow-auto rounded-xl border border-base-300 bg-base-200/60 p-4">
          <div class="grid gap-3 md:grid-cols-2">
            <div>
              <div class="text-xs text-base-content/50">{m.tools_audio_recorder_files_filename()}</div>
              <div class="mt-1 font-medium break-all">
                {selectedRecordedFile?.name ?? fileNameFromPath(lastResult.filePath)}
              </div>
            </div>
            <div>
              <div class="text-xs text-base-content/50">{m.tools_audio_recorder_files_created_at()}</div>
              <div class="mt-1 font-medium">
                {selectedRecordedFile
                  ? formatDate(selectedRecordedFile.createdAt)
                  : m.tools_audio_recorder_files_current_session()}
              </div>
            </div>
            <div>
              <div class="text-xs text-base-content/50">{m.tools_audio_recorder_files_record_id()}</div>
              <div class="mt-1 font-mono text-xs break-all">
                {selectedRecordedFile?.id ?? m.tools_audio_recorder_files_not_persisted()}
              </div>
            </div>
            <div>
              <div class="text-xs text-base-content/50">{m.tools_audio_recorder_files_device_id()}</div>
              <div class="mt-1 font-mono text-xs break-all">{lastResult.deviceId}</div>
            </div>
            <div>
              <div class="text-xs text-base-content/50">{m.tools_audio_recorder_files_device_name()}</div>
              <div class="mt-1 font-medium">{lastResult.deviceName}</div>
            </div>
            <div>
              <div class="text-xs text-base-content/50">{m.tools_audio_recorder_files_duration()}</div>
              <div class="mt-1 font-medium">{formatDuration(lastResult.durationMs)} ({lastResult.durationMs} ms)</div>
            </div>
            <div>
              <div class="text-xs text-base-content/50">{m.tools_audio_recorder_files_written_frames()}</div>
              <div class="mt-1 font-medium">{lastResult.writtenFrames}</div>
            </div>
            <div>
              <div class="text-xs text-base-content/50">{m.tools_audio_recorder_files_estimated_pcm_bytes()}</div>
              <div class="mt-1 font-medium">{formatBytes(estimatedPcmBytes(lastResult))}</div>
            </div>
            <div>
              <div class="text-xs text-base-content/50">{m.tools_audio_recorder_files_bitrate()}</div>
              <div class="mt-1 font-medium">{formatBitRate(lastResult)}</div>
            </div>
            <div>
              <div class="text-xs text-base-content/50">{m.tools_audio_recorder_files_input_channels()}</div>
              <div class="mt-1 font-medium">
                {m.tools_audio_recorder_channel_value({ channels: lastResult.input.channels })}
              </div>
            </div>
            <div>
              <div class="text-xs text-base-content/50">{m.tools_audio_recorder_files_input_sample_rate()}</div>
              <div class="mt-1 font-medium">
                {m.tools_audio_recorder_sample_rate_value({ rate: lastResult.input.sampleRate })}
              </div>
            </div>
            <div>
              <div class="text-xs text-base-content/50">{m.tools_audio_recorder_files_input_sample_format()}</div>
              <div class="mt-1 font-medium">{formatSampleFormat(lastResult.input.sampleFormat)}</div>
            </div>
            <div>
              <div class="text-xs text-base-content/50">{m.tools_audio_recorder_files_input_buffer()}</div>
              <div class="mt-1 font-medium">
                {lastResult.input.bufferSize === null
                  ? m.tools_audio_recorder_buffer_default()
                  : m.tools_audio_recorder_buffer_frames({ frames: lastResult.input.bufferSize })}
              </div>
            </div>
            <div>
              <div class="text-xs text-base-content/50">{m.tools_audio_recorder_files_wav_channels()}</div>
              <div class="mt-1 font-medium">{m.tools_audio_recorder_channel_value({ channels: lastResult.wav.channels })}</div>
            </div>
            <div>
              <div class="text-xs text-base-content/50">{m.tools_audio_recorder_files_wav_sample_rate()}</div>
              <div class="mt-1 font-medium">
                {m.tools_audio_recorder_sample_rate_value({ rate: lastResult.wav.sampleRate })}
              </div>
            </div>
            <div>
              <div class="text-xs text-base-content/50">{m.tools_audio_recorder_files_wav_bit_depth()}</div>
              <div class="mt-1 font-medium">
                {m.tools_audio_recorder_bit_depth_value({ bits: lastResult.wav.bitsPerSample })}
              </div>
            </div>
            <div>
              <div class="text-xs text-base-content/50">{m.tools_audio_recorder_files_wav_sample_format()}</div>
              <div class="mt-1 font-medium">
                {lastResult.wav.sampleFormat === "float"
                  ? m.tools_audio_recorder_wav_format_float()
                  : m.tools_audio_recorder_wav_format_int()}
              </div>
            </div>
            <div class="md:col-span-2">
              <div class="text-xs text-base-content/50">{m.tools_audio_recorder_files_full_path()}</div>
              <div class="mt-1 font-mono text-xs break-all text-base-content/70">{lastResult.filePath}</div>
            </div>
          </div>
        </div>

        {#if selectedRecordingId}
          <button
            class="btn shrink-0 btn-outline btn-error"
            type="button"
            onclick={() => {
              const file = recordedFiles.find((item) => item.id === selectedRecordingId);
              if (file) void deleteRecordedFile(file);
            }}>
            {m.tools_audio_recorder_files_delete_file()}
          </button>
        {/if}
      {:else}
        <div class="rounded-xl border border-dashed border-base-300 p-10 text-center text-sm text-base-content/60">
          {m.tools_audio_recorder_files_select_hint()}
        </div>
      {/if}
    </div>
  </div>
</div>
