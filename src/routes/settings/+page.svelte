<script lang="ts">
  import { m } from "$lib/i18n/paraglide/messages";
  import FontSelector from "$lib/components/selectors/FontSelector.svelte";
  import ThemeSelector from "$lib/components/selectors/ThemeSelector.svelte";
  import LanguageSelector from "$lib/components/selectors/LanguageSelector.svelte";
</script>

<ul class="list mt-10 rounded-box bg-base-100 shadow-md xl:w-xl 2xl:w-3xl">
  <li class="p-4 pb-2 text-xl font-bold tracking-wide">{m.general_settings()}</li>
  <li class="list-row items-center justify-center">
    <p class="list-col-grow text-base">{m.language()}</p>
    <LanguageSelector />
  </li>
  <li class="list-row items-center justify-center">
    <p class="list-col-grow text-base">{m.theme()}</p>
    <ThemeSelector />
  </li>
</ul>

<ul class="list mt-10 rounded-box bg-base-100 shadow-md xl:w-xl 2xl:w-3xl">
  <li class="p-4 pb-2 text-xl font-bold tracking-wide">{m.appearance_settings()}</li>
  <li class="list-row items-center justify-center">
    <p class="list-col-grow text-base">{m.font_family()}</p>
    <FontSelector />
  </li>
</ul>

<style>
</style>
