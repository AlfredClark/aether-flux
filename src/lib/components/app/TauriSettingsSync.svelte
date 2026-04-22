<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import settings from "$lib/stores/settings";

  onMount(() => {
    let disposed = false;

    const syncHotkey = async () => {
      try {
        await invoke("configure_asr_hotkey", {
          enabled: settings.asr_hotkey_enabled.get(),
          shortcut: settings.asr_hotkey_shortcut.get()
        });
      } catch (error) {
        if (!disposed) {
          console.error("Failed to configure ASR hotkey", error);
        }
      }
    };

    const syncTrayMode = async (enabled: boolean) => {
      try {
        await invoke("set_tray_mode_enabled", { enabled });
      } catch (error) {
        if (!disposed) {
          console.error("Failed to configure tray mode", error);
        }
      }
    };

    void syncHotkey();
    void syncTrayMode(settings.tray_mode_enabled.get());

    const unsubscribeHotkeyEnabled = settings.asr_hotkey_enabled.subscribe(() => {
      void syncHotkey();
    });
    const unsubscribeHotkeyShortcut = settings.asr_hotkey_shortcut.subscribe(() => {
      void syncHotkey();
    });
    const unsubscribeTrayMode = settings.tray_mode_enabled.subscribe((value) => {
      void syncTrayMode(value);
    });

    return () => {
      disposed = true;
      unsubscribeHotkeyEnabled();
      unsubscribeHotkeyShortcut();
      unsubscribeTrayMode();
    };
  });
</script>
