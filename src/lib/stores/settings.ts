import { persistedStore, type StoresState } from "$lib/stores/stores";

export interface Settings {
  theme: StoresState<string>;
  font_family: StoresState<string>;
  font_zoom: StoresState<number>;
}

export default {
  theme: persistedStore<string>("settings:theme", "light"),
  font_family: persistedStore<string>("settings:font_family", "System"),
  font_zoom: persistedStore<number>("settings:font_zoom", 1)
} as Settings;
