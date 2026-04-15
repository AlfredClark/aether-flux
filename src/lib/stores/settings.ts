import { persistedStore, type StoresState } from "$lib/stores/stores";

export interface Settings {
  theme: StoresState<string>;
  font_family: StoresState<string>;
}

export default {
  theme: persistedStore<string>("settings:theme", "light"),
  font_family: persistedStore<string>("settings:font_family", "System")
} as Settings;
