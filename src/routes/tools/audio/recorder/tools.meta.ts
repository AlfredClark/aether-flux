import type { Tools } from "../../+layout";
import { m } from "$lib/i18n/paraglide/messages";
import RecorderIcon from "$lib/icons/tools/RecorderIcon.svelte";

export default {
  name: "recorder",
  path: "/tools/audio/recorder",
  label: m.tools_audio_recorder(),
  desc: m.tools_audio_recorder_desc(),
  icon: RecorderIcon
} as Tools;
