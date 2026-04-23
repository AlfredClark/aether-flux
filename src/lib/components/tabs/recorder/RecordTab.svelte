<script lang="ts">
  import { formatDuration, formatSampleFormat } from "./format";
  import type { BufferSizeOption, RecordInputDeviceInfo, StopRecordResult } from "./types";
  import { m } from "$lib/i18n/paraglide/messages";

  type Props = {
    error: string;
    devices: RecordInputDeviceInfo[];
    selectedDeviceId: string;
    selectedDevice: RecordInputDeviceInfo | null;
    supportedConfigCount: number;
    channelOptions: number[];
    sampleFormatOptions: string[];
    sampleRateOptions: number[];
    bufferSizeOptions: BufferSizeOption[];
    wavChannelOptions: number[];
    wavSampleRateOptions: number[];
    wavBitDepthOptions: number[];
    inputChannels: number;
    inputSampleRate: number;
    inputSampleFormat: string;
    inputBufferSize: string;
    wavChannels: number;
    wavSampleRate: number;
    wavBitsPerSample: number;
    wavSampleFormat: "int" | "float";
    loading: boolean;
    recording: boolean;
    outputPath: string;
    lastResult: StopRecordResult | null;
    loadDevices: () => void | Promise<void>;
    startRecording: () => void | Promise<void>;
    stopRecording: () => void | Promise<void>;
  };

  let {
    error,
    devices,
    selectedDeviceId = $bindable(),
    selectedDevice,
    supportedConfigCount,
    channelOptions,
    sampleFormatOptions,
    sampleRateOptions,
    bufferSizeOptions,
    wavChannelOptions,
    wavSampleRateOptions,
    wavBitDepthOptions,
    inputChannels = $bindable(),
    inputSampleRate = $bindable(),
    inputSampleFormat = $bindable(),
    inputBufferSize = $bindable(),
    wavChannels = $bindable(),
    wavSampleRate = $bindable(),
    wavBitsPerSample = $bindable(),
    wavSampleFormat = $bindable(),
    loading,
    recording,
    outputPath,
    lastResult,
    loadDevices,
    startRecording,
    stopRecording
  }: Props = $props();
</script>

<div class="mx-auto flex h-full min-h-0 w-full max-w-7xl flex-col gap-4">
  {#if error}
    <div class="alert alert-error">
      <span>{error}</span>
    </div>
  {/if}

  <div class="grid min-h-0 flex-1 content-start gap-4 pr-1">
    <div class="grid gap-4 xl:grid-cols-[1.15fr_0.85fr]">
      <div class="card border border-base-300 bg-base-100 shadow-md">
        <div class="card-body gap-5">
          <div class="flex flex-wrap items-center justify-between gap-3">
            <div>
              <div class="text-base font-semibold">{m.tools_audio_recorder_record_device_title()}</div>
              <div class="text-xs text-base-content/60">{m.tools_audio_recorder_record_device_description()}</div>
            </div>
            <button
              class="btn btn-sm btn-secondary"
              type="button"
              onclick={() => void loadDevices()}
              disabled={loading || recording}>
              {loading ? m.tools_audio_recorder_record_refreshing_devices() : m.tools_audio_recorder_record_refresh_devices()}
            </button>
          </div>

          <div class="form-control">
            <div class="label">
              <span class="label-text">{m.tools_audio_recorder_record_device_label()}</span>
            </div>
            <select class="select-bordered select w-full" bind:value={selectedDeviceId} disabled={recording || loading}>
              {#each devices as device (device.id)}
                <option value={device.id}>
                  {device.name}{device.isDefault ? ` (${m.tools_audio_recorder_device_default()})` : ""}
                </option>
              {/each}
            </select>
          </div>

          {#if selectedDevice}
            <div class="grid gap-3 rounded-xl border border-base-300 bg-base-200/60 p-4 md:grid-cols-2">
              <div>
                <div class="text-xs text-base-content/50">{m.tools_audio_recorder_details_device_name()}</div>
                <div class="mt-1 font-medium">{selectedDevice.name}</div>
              </div>
              <div>
                <div class="text-xs text-base-content/50">{m.tools_audio_recorder_details_device_manufacturer()}</div>
                <div class="mt-1 font-medium">{selectedDevice.manufacturer ?? m.tools_audio_recorder_not_provided()}</div>
              </div>
              <div class="md:col-span-2">
                <div class="text-xs text-base-content/50">{m.tools_audio_recorder_details_device_extended()}</div>
                <div class="mt-1 text-sm text-base-content/70">
                  {selectedDevice.extended || m.tools_audio_recorder_no_extended_description()}
                </div>
              </div>
            </div>

            {#if selectedDevice.configError}
              <div class="alert py-2 text-sm alert-warning">
                <span>{selectedDevice.configError}</span>
              </div>
            {/if}
          {:else}
            <div class="rounded-xl border border-dashed border-base-300 p-6 text-sm text-base-content/60">
              {m.tools_audio_recorder_record_no_input_devices()}
            </div>
          {/if}
        </div>
      </div>

      <div class="card border border-base-300 bg-base-100 shadow-md">
        <div class="card-body gap-4">
          <div class="text-base font-semibold">{m.tools_audio_recorder_record_control_title()}</div>
          <div class="rounded-2xl border border-base-300 bg-linear-to-br from-base-200 to-base-100 p-5">
            <div class="flex items-center gap-3">
              <div class={`h-3 w-3 rounded-full ${recording ? "animate-pulse bg-error" : "bg-base-content/30"}`}></div>
              <div>
                <div class="font-medium">
                  {recording ? m.tools_audio_recorder_record_status_recording() : m.tools_audio_recorder_record_status_idle()}
                </div>
                <div class="text-xs text-base-content/60">
                  {outputPath || m.tools_audio_recorder_record_output_placeholder()}
                </div>
              </div>
            </div>
            <div class="mt-5 flex flex-wrap gap-3">
              <button
                class="btn btn-primary"
                type="button"
                onclick={() => void startRecording()}
                disabled={recording || !selectedDevice || supportedConfigCount === 0}>
                {m.tools_audio_recorder_record_start()}
              </button>
              <button class="btn btn-error" type="button" onclick={() => void stopRecording()} disabled={!recording}>
                {m.tools_audio_recorder_record_stop()}
              </button>
            </div>
          </div>

          {#if lastResult}
            <div class="stats stats-horizontal border border-base-300 bg-base-100 shadow-sm">
              <div class="stat">
                <div class="stat-title">{m.tools_audio_recorder_files_duration()}</div>
                <div class="stat-value text-2xl">{formatDuration(lastResult.durationMs)}</div>
              </div>
              <div class="stat">
                <div class="stat-title">{m.tools_audio_recorder_files_written_frames()}</div>
                <div class="stat-value text-2xl">{lastResult.writtenFrames}</div>
              </div>
            </div>
          {/if}
        </div>
      </div>
    </div>

    <div class="grid min-h-0 gap-4 xl:grid-cols-2">
      <div class="card border border-base-300 bg-base-100 shadow-md">
        <div class="card-body gap-4">
          <div>
            <div class="text-base font-semibold">{m.tools_audio_recorder_record_input_config_title()}</div>
            <div class="text-xs text-base-content/60">{m.tools_audio_recorder_record_input_config_description()}</div>
          </div>

          <div class="grid grid-cols-[repeat(auto-fit,minmax(min(14rem,100%),1fr))] gap-4">
            <label class="form-control min-w-0">
              <span class="label"><span class="label-text">{m.tools_audio_recorder_details_channels()}</span></span>
              <select class="select-bordered select" bind:value={inputChannels} disabled={recording}>
                {#each channelOptions as channels, i (i)}
                  <option value={channels}>{m.tools_audio_recorder_channel_value({ channels })}</option>
                {/each}
              </select>
            </label>

            <label class="form-control min-w-0">
              <span class="label"><span class="label-text">{m.tools_audio_recorder_files_input_sample_format()}</span></span>
              <select class="select-bordered select" bind:value={inputSampleFormat} disabled={recording}>
                {#each sampleFormatOptions as format, i (i)}
                  <option value={format}>{formatSampleFormat(format)}</option>
                {/each}
              </select>
            </label>

            <label class="form-control min-w-0">
              <span class="label"><span class="label-text">{m.tools_audio_recorder_details_sample_rate()}</span></span>
              <select class="select-bordered select" bind:value={inputSampleRate} disabled={recording}>
                {#each sampleRateOptions as rate, i (i)}
                  <option value={rate}>{m.tools_audio_recorder_sample_rate_value({ rate })}</option>
                {/each}
              </select>
            </label>

            <label class="form-control min-w-0">
              <span class="label"><span class="label-text">{m.tools_audio_recorder_record_buffer_size()}</span></span>
              <select class="select-bordered select" bind:value={inputBufferSize} disabled={recording}>
                {#each bufferSizeOptions as option, i (i)}
                  <option value={option.value}>{option.label}</option>
                {/each}
              </select>
            </label>
          </div>
        </div>
      </div>

      <div class="card border border-base-300 bg-base-100 shadow-md">
        <div class="card-body gap-4">
          <div>
            <div class="text-base font-semibold">{m.tools_audio_recorder_record_wav_config_title()}</div>
            <div class="text-xs text-base-content/60">{m.tools_audio_recorder_record_wav_config_description()}</div>
          </div>

          <div class="grid grid-cols-[repeat(auto-fit,minmax(min(14rem,100%),1fr))] gap-4">
            <label class="form-control min-w-0">
              <span class="label"><span class="label-text">{m.tools_audio_recorder_files_wav_channels()}</span></span>
              <select class="select-bordered select" bind:value={wavChannels} disabled={recording}>
                {#each wavChannelOptions as channels, i (i)}
                  <option value={channels}>{m.tools_audio_recorder_channel_value({ channels })}</option>
                {/each}
              </select>
            </label>

            <label class="form-control min-w-0">
              <span class="label"><span class="label-text">{m.tools_audio_recorder_files_wav_sample_rate()}</span></span>
              <select class="select-bordered select" bind:value={wavSampleRate} disabled={recording}>
                {#each wavSampleRateOptions as rate, i (i)}
                  <option value={rate}>{m.tools_audio_recorder_sample_rate_value({ rate })}</option>
                {/each}
              </select>
            </label>

            <label class="form-control min-w-0">
              <span class="label"><span class="label-text">{m.tools_audio_recorder_files_wav_sample_format()}</span></span>
              <select class="select-bordered select" bind:value={wavSampleFormat} disabled={recording}>
                <option value="int">{m.tools_audio_recorder_wav_format_int()}</option>
                <option value="float">{m.tools_audio_recorder_wav_format_float()}</option>
              </select>
            </label>

            <label class="form-control min-w-0">
              <span class="label"><span class="label-text">{m.tools_audio_recorder_files_wav_bit_depth()}</span></span>
              <select class="select-bordered select" bind:value={wavBitsPerSample} disabled={recording}>
                {#each wavBitDepthOptions as bits, i (i)}
                  <option value={bits}>{m.tools_audio_recorder_bit_depth_value({ bits })}</option>
                {/each}
              </select>
            </label>
          </div>
        </div>
      </div>
    </div>
  </div>
</div>
