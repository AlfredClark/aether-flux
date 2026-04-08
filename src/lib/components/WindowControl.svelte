<script lang="ts">
  import { window } from "@tauri-apps/api";
  import { m } from "$lib/i18n/paraglide/messages";
  import CloseIcon from "$lib/icons/CloseIcon.svelte";
  import MinimizeIcon from "$lib/icons/MinimizeIcon.svelte";
  import MaximizeIcon from "$lib/icons/MaximizeIcon.svelte";

  function minimizeWindow() {
    window.getCurrentWindow().minimize();
  }

  function toggleMaximizeWindow() {
    window.getCurrentWindow().toggleMaximize();
  }

  function closeWindow() {
    window.getCurrentWindow().close();
  }
</script>

<div class="grid grid-cols-3 gap-3">
  <button title={m.window_minimize()} class="btn btn-circle btn-sm" onclick={minimizeWindow}>
    <MinimizeIcon className="size-6" />
  </button>
  <button title={m.window_maximize()} class="btn btn-circle btn-sm" onclick={toggleMaximizeWindow}>
    <MaximizeIcon className="size-6" />
  </button>
  <button title={m.window_close()} class="btn btn-circle btn-sm" onclick={closeWindow}>
    <CloseIcon className="size-6" />
  </button>
</div>
