<script lang="ts">
  import { browser } from "$app/environment";
  import { resolve } from "$app/paths";
  import { invoke } from "@tauri-apps/api/core";
  import { appCacheDir, join } from "@tauri-apps/api/path";
  import { mkdir, readFile, remove } from "@tauri-apps/plugin-fs";
  import { openPath } from "@tauri-apps/plugin-opener";
  import { onMount } from "svelte";
  import DetailsTab from "$lib/components/tabs/recorder/DetailsTab.svelte";
  import FilesTab from "$lib/components/tabs/recorder/FilesTab.svelte";
  import { fileNameFromPath } from "$lib/components/tabs/recorder/format";
  import RecordTab from "$lib/components/tabs/recorder/RecordTab.svelte";
  import { m } from "$lib/i18n/paraglide/messages";
  import type {
    BufferSizeOption,
    RecordedFile,
    RecorderPageTab,
    RecordInputDeviceInfo,
    RecordSupportedInputConfig,
    StopRecordResult
  } from "$lib/components/tabs/recorder/types";
  import { SvelteSet } from "svelte/reactivity";

  const ACTIVE_TAB_STORAGE_KEY = "tools.audio.recorder.active-tab";
  const RECORDED_FILES_STORAGE_KEY = "tools.audio.recorder.files";
  const fallbackCommonSampleRates = [8000, 11025, 16000, 22050, 32000, 44100, 48000, 88200, 96000, 176400, 192000];
  const commonInputSampleRates = [48000, 44100, 16000];
  const commonBufferSizes = [64, 128, 256, 512, 1024, 2048, 4096, 8192];

  let activeTab: RecorderPageTab = $state("record");
  let devices: RecordInputDeviceInfo[] = $state([]);
  let selectedDeviceId = $state("");
  let inputChannels = $state(1);
  let inputSampleRate = $state(48000);
  let inputSampleFormat = $state("i16");
  let inputBufferSize = $state("default");
  let wavChannels = $state(1);
  let wavSampleRate = $state(48000);
  let wavBitsPerSample = $state(16);
  let wavSampleFormat = $state<"int" | "float">("int");
  let loading = $state(false);
  let recording = $state(false);
  let error = $state("");
  let outputPath = $state("");
  let audioUrl = $state("");
  let selectedRecordingId = $state("");
  let lastResult: StopRecordResult | null = $state(null);
  let recordedFiles: RecordedFile[] = $state([]);
  let previousDeviceId = "";
  let uiStateRestored = false;

  let selectedDevice = $derived(devices.find((device) => device.id === selectedDeviceId) ?? null);
  let supportedConfigs = $derived(selectedDevice?.supportedConfigs ?? []);
  let recommendedConfigs = $derived(getRecommendedConfigs(selectedDevice));
  let selectedRecordedFile = $derived(recordedFiles.find((file) => file.id === selectedRecordingId) ?? null);
  let channelOptions = $derived(getChannelOptions(selectedDevice));
  let sampleFormatOptions = $derived(getSampleFormatOptions(selectedDevice, inputChannels));
  let sampleRateOptions = $derived(getSampleRateOptions(selectedDevice, inputChannels, inputSampleFormat));
  let bufferSizeOptions = $derived(getBufferSizeOptions(selectedDevice, inputChannels, inputSampleFormat, inputSampleRate));
  let wavChannelOptions = $derived(getWavChannelOptions(inputChannels));
  let wavSampleRateOptions = $derived(uniqueSorted([inputSampleRate, ...fallbackCommonSampleRates]));
  let wavBitDepthOptions = $derived(wavSampleFormat === "float" ? [32] : [8, 16, 24, 32]);

  onMount(() => {
    restoreUiState();
    void loadDevices();

    return () => {
      if (audioUrl) URL.revokeObjectURL(audioUrl);
      if (recording) void stopRecording();
    };
  });

  $effect(() => {
    if (selectedDeviceId && selectedDeviceId !== previousDeviceId) {
      previousDeviceId = selectedDeviceId;
      syncFromDevice(selectedDevice);
    }
  });

  $effect(() => {
    if (!selectedDevice) return;

    if (!channelOptions.includes(inputChannels)) {
      inputChannels = channelOptions[0] ?? 1;
    }
    if (!sampleFormatOptions.includes(inputSampleFormat)) {
      inputSampleFormat = sampleFormatOptions[0] ?? "i16";
    }
    if (!sampleRateOptions.includes(inputSampleRate)) {
      inputSampleRate = sampleRateOptions.includes(48000) ? 48000 : (sampleRateOptions[0] ?? 48000);
    }
    if (!bufferSizeOptions.some((option) => option.value === inputBufferSize)) {
      inputBufferSize = "default";
    }
    if (!wavChannelOptions.includes(wavChannels)) {
      wavChannels = Math.min(inputChannels, wavChannelOptions.at(-1) ?? inputChannels);
    }
    if (!wavBitDepthOptions.includes(wavBitsPerSample)) {
      wavBitsPerSample = wavBitDepthOptions[0] ?? 16;
    }
  });

  $effect(() => {
    if (browser && uiStateRestored) {
      localStorage.setItem(RECORDED_FILES_STORAGE_KEY, JSON.stringify(recordedFiles));
    }
  });

  function restoreUiState() {
    if (!browser) return;

    const storedTab = localStorage.getItem(ACTIVE_TAB_STORAGE_KEY);
    if (storedTab === "record" || storedTab === "details" || storedTab === "files") {
      activeTab = storedTab;
    }

    const storedFiles = localStorage.getItem(RECORDED_FILES_STORAGE_KEY);
    if (!storedFiles) {
      uiStateRestored = true;
      return;
    }

    try {
      const files = JSON.parse(storedFiles);
      if (Array.isArray(files)) {
        recordedFiles = files.filter(isRecordedFile);
        selectedRecordingId = recordedFiles[0]?.id ?? "";
      }
    } catch {
      recordedFiles = [];
    }

    uiStateRestored = true;
  }

  function isRecordedFile(value: unknown): value is RecordedFile {
    if (!value || typeof value !== "object") return false;
    const item = value as Partial<RecordedFile>;
    return (
      typeof item.id === "string" &&
      typeof item.name === "string" &&
      typeof item.filePath === "string" &&
      typeof item.deviceName === "string" &&
      typeof item.createdAt === "number"
    );
  }

  function setActiveTab(tab: RecorderPageTab) {
    activeTab = tab;
    if (browser) {
      localStorage.setItem(ACTIVE_TAB_STORAGE_KEY, tab);
    }
  }

  async function loadDevices() {
    loading = true;
    error = "";

    try {
      devices = await invoke<RecordInputDeviceInfo[]>("list_record_input_devices");
      selectedDeviceId = devices.find((device) => device.isDefault)?.id ?? devices[0]?.id ?? "";
      previousDeviceId = "";
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function syncFromDevice(device: RecordInputDeviceInfo | null) {
    const config = device?.supportedConfigs[0] ?? device?.defaultConfig;
    if (!config) return;

    applySupportedConfig(config, false);
    wavSampleFormat = "int";
    wavBitsPerSample = 16;
  }

  function applySupportedConfig(config: RecordSupportedInputConfig, switchToRecordTab = true) {
    inputChannels = config.channels;
    inputSampleFormat = config.sampleFormat;
    inputSampleRate = pickInitialSampleRate(config);
    inputBufferSize = "default";
    wavChannels = config.channels;
    wavSampleRate = inputSampleRate;
    wavSampleFormat = config.sampleFormat.startsWith("f") ? "float" : "int";
    wavBitsPerSample = wavBitDepthFromSampleFormat(config.sampleFormat);

    if (switchToRecordTab) {
      setActiveTab("record");
    }
  }

  function pickInitialSampleRate(config: RecordSupportedInputConfig) {
    const supportedRates = getConfigSampleRates(config);
    return (
      commonInputSampleRates.find((rate) => supportedRates.includes(rate)) ?? supportedRates.at(-1) ?? config.maxSampleRate
    );
  }

  function getChannelOptions(device: RecordInputDeviceInfo | null) {
    return uniqueSorted((device?.supportedConfigs ?? []).map((config) => config.channels));
  }

  function getSampleFormatOptions(device: RecordInputDeviceInfo | null, channels: number) {
    const formats = (device?.supportedConfigs ?? [])
      .filter((config) => config.channels === channels)
      .map((config) => config.sampleFormat);
    return Array.from(new Set(formats)).sort();
  }

  function getSampleRateOptions(device: RecordInputDeviceInfo | null, channels: number, sampleFormat: string) {
    const configs = matchingConfigs(device, channels, sampleFormat);
    const rates = configs.flatMap(getConfigSampleRates);
    return uniqueSorted(rates);
  }

  function getBufferSizeOptions(
    device: RecordInputDeviceInfo | null,
    channels: number,
    sampleFormat: string,
    sampleRate: number
  ): BufferSizeOption[] {
    const options: BufferSizeOption[] = [{ value: "default", label: m.tools_audio_recorder_buffer_default() }];
    const configs = matchingConfigs(device, channels, sampleFormat).filter((config) =>
      getConfigSampleRates(config).includes(sampleRate)
    );
    const rangeConfigs = configs.filter((config) => config.bufferSize.kind === "range");
    const sizes = uniqueSorted(
      rangeConfigs.flatMap((config) => {
        const min = config.bufferSize.min ?? 0;
        const max = config.bufferSize.max ?? 0;
        return [min, max, ...commonBufferSizes.filter((size) => size >= min && size <= max)];
      })
    );

    return [
      ...options,
      ...sizes.map((size) => ({
        value: String(size),
        label: m.tools_audio_recorder_buffer_frames({ frames: size })
      }))
    ];
  }

  function matchingConfigs(device: RecordInputDeviceInfo | null, channels: number, sampleFormat: string) {
    return (device?.supportedConfigs ?? []).filter(
      (config) => config.channels === channels && config.sampleFormat === sampleFormat
    );
  }

  function getRecommendedConfigs(device: RecordInputDeviceInfo | null) {
    const configs = device?.supportedConfigs ?? [];
    if (configs.length === 0) return [];

    const defaultConfig = device?.defaultConfig;
    const result: RecordSupportedInputConfig[] = [];
    const seen = new SvelteSet<string>();

    function add(config: RecordSupportedInputConfig | null | undefined) {
      if (!config) return;
      const rates = getConfigSampleRates(config);
      for (const rate of rates) {
        const singleRateConfig = withSingleSampleRate(config, rate);
        const key = configKey(singleRateConfig);
        if (seen.has(key)) continue;
        seen.add(key);
        result.push(singleRateConfig);
      }
    }

    add(defaultConfig);

    const highestConfig = configs.slice().sort((a, b) => scoreHighestConfig(b) - scoreHighestConfig(a))[0];
    if (highestConfig) {
      add(withSingleSampleRate(highestConfig, getConfigSampleRates(highestConfig).at(-1) ?? highestConfig.maxSampleRate));
    }

    getCommonRecommendedConfigs(configs).forEach(add);

    return result.sort(compareRecommendedConfig);
  }

  function configKey(config: RecordSupportedInputConfig) {
    return `${config.channels}-${config.sampleFormat}-${getConfigSampleRates(config)[0] ?? config.maxSampleRate}-${config.bufferSize.kind}-${config.bufferSize.min}-${config.bufferSize.max}`;
  }

  function scoreHighestConfig(config: RecordSupportedInputConfig) {
    let score = 0;
    score += config.channels * 1_000_000;
    score += (getConfigSampleRates(config).at(-1) ?? config.maxSampleRate) * 10;
    score += sampleFormatQuality(config.sampleFormat);
    return score;
  }

  function getCommonRecommendedConfigs(configs: RecordSupportedInputConfig[]) {
    const commonChannelPreference = [1, 2];
    const commonFormatPreference = ["i16", "f32", "u16"];
    const result: RecordSupportedInputConfig[] = [];
    const seenCombos = new SvelteSet<string>();

    for (const rate of commonInputSampleRates) {
      for (const channels of commonChannelPreference) {
        for (const sampleFormat of commonFormatPreference) {
          const config = configs
            .filter(
              (item) =>
                item.channels === channels && item.sampleFormat === sampleFormat && getConfigSampleRates(item).includes(rate)
            )
            .sort((a, b) => scoreHighestConfig(b) - scoreHighestConfig(a))[0];
          const comboKey = `${channels}-${sampleFormat}-${rate}`;
          if (!config || seenCombos.has(comboKey)) continue;
          seenCombos.add(comboKey);
          result.push({ ...config, supportedSampleRates: [rate] });
        }
      }
    }

    return result.slice(0, 6);
  }

  function compareRecommendedConfig(a: RecordSupportedInputConfig, b: RecordSupportedInputConfig) {
    const rateA = getConfigSampleRates(a)[0] ?? a.maxSampleRate;
    const rateB = getConfigSampleRates(b)[0] ?? b.maxSampleRate;
    return (
      a.channels - b.channels || rateB - rateA || sampleFormatQuality(b.sampleFormat) - sampleFormatQuality(a.sampleFormat)
    );
  }

  function sampleFormatQuality(sampleFormat: string) {
    if (sampleFormat === "f32") return 4;
    if (sampleFormat === "i32") return 3;
    if (sampleFormat === "i24") return 2;
    if (sampleFormat === "i16") return 1;
    return 0;
  }

  function getConfigSampleRates(config: RecordSupportedInputConfig) {
    return uniqueSorted(
      config.supportedSampleRates.length > 0 ? config.supportedSampleRates : [config.minSampleRate, config.maxSampleRate]
    );
  }

  function withSingleSampleRate(config: RecordSupportedInputConfig, sampleRate: number): RecordSupportedInputConfig {
    return {
      ...config,
      minSampleRate: sampleRate,
      maxSampleRate: sampleRate,
      supportedSampleRates: [sampleRate]
    };
  }

  function wavBitDepthFromSampleFormat(sampleFormat: string) {
    const bits = Number(sampleFormat.match(/\d+/)?.[0] ?? 16);
    if (sampleFormat.startsWith("f")) return 32;
    if (bits === 8 || bits === 16 || bits === 24 || bits === 32) return bits;
    return 32;
  }

  function getWavChannelOptions(inputChannelCount: number) {
    return Array.from({ length: Math.max(2, inputChannelCount) }, (_, index) => index + 1);
  }

  function uniqueSorted(values: number[]) {
    return Array.from(new Set(values.filter((value) => Number.isFinite(value) && value > 0))).sort((a, b) => a - b);
  }

  async function startRecording() {
    if (recording || !selectedDevice) return;
    error = "";
    lastResult = null;

    if (audioUrl) {
      URL.revokeObjectURL(audioUrl);
      audioUrl = "";
    }

    const dir = await appCacheDir();
    const recorderDir = await join(dir, "recorder");
    const path = await join(recorderDir, `manual-${Date.now()}.wav`);

    try {
      await invoke("start_record", {
        request: {
          deviceId: selectedDeviceId,
          outputPath: path,
          input: {
            channels: inputChannels,
            sampleRate: inputSampleRate,
            sampleFormat: inputSampleFormat,
            bufferSize: inputBufferSize === "default" ? null : Number(inputBufferSize)
          },
          wav: {
            channels: wavChannels,
            sampleRate: wavSampleRate,
            bitsPerSample: wavBitsPerSample,
            sampleFormat: wavSampleFormat
          }
        }
      });
      outputPath = path;
      recording = true;
    } catch (e) {
      error = String(e);
      recording = false;
    }
  }

  async function stopRecording() {
    if (!recording) return;
    error = "";

    try {
      const result = await invoke<StopRecordResult>("stop_record");
      const file = {
        ...result,
        id: `${result.filePath}-${Date.now()}`,
        name: fileNameFromPath(result.filePath),
        createdAt: Date.now()
      };

      recordedFiles = [file, ...recordedFiles.filter((item) => item.filePath !== result.filePath)];
      selectedRecordingId = file.id;
      lastResult = result;
      outputPath = result.filePath;
      await playRecordedFile(file);
    } catch (e) {
      error = String(e);
    } finally {
      recording = false;
    }
  }

  async function playRecordedFile(file: RecordedFile) {
    error = "";

    try {
      const bytes = await readFile(file.filePath);
      const blob = new Blob([bytes], { type: "audio/wav" });

      if (audioUrl) URL.revokeObjectURL(audioUrl);
      audioUrl = URL.createObjectURL(blob);
      outputPath = file.filePath;
      selectedRecordingId = file.id;
      lastResult = file;
      setActiveTab("files");
    } catch (e) {
      error = String(e);
    }
  }

  async function deleteRecordedFile(file: RecordedFile) {
    error = "";

    try {
      await remove(file.filePath);
    } catch (e) {
      error = String(e);
      return;
    }

    removeRecordedFileState(file);
  }

  async function clearRecordedFiles() {
    error = "";

    for (const file of uniqueRecordedFilesByPath(recordedFiles)) {
      try {
        await remove(file.filePath);
      } catch (e) {
        error = String(e);
        return;
      }
    }

    clearRecordedFileState();
  }

  async function openRecorderFolder() {
    error = "";

    try {
      const dir = await appCacheDir();
      const recorderDir = await join(dir, "recorder");
      await mkdir(recorderDir, { recursive: true });
      await openPath(recorderDir);
    } catch (e) {
      error = String(e);
    }
  }

  function uniqueRecordedFilesByPath(files: RecordedFile[]) {
    const seen = new SvelteSet<string>();
    return files.filter((file) => {
      if (seen.has(file.filePath)) return false;
      seen.add(file.filePath);
      return true;
    });
  }

  function removeRecordedFileState(file: RecordedFile) {
    recordedFiles = recordedFiles.filter((item) => item.id !== file.id);
    if (selectedRecordingId === file.id) {
      selectedRecordingId = recordedFiles[0]?.id ?? "";
      clearPlaybackState();
    }
  }

  function clearRecordedFileState() {
    recordedFiles = [];
    selectedRecordingId = "";
    clearPlaybackState();
  }

  function clearPlaybackState() {
    lastResult = null;
    outputPath = "";
    if (audioUrl) {
      URL.revokeObjectURL(audioUrl);
      audioUrl = "";
    }
  }
</script>

<div class="flex h-full min-h-0 w-full flex-col overflow-hidden">
  <div class="tabs-lift tabs flex w-full shrink-0 flex-wrap content-start overflow-hidden">
    <input
      type="radio"
      name="audio-recorder-tabs"
      class="tab h-auto px-8 py-3 text-sm"
      aria-label={m.tools_audio_recorder_tab_record()}
      checked={activeTab === "record"}
      onchange={() => setActiveTab("record")} />
    <input
      type="radio"
      name="audio-recorder-tabs"
      class="tab h-auto px-8 py-3 text-sm"
      aria-label={m.tools_audio_recorder_tab_details()}
      checked={activeTab === "details"}
      onchange={() => setActiveTab("details")} />
    <input
      type="radio"
      name="audio-recorder-tabs"
      class="tab h-auto px-8 py-3 text-sm"
      aria-label={m.tools_audio_recorder_tab_files()}
      checked={activeTab === "files"}
      onchange={() => setActiveTab("files")} />
    <a class="btn m-0 ml-auto shrink-0 px-10 text-sm btn-error" href={resolve("/tools")}>
      {m.action_back()}
    </a>
  </div>

  <div class="min-h-0 flex-1 overflow-hidden bg-base-100 p-4 sm:p-6">
    {#if activeTab === "record"}
      <RecordTab
        bind:selectedDeviceId
        bind:inputChannels
        bind:inputSampleRate
        bind:inputSampleFormat
        bind:inputBufferSize
        bind:wavChannels
        bind:wavSampleRate
        bind:wavBitsPerSample
        bind:wavSampleFormat
        {error}
        {devices}
        {selectedDevice}
        supportedConfigCount={supportedConfigs.length}
        {channelOptions}
        {sampleFormatOptions}
        {sampleRateOptions}
        {bufferSizeOptions}
        {wavChannelOptions}
        {wavSampleRateOptions}
        {wavBitDepthOptions}
        {loading}
        {recording}
        {outputPath}
        {lastResult}
        {loadDevices}
        {startRecording}
        {stopRecording} />
    {:else if activeTab === "details"}
      <DetailsTab
        bind:selectedDeviceId
        {error}
        {devices}
        {selectedDevice}
        {supportedConfigs}
        {recommendedConfigs}
        {loading}
        {recording}
        {applySupportedConfig} />
    {:else}
      <FilesTab
        bind:selectedRecordingId
        {error}
        {recordedFiles}
        {selectedRecordedFile}
        {audioUrl}
        {lastResult}
        {playRecordedFile}
        {deleteRecordedFile}
        {clearRecordedFiles}
        {openRecorderFolder} />
    {/if}
  </div>
</div>
