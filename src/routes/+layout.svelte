<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { locales, setLocale } from "$lib/i18n/paraglide/runtime";
  import "./layout.css";
  import settings from "$lib/stores/settings";
  import TauriSettingsSync from "$lib/components/app/TauriSettingsSync.svelte";
  import Default from "$lib/layouts/Default.svelte";
  import GlobalDialog from "$lib/components/dialogs/GlobalDialog.svelte";

  type AppSettings = {
    locale: string;
  };

  const { children } = $props();
  let localeReady = $state(false);

  settings.theme.subscribe((value) => {
    document.documentElement.setAttribute("data-theme", value);
  });

  settings.font_family.subscribe((value) => {
    document.documentElement.style.setProperty("--font-sans", value);
  });

  settings.font_zoom.subscribe((value) => {
    document.documentElement.style.setProperty("zoom", value.toString());
  });

  onMount(() => {
    let disposed = false;

    const syncBackendLocale = async () => {
      try {
        const appSettings = await invoke<AppSettings>("get_app_settings");
        if (!disposed && locales.includes(appSettings.locale as (typeof locales)[number])) {
          setLocale(appSettings.locale as (typeof locales)[number], { reload: false });
        }
      } catch (error) {
        if (!disposed) {
          console.error("Failed to load app locale settings", error);
        }
      } finally {
        if (!disposed) {
          localeReady = true;
        }
      }
    };

    void syncBackendLocale();

    return () => {
      disposed = true;
    };
  });
</script>

{#if localeReady}
  <Default>
    {@render children()}
  </Default>

  <TauriSettingsSync />
  <GlobalDialog />
{/if}
