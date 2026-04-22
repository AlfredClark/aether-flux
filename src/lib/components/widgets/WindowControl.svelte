<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { m } from "$lib/i18n/paraglide/messages";
  import CloseIcon from "$lib/icons/window/CloseIcon.svelte";
  import MinimizeIcon from "$lib/icons/window/MinimizeIcon.svelte";
  import MaximizeIcon from "$lib/icons/window/MaximizeIcon.svelte";
  import { openModal } from "$lib/stores/modal";
  import settings from "$lib/stores/settings";
  import AlwaysOnTop from "$lib/icons/window/AlwaysOnTop.svelte";
  import { onMount } from "svelte";

  let isTop = false;

  async function toggleAlwaysOnTop() {
    await getCurrentWindow().setAlwaysOnTop(!(await getCurrentWindow().isAlwaysOnTop()));
    isTop = await getCurrentWindow().isAlwaysOnTop();
  }

  async function minimizeWindow() {
    await getCurrentWindow().minimize();
  }

  async function toggleMaximizeWindow() {
    await getCurrentWindow().toggleMaximize();
  }

  async function closeWindow() {
    if (settings.tray_mode_enabled.get()) {
      await getCurrentWindow().close();
      return;
    }

    openModal({
      title: m.msg_warning(),
      backdrop: true,
      type: "warning",
      message: m.warn_close(),
      cancelText: m.msg_cancel(),
      onConfirm: async () => {
        await getCurrentWindow().close();
      }
    });
  }

  onMount(async () => {
    isTop = await getCurrentWindow().isAlwaysOnTop();
  });
</script>

<div class="grid grid-cols-4 gap-3">
  <button title={m.window_always_on_top()} class="btn btn-circle btn-sm" onclick={toggleAlwaysOnTop}>
    <AlwaysOnTop className="size-6" {isTop} />
  </button>
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
