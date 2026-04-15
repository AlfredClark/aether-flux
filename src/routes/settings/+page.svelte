<script lang="ts">
  import { m } from "$lib/i18n/paraglide/messages";
  import { locales, getLocale, setLocale } from "$lib/i18n/paraglide/runtime";
  import themes from "daisyui/theme/object";
  import settings from "$lib/stores/settings";
  import ThemeCard from "$lib/components/ThemeCard.svelte";

  let nowLocale = $state(getLocale());
  let nowTheme = $state(settings.theme.get());
  let themeList = $state(Object.keys(themes).sort());

  function setLocaleAndReload(locale: typeof locales[number]) {
    setLocale(locale, {reload: false});
    location.replace("/settings")
  }

  function setTheme(theme: string) {
    nowTheme = theme;
    settings.theme.set(theme);
  }
</script>

<ul class="list mt-10 w-xl rounded-box bg-base-100 shadow-md 2xl:w-2xl">
  <li class="p-4 pb-2 text-xl font-bold tracking-wide opacity-100">{m.general_settings()}</li>
  <li class="list-row items-center justify-center">
    <p class="list-col-grow text-base">{m.language()}</p>
    <div class="dropdown">
      <div tabindex="0" role="button" class="btn m-1">{m[nowLocale]()}</div>
      <ul tabindex="-1" class="dropdown-content menu bg-base-100 rounded-box z-1 w-52 p-2 shadow-sm">
        {#each locales as locale (locale)}
          <li><button onclick="{() => {setLocaleAndReload(locale)}}">{m[locale]()}</button></li>
        {/each}
      </ul>
    </div>
  </li>
  <li class="list-row items-center justify-center">
    <p class="list-col-grow text-base">{m.theme()}</p>
    <div class="dropdown dropdown-bottom dropdown-end">
      <div tabindex="0" role="button">
        <ThemeCard theme={nowTheme}/>
      </div>
      <div class="flex flex-wrap w-xl bg-base-100 dropdown-content overflow-auto">
          {#each themeList as theme (theme)}
            <button class="w-1/3 p-1" onclick="{() => {setTheme(theme)}}">
              <ThemeCard {theme}/>
            </button>
          {/each}
      </div>
    </div>
  </li>
</ul>

<style>
</style>
