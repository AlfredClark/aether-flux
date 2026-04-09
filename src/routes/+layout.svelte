<script lang="ts">
  import "./layout.css";
  import Default from "$lib/layouts/Default.svelte";
  import GlobalModalDialog from "$lib/components/GlobalModalDialog.svelte";

  const { children } = $props();
</script>

<Default>
  {@render children()}
</Default>

<GlobalModalDialog />
