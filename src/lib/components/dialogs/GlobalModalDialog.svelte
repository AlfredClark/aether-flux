<script lang="ts">
  import { onMount } from "svelte";
  import { modalStore, closeModal } from "$lib/stores/modal";
  import { m } from "$lib/i18n/paraglide/messages";

  let dialogElement: HTMLDialogElement | undefined;

  const unsubscribe = modalStore.subscribe((state) => {
    if (!dialogElement) return;
    if (state.isOpen && !dialogElement.open) {
      dialogElement.showModal();
    } else if (!state.isOpen && dialogElement.open) {
      dialogElement.close();
    }
  });

  function onBackdropClick(event: MouseEvent) {
    if (event.target === dialogElement && $modalStore.options?.backdrop) {
      handleCancel();
    }
  }

  function handleConfirm(event: MouseEvent) {
    event.preventDefault();
    $modalStore.options?.onConfirm?.();
    closeModal();
  }

  function handleCancel() {
    $modalStore.options?.onCancel?.();
    closeModal();
  }

  onMount(() => {
    return unsubscribe;
  });
</script>

<dialog bind:this={dialogElement} class="modal" onclick={onBackdropClick} onclose={handleCancel}>
  <div class="modal-box">
    <h3 class="text-lg font-bold">
      {$modalStore.options?.title}
    </h3>
    <p class="py-4">
      {$modalStore.options?.message}
    </p>
    <div class="modal-action">
      <form method="dialog">
        {#if $modalStore.options?.cancelText || $modalStore.options?.onCancel}
          <button class="btn" value="cancel">{$modalStore.options.cancelText || m.msg_cancel()}</button>
        {/if}
        <button class="btn btn-primary" onclick={handleConfirm}>
          {$modalStore.options?.confirmText || m.msg_confirm()}
        </button>
      </form>
    </div>
  </div>
</dialog>
