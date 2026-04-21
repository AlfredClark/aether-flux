<script lang="ts">
  import "./layout.css";
  import settings from "$lib/stores/settings";
  import Default from "$lib/layouts/Default.svelte";
  import GlobalDialog from "$lib/components/dialogs/GlobalDialog.svelte";

  const { children } = $props();
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

<Default>
  {@render children()}
</Default>

<GlobalDialog />
