<script lang="ts">
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { m } from "$lib/i18n/paraglide/messages";
  import type { Pathname } from "$app/types";
  import WindowControl from "$lib/components/widgets/WindowControl.svelte";
  import { getTabs, type Tab } from "$lib/tabs";
  import { goto } from "$app/navigation";
  import { resolve } from "$app/paths";

  let { children } = $props();

  const tabs = getTabs();
  let version = $state(__APP_PKG__.version);
  let path: Pathname = $state(window.location.pathname as Pathname);

  $effect(() => {
    if (path === "/" || !window.location.pathname.startsWith(path)) {
      goto(resolve(path));
    }
  });

  onMount(async () => {
    tabs.map((tab: Tab) => {
      if (tab.path == "/" && path !== tab.path) {
        tab.home = false;
        return tab;
      }
      if (path.startsWith(tab.path)) {
        tab.home = true;
        path = tab.path as Pathname;
      }
      return tab;
    });
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
    <div class="tabs-border tabs grid w-2/5 grid-cols-5">
      {#each tabs as tab (tab.path)}
        <input
          type="radio"
          name="nav_tabs"
          class="tab text-base 2xl:text-lg"
          bind:group={path}
          aria-label={tab.label}
          value={tab.path}
          checked={tab.home} />
      {/each}
    </div>
  </nav>

  <main class="flex h-full w-full flex-col items-center justify-start overflow-y-auto bg-base-300">
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
