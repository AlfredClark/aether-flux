<script lang="ts">
  import { m } from "$lib/i18n/paraglide/messages";
  import FontSelector from "$lib/components/selectors/FontSelector.svelte";
  import ThemeSelector from "$lib/components/selectors/ThemeSelector.svelte";
  import LanguageSelector from "$lib/components/selectors/LanguageSelector.svelte";
  import FontZoomSelector from "$lib/components/selectors/FontZoomSelector.svelte";
  import { openModal } from "$lib/stores/modal";
  import settings from "$lib/stores/settings";

  let trayModeEnabled = settings.tray_mode_enabled.get();

  function toggleTrayMode() {
    trayModeEnabled = !trayModeEnabled;
    settings.tray_mode_enabled.set(trayModeEnabled);
  }

  function restoreSettings(): void {
    openModal({
      title: m.msg_warning(),
      backdrop: true,
      type: "warning",
      message: m.warn_restore(),
      cancelText: m.msg_cancel(),
      onConfirm: async () => {
        settings.theme.clear();
        settings.font_family.clear();
        settings.font_zoom.clear();
        settings.tray_mode_enabled.clear();
        settings.asr_hotkey_enabled.clear();
        settings.asr_hotkey_shortcut.clear();
        settings.asr_hotkey_trigger_mode.clear();
        window.location.reload();
      }
    });
  }
</script>

<ul class="list mt-10 rounded-box bg-base-100 shadow-md xl:w-xl 2xl:w-3xl">
  <li class="p-4 pb-2 text-center text-xl font-bold tracking-wide">{m.settings_general()}</li>
  <li class="list-row items-center justify-center">
    <p class="list-col-grow text-base">{m.settings_language()}</p>
    <LanguageSelector />
  </li>
  <li class="list-row items-center justify-center">
    <p class="list-col-grow text-base">{m.settings_theme()}</p>
    <ThemeSelector />
  </li>
</ul>

<ul class="list mt-10 rounded-box bg-base-100 shadow-md xl:w-xl 2xl:w-3xl">
  <li class="p-4 pb-2 text-center text-xl font-bold tracking-wide">{m.settings_appearance()}</li>
  <li class="list-row items-center justify-center">
    <p class="list-col-grow text-base">{m.settings_font_family()}</p>
    <FontSelector />
  </li>
  <li class="list-row items-center justify-center">
    <p class="list-col-grow text-base">{m.settings_font_zoom()}</p>
    <FontZoomSelector />
  </li>
</ul>

<ul class="list mt-10 rounded-box bg-base-100 shadow-md xl:w-xl 2xl:w-3xl">
  <li class="p-4 pb-2 text-center text-xl font-bold tracking-wide">{m.settings_desktop()}</li>
  <li class="list-row items-center justify-center gap-4">
    <div class="list-col-grow">
      <p class="text-base">{m.settings_tray_mode()}</p>
      <p class="text-sm text-base-content/60">{m.settings_tray_mode_description()}</p>
    </div>
    <input type="checkbox" class="toggle toggle-primary" checked={trayModeEnabled} onchange={toggleTrayMode} />
  </li>
</ul>

<ul class="list mt-10 rounded-box bg-base-100 shadow-md xl:w-xl 2xl:w-3xl">
  <li class="list-row flex w-full items-center justify-center">
    <button class="btn w-1/2 btn-error" onclick={restoreSettings}>{m.settings_restore()}</button>
  </li>
</ul>

<style>
</style>
