import type { StopRecordResult } from "./types";

export function formatSampleFormat(format: string) {
  return format.toUpperCase();
}

export function formatDuration(ms: number) {
  const seconds = ms / 1000;
  return `${seconds.toFixed(seconds >= 10 ? 1 : 2)}s`;
}

export function formatDate(timestamp: number) {
  return new Date(timestamp).toLocaleString();
}

export function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / 1024 / 1024).toFixed(2)} MiB`;
}

export function estimatedPcmBytes(file: StopRecordResult) {
  return Math.round((file.writtenFrames * file.wav.channels * file.wav.bitsPerSample) / 8);
}

export function formatBitRate(file: StopRecordResult) {
  const bitsPerSecond = file.wav.sampleRate * file.wav.channels * file.wav.bitsPerSample;
  return `${Math.round(bitsPerSecond / 1000)} kbps`;
}

export function fileNameFromPath(path: string) {
  return path.split(/[\\/]/).pop() || path;
}
