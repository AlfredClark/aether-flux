<script lang="ts">
  import "./layout.css";
  import settings from "$lib/stores/settings";
  import TauriSettingsSync from "$lib/components/app/TauriSettingsSync.svelte";
  import Default from "$lib/layouts/Default.svelte";
  import GlobalDialog from "$lib/components/dialogs/GlobalDialog.svelte";

  const { children } = $props();
  const isRecordingStatusRoute = window.location.pathname === "/recording-status";
  settings.theme.subscribe((value) => {
    document.documentElement.setAttribute("data-theme", value);
  });

  settings.font_family.subscribe((value) => {
    document.documentElement.style.setProperty("--font-sans", value);
  });

  settings.font_zoom.subscribe((value) => {
    document.documentElement.style.setProperty("zoom", value.toString());
  });
</script>

{#if isRecordingStatusRoute}
  {@render children()}
{:else}
  <Default>
    {@render children()}
  </Default>

  <TauriSettingsSync />
  <GlobalDialog />
{/if}
