<script lang="ts">
  import { m } from "$lib/i18n/paraglide/messages";
  import { openModal } from "$lib/stores/modal";
</script>

<button
  class="btn btn-primary"
  onclick={() => {
    openModal({ title: m.info(), type: "info", message: "Demo Modal!" });
  }}
>
  Open Modal
</button>
