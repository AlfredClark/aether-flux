import { persistedStore, type StoresState } from "$lib/stores/stores";

export interface Settings {
  theme: StoresState<string>;
  font_family: StoresState<string>;
  font_zoom: StoresState<number>;
  tray_mode_enabled: StoresState<boolean>;
  asr_hotkey_enabled: StoresState<boolean>;
  asr_hotkey_shortcut: StoresState<string>;
  asr_hotkey_trigger_mode: StoresState<"press_press" | "press_release">;
}

export default {
  theme: persistedStore<string>("settings:theme", "light"),
  font_family: persistedStore<string>("settings:font_family", "System"),
  font_zoom: persistedStore<number>("settings:font_zoom", 1),
  tray_mode_enabled: persistedStore<boolean>("settings:tray_mode_enabled", false),
  asr_hotkey_enabled: persistedStore<boolean>("settings:asr_hotkey_enabled", false),
  asr_hotkey_shortcut: persistedStore<string>("settings:asr_hotkey_shortcut", "Alt+M"),
  asr_hotkey_trigger_mode: persistedStore<"press_press" | "press_release">(
    "settings:asr_hotkey_trigger_mode",
    "press_release"
  )
} as Settings;
