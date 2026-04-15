<script lang="ts">
  import "./layout.css";
  import settings from "$lib/stores/settings";
  import Default from "$lib/layouts/Default.svelte";
  import GlobalModalDialog from "$lib/components/GlobalModalDialog.svelte";

  const { children } = $props();
  settings.theme.subscribe((value) => {
    document.documentElement.setAttribute("data-theme", value);
  });
</script>

<Default>
  {@render children()}
</Default>

<GlobalModalDialog />
