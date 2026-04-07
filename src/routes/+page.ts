import { isTauri } from "@tauri-apps/api/core";

if (!isTauri()) {
  window.close();
}
