<script lang="ts">
  import { setLocale } from "$lib/i18n/paraglide/runtime";

  function setEN() {
    setLocale("en");
  }

  function setZH() {
    setLocale("zh");
  }
</script>

<div class="flex flex-col items-center justify-center gap-5">
  <button class="btn btn-primary" onclick={setEN}>EN</button>
  <button class="btn btn-secondary" onclick={setZH}>ZH</button>
</div>
