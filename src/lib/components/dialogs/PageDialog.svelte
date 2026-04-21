<script lang="ts">
  import { m } from "$lib/i18n/paraglide/messages";

  type PageDialogCloseReason = "confirm" | "cancel" | "dismiss";

  export let open = false;
  export let title = "";
  export let message = "";
  export let backdrop = true;
  export let showActions = true;
  export let showConfirmButton = true;
  export let showCancelButton = true;
  export let closeOnConfirm = true;
  export let closeOnCancel = true;
  export let confirmText = "";
  export let cancelText = "";
  export let confirmButtonClass = "btn btn-primary";
  export let cancelButtonClass = "btn";
  export let boxClass = "";
  export let onconfirm: (() => void) | undefined = undefined;
  export let oncancel: (() => void) | undefined = undefined;
  export let onclose: ((detail: { reason: PageDialogCloseReason }) => void) | undefined = undefined;

  let dialogElement: HTMLDialogElement | undefined;
  let closeReason: PageDialogCloseReason = "dismiss";
  let suppressCancelEvent = false;

  $: if (dialogElement) {
    if (open && !dialogElement.open) {
      dialogElement.showModal();
    } else if (!open && dialogElement.open) {
      dialogElement.close();
    }
  }

  function closeDialog(reason: PageDialogCloseReason) {
    closeReason = reason;
    open = false;

    if (dialogElement?.open) {
      dialogElement.close();
    }
  }

  function handleConfirm(event?: Event) {
    event?.preventDefault();
    onconfirm?.();

    if (closeOnConfirm) {
      suppressCancelEvent = true;
      closeDialog("confirm");
      suppressCancelEvent = false;
    }
  }

  function handleCancel(event?: Event) {
    event?.preventDefault();
    oncancel?.();

    if (closeOnCancel) {
      closeDialog("cancel");
    }
  }

  function handleBackdropClick(event: MouseEvent) {
    if (event.target === dialogElement && backdrop) {
      handleCancel(event);
    }
  }

  function handleNativeCancel(event: Event) {
    if (!backdrop) {
      event.preventDefault();
      return;
    }

    event.preventDefault();
    handleCancel(event);
  }

  function handleClose() {
    open = false;
    onclose?.({ reason: closeReason });

    if (closeReason === "dismiss" && !suppressCancelEvent) {
      oncancel?.();
    }

    closeReason = "dismiss";
    suppressCancelEvent = false;
  }
</script>

<dialog
  bind:this={dialogElement}
  class="modal"
  onclick={handleBackdropClick}
  oncancel={handleNativeCancel}
  onclose={handleClose}>
  <div class={`modal-box ${boxClass}`.trim()}>
    {#if title}
      <h3 class="text-lg font-bold">
        {title}
      </h3>
    {/if}

    {#if message}
      <p class={title ? "py-4" : "pb-4"}>
        {message}
      </p>
    {/if}

    <slot />

    {#if showActions}
      <div class="modal-action">
        <slot name="actions">
          <form method="dialog">
            {#if showCancelButton}
              <button class={cancelButtonClass} value="cancel" onclick={handleCancel}>
                {cancelText || m.msg_cancel()}
              </button>
            {/if}
            {#if showConfirmButton}
              <button class={confirmButtonClass} onclick={handleConfirm}>
                {confirmText || m.msg_confirm()}
              </button>
            {/if}
          </form>
        </slot>
      </div>
    {/if}
  </div>
</dialog>
