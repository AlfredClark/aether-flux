export type RecorderPageTab = "record" | "details" | "files";

export type RecordSupportedBufferSize = {
  kind: "range" | "unknown";
  min: number | null;
  max: number | null;
};

export type RecordSupportedInputConfig = {
  channels: number;
  minSampleRate: number;
  maxSampleRate: number;
  supportedSampleRates: number[];
  sampleFormat: string;
  bufferSize: RecordSupportedBufferSize;
};

export type RecordInputDeviceInfo = {
  id: string;
  name: string;
  manufacturer: string | null;
  extended: string;
  isDefault: boolean;
  defaultConfig: RecordSupportedInputConfig | null;
  supportedConfigs: RecordSupportedInputConfig[];
  configError: string | null;
};

export type RecordInputConfig = {
  channels: number;
  sampleRate: number;
  sampleFormat: string;
  bufferSize: number | null;
};

export type RecordWavConfig = {
  channels: number;
  sampleRate: number;
  bitsPerSample: number;
  sampleFormat: "int" | "float";
};

export type StopRecordResult = {
  filePath: string;
  deviceId: string;
  deviceName: string;
  input: RecordInputConfig;
  wav: RecordWavConfig;
  durationMs: number;
  writtenFrames: number;
};

export type RecordedFile = StopRecordResult & {
  id: string;
  name: string;
  createdAt: number;
};

export type BufferSizeOption = {
  value: string;
  label: string;
};
