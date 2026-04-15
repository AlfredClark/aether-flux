import { persistedStore, type StoresState } from "$lib/stores/stores";

export interface Settings {
  theme: StoresState<string>;
}

export default {
  theme: persistedStore<string>("settings:theme", "light")
} as Settings;
