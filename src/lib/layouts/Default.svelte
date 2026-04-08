<script lang="ts">
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { m } from "$lib/i18n/paraglide/messages";
  import type { PathnameWithSearchOrHash } from "$app/types";
  import WindowControl from "$lib/components/WindowControl.svelte";
  import { getRouter } from "$lib/utils/router";
  import { goto } from "$app/navigation";
  import { resolve } from "$app/paths";

  let { children } = $props();

  const router = getRouter();
  let version = $state(__APP_PKG__.version);
  let path: PathnameWithSearchOrHash = $state("/home");

  $effect(() => {
    goto(resolve(path));
  });

  onMount(async () => {
    version = await getVersion();
  });
</script>

<div id="container" class="grid h-screen w-screen overflow-hidden select-none">
  <header class="w-full border border-base-300 p-1">
    <div class="mockup-window flex flex-row items-end justify-between p-1" data-tauri-drag-region>
      <div class="flex flex-row items-center justify-center gap-1">
        <img class="size-8" src="/icon-tauri-dark.svg" alt="" />
        <b class="text-xl">{m.app_name()}</b>
        <b class="badge badge-ghost">{version}</b>
      </div>
      <WindowControl />
    </div>
  </header>

  <nav class="navbar-center">
    <div class="tabs-border tabs grid w-2/5 grid-cols-{router.length}">
      {#each router as route (route.path)}
        <input
          type="radio"
          name="nav_tabs"
          class="tab"
          bind:group={path}
          aria-label={route.name}
          value={route.path}
          checked={route.default}
        />
      {/each}
    </div>
  </nav>

  <main class="flex h-full w-full items-center justify-center">
    {@render children?.()}
  </main>

  <footer class="footer border border-base-300 p-1">
    <div class="flex w-full items-center justify-center">
      <p>Copyright © {new Date().getFullYear()} Alfred Clark. All rights reserved.</p>
    </div>
  </footer>
</div>

<style>
  #container {
    grid-template-areas:
      "header"
      "nav"
      "main"
      "footer";
    grid-template-rows: auto auto 1fr auto;
  }

  #container > header {
    grid-area: header;
  }

  #container > nav {
    grid-area: nav;
  }

  #container > main {
    grid-area: main;
  }

  #container > footer {
    grid-area: footer;
  }

  .mockup-window {
    &:before {
      display: none;
    }
  }
</style>
