<script lang="ts">
  import { browser } from "$app/environment";
  import { resolve } from "$app/paths";
  import { onMount } from "svelte";
  import AsrSettingsTab from "$lib/components/asr/AsrSettingsTab.svelte";
  import AsrTab from "$lib/components/asr/AsrTab.svelte";
  import WordBankTab from "$lib/components/asr/WordBankTab.svelte";
  import { m } from "$lib/i18n/paraglide/messages";

  type AsrPageTab = "use" | "word-bank" | "settings";

  const ACTIVE_TAB_STORAGE_KEY = "tools.audio.asr.active-tab";

  let activeTab: AsrPageTab = "use";

  function setActiveTab(tab: AsrPageTab) {
    activeTab = tab;
    if (browser) {
      localStorage.setItem(ACTIVE_TAB_STORAGE_KEY, tab);
    }
  }

  onMount(() => {
    if (!browser) return;
    const storedTab = localStorage.getItem(ACTIVE_TAB_STORAGE_KEY);
    if (storedTab === "use" || storedTab === "word-bank" || storedTab === "settings") {
      activeTab = storedTab;
    }
  });
</script>

<div class="tabs-lift tabs flex h-full w-full flex-wrap content-start overflow-hidden">
  <input
    type="radio"
    name="audio-asr-tabs"
    class="tab h-auto px-8 py-3 text-sm"
    aria-label={m.tools_audio_asr_use()}
    checked={activeTab === "use"}
    on:change={() => setActiveTab("use")} />
  <div class="tab-content h-full w-full overflow-hidden border-base-300 bg-base-100 p-4 sm:p-6">
    <AsrTab />
  </div>

  <input
    type="radio"
    name="audio-asr-tabs"
    class="tab h-auto px-8 py-3 text-sm"
    aria-label={m.tools_audio_asr_word_bank()}
    checked={activeTab === "word-bank"}
    on:change={() => setActiveTab("word-bank")} />
  <div class="tab-content h-full w-full overflow-hidden border-base-300 bg-base-100 p-4 sm:p-6">
    <WordBankTab />
  </div>

  <input
    type="radio"
    name="audio-asr-tabs"
    class="tab h-auto px-8 py-3 text-sm"
    aria-label={m.tools_audio_asr_settings()}
    checked={activeTab === "settings"}
    on:change={() => setActiveTab("settings")} />
  <div class="tab-content h-full w-full overflow-hidden border-base-300 bg-base-100 p-4 sm:p-6">
    <AsrSettingsTab />
  </div>

  <a class="btn m-0 ml-auto shrink-0 px-10 text-sm btn-error" href={resolve("/tools")}>{m.msg_back()}</a>
</div>
