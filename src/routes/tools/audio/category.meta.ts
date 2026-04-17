import type { Tools, ToolsCategory } from "../+layout";
import { m } from "$lib/i18n/paraglide/messages";

const tools = import.meta.glob("./*/tools.meta.ts", { eager: true, import: "default" });
const exportTools = Object.values(tools);

export default {
  name: "audio",
  label: m.tools_audio(),
  desc: m.tools_audio_desc(),
  tools: exportTools as Tools[]
} as ToolsCategory;
