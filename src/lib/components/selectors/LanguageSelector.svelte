<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { m } from "$lib/i18n/paraglide/messages";
  import { locales, getLocale, setLocale } from "$lib/i18n/paraglide/runtime";

  let nowLocale = $state(getLocale());

  async function setLocaleAndReload(locale: (typeof locales)[number]) {
    try {
      await invoke("update_app_settings", {
        patch: {
          locale
        }
      });
    } catch (error) {
      console.error("Failed to update app locale setting", error);
    }

    setLocale(locale, { reload: false });
    location.replace("/settings");
  }

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
</script>

<details class="dropdown dropdown-end" use:clickOutsideDetails>
  <summary class="btn m-1">{m[nowLocale]()}</summary>
  <ul class="dropdown-content menu z-1 w-52 rounded-box bg-base-200 p-2 shadow-sm">
    {#each locales as locale (locale)}
      <li>
        <button
          onclick={async () => {
            await setLocaleAndReload(locale);
          }}>{m[locale]()}</button>
      </li>
    {/each}
  </ul>
</details>
