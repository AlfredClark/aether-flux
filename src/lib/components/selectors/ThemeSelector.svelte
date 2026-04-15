<script lang="ts">
  import themes from "daisyui/theme/object";
  import settings from "$lib/stores/settings";
  import ThemeCard from "$lib/components/widgets/ThemeCard.svelte";

  let nowTheme = $state(settings.theme.get());
  let themeList = $state(Object.keys(themes).sort());

  function clickOutsideDetails(node: HTMLDetailsElement) {
    const handleClick = (event: MouseEvent) => {
      if (!node.contains(event.target as Node) && node.open) {
        node.open = false;
      }
    };
    document.addEventListener("click", handleClick);
    return {
      destroy() {
        document.removeEventListener("click", handleClick);
      }
    };
  }

  function setTheme(theme: string) {
    nowTheme = theme;
    settings.theme.set(theme);
  }
</script>

<details class="dropdown dropdown-center" use:clickOutsideDetails>
  <summary class="list-none"><ThemeCard theme={nowTheme} /></summary>
  <ul class="dropdown-content menu z-1 h-100 w-3xl flex-row overflow-y-auto rounded-box bg-base-200 p-2 shadow-sm 2xl:w-5xl">
    {#each themeList as theme (theme)}
      <li class="m-0 w-1/4 p-0 2xl:w-1/5">
        <button
          onclick={() => {
            setTheme(theme);
          }}><ThemeCard {theme} /></button>
      </li>
    {/each}
  </ul>
</details>
