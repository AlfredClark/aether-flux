<script lang="ts">
  import { onMount } from "svelte";
  import settings from "$lib/stores/settings";
  import { getSystemFonts } from "tauri-plugin-system-fonts-api";

  let changing = false;
  let nowFont = settings.font_family.get();
  let source: string[] = [];
  let inputValue = nowFont;
  let isOpen = false;

  $: keyword = inputValue.trim().toLowerCase();
  $: filtered = keyword ? source.filter((item) => item.toLowerCase().includes(keyword)) : source;

  function handleFocus() {
    isOpen = true;
  }

  function handleSelect(item: string) {
    settings.font_family.set(item);
    inputValue = item;
    isOpen = false;
    changing = false;
    nowFont = item;
  }

  function handleBlur() {
    setTimeout(() => {
      isOpen = false;
      changing = false;
      inputValue = nowFont;
    }, 100);
  }

  onMount(async () => {
    const fontList = await getSystemFonts();
    source = [...new Set(fontList.map((font) => font.name))];
  });
</script>

{#if changing}
  <div class="w-full max-w-md">
    <div class="form-control w-full">
      <div class="relative">
        <input
          type="text"
          bind:value={inputValue}
          class="input-bordered input w-auto text-center"
          placeholder={nowFont}
          onfocus={handleFocus}
          onblur={handleBlur} />
        {#if isOpen && filtered.length > 0}
          <ul
            class="menu absolute z-50 mt-2 max-h-50 flex-row overflow-x-hidden overflow-y-auto rounded-box border border-base-300 bg-base-200 p-2 shadow">
            <li class="w-full">
              <button
                type="button"
                class="text-left"
                onmousedown={(event) => {
                  handleSelect("System");
                  event.preventDefault();
                }}>
                System
              </button>
            </li>
            {#each filtered as item (item)}
              <li class="w-full">
                <button
                  type="button"
                  class="text-left"
                  style:font-family={item}
                  onmousedown={(event) => {
                    handleSelect(item);
                    event.preventDefault();
                  }}>
                  {item}
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    </div>
  </div>
{:else}
  <button
    class="btn"
    onclick={(event) => {
      changing = true;
      event.preventDefault();
      inputValue = nowFont === "System" ? "" : nowFont;
    }}>{nowFont}</button>
{/if}
