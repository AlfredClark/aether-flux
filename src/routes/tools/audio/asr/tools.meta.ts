import type { Tools } from "../../+layout";
import { m } from "$lib/i18n/paraglide/messages";
import AsrIcon from "$lib/icons/tools/AsrIcon.svelte";

export default {
  name: "asr",
  path: "/tools/audio/asr",
  label: m.tools_audio_asr(),
  desc: m.tools_audio_asr_desc(),
  icon: AsrIcon
} as Tools;
