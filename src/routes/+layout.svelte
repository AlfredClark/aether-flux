<script lang="ts">
  import "./layout.css";
  import Default from "$lib/layouts/Default.svelte";

  const { children } = $props();
</script>

<Default>
  {@render children()}
</Default>
