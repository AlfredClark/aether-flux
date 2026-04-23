<script lang="ts">
  import { m } from "$lib/i18n/paraglide/messages";
  import { formatSampleFormat } from "./format";
  import type { RecordInputDeviceInfo, RecordSupportedInputConfig } from "./types";

  type Props = {
    error: string;
    devices: RecordInputDeviceInfo[];
    selectedDeviceId: string;
    selectedDevice: RecordInputDeviceInfo | null;
    supportedConfigs: RecordSupportedInputConfig[];
    recommendedConfigs: RecordSupportedInputConfig[];
    loading: boolean;
    recording: boolean;
    applySupportedConfig: (config: RecordSupportedInputConfig) => void;
  };

  let {
    error,
    devices,
    selectedDeviceId = $bindable(),
    selectedDevice,
    supportedConfigs,
    recommendedConfigs,
    loading,
    recording,
    applySupportedConfig
  }: Props = $props();

  function formatSupportedSampleRate(config: RecordSupportedInputConfig) {
    return config.supportedSampleRates[0] ?? config.maxSampleRate;
  }

  function formatSampleBitDepth(format: string) {
    const match = format.match(/\d+/);
    return match ? m.tools_audio_recorder_bit_depth_value({ bits: match[0] }) : m.msg_unknown();
  }
</script>

<div class="mx-auto flex h-full min-h-0 w-full max-w-7xl flex-col gap-4">
  {#if error}
    <div class="alert alert-error">
      <span>{error}</span>
    </div>
  {/if}

  <div class="card min-h-0 flex-1 border border-base-300 bg-base-100 shadow-md">
    <div class="card-body min-h-0 gap-4 overflow-hidden">
      <div class="flex flex-wrap items-start justify-between gap-3">
        <div>
          <div class="text-base font-semibold">{m.tools_audio_recorder_details_title()}</div>
          <div class="text-xs text-base-content/60">{m.tools_audio_recorder_details_description()}</div>
        </div>
        <div class="form-control w-full max-w-md">
          <div class="label"><span class="label-text">{m.tools_audio_recorder_details_device_select()}</span></div>
          <select class="select-bordered select w-full" bind:value={selectedDeviceId} disabled={recording || loading}>
            {#each devices as device (device.id)}
              <option value={device.id}>
                {device.name}{device.isDefault ? ` (${m.tools_audio_recorder_device_default()})` : ""}
              </option>
            {/each}
          </select>
        </div>
      </div>

      {#if selectedDevice}
        <div class="grid gap-3 rounded-xl border border-base-300 bg-base-200/60 p-4 md:grid-cols-3">
          <div>
            <div class="text-xs text-base-content/50">{m.tools_audio_recorder_details_device_name()}</div>
            <div class="mt-1 font-medium">{selectedDevice.name}</div>
          </div>
          <div>
            <div class="text-xs text-base-content/50">{m.tools_audio_recorder_details_device_manufacturer()}</div>
            <div class="mt-1 font-medium">{selectedDevice.manufacturer ?? m.tools_audio_recorder_not_provided()}</div>
          </div>
          <div>
            <div class="text-xs text-base-content/50">{m.tools_audio_recorder_details_recommended_total()}</div>
            <div class="mt-1 font-medium">
              {m.tools_audio_recorder_details_config_count({
                recommended: recommendedConfigs.length,
                total: supportedConfigs.length
              })}
            </div>
          </div>
          <div class="md:col-span-3">
            <div class="text-xs text-base-content/50">{m.tools_audio_recorder_details_device_extended()}</div>
            <div class="mt-1 text-sm text-base-content/70">
              {selectedDevice.extended || m.tools_audio_recorder_no_extended_description()}
            </div>
          </div>
        </div>
      {/if}

      <div class="min-h-0 flex-1 overflow-auto rounded-box border border-base-300">
        <table class="table table-sm">
          <thead>
            <tr>
              <th class="text-center">{m.tools_audio_recorder_details_channels()}</th>
              <th class="text-center">{m.tools_audio_recorder_details_sample_rate()}</th>
              <th class="text-center">{m.tools_audio_recorder_details_sample_format()}</th>
              <th class="text-center">{m.tools_audio_recorder_details_bit_depth()}</th>
              <th class="text-center">{m.tools_audio_recorder_details_action()}</th>
            </tr>
          </thead>
          <tbody>
            {#each recommendedConfigs as config, index (`${config.channels}-${config.sampleFormat}-${formatSupportedSampleRate(config)}-${index}`)}
              <tr>
                <td class="text-center">{m.tools_audio_recorder_channel_value({ channels: config.channels })}</td>
                <td class="text-center"
                  >{m.tools_audio_recorder_sample_rate_value({ rate: formatSupportedSampleRate(config) })}</td>
                <td class="text-center">{formatSampleFormat(config.sampleFormat)}</td>
                <td class="text-center">{formatSampleBitDepth(config.sampleFormat)}</td>
                <td class="text-center">
                  <button class="btn btn-xs btn-primary" type="button" onclick={() => applySupportedConfig(config)}>
                    {m.tools_audio_recorder_details_apply_action()}
                  </button>
                </td>
              </tr>
            {:else}
              <tr>
                <td colspan="5" class="py-6 text-center text-base-content/60">
                  {m.tools_audio_recorder_details_empty()}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>
  </div>
</div>
