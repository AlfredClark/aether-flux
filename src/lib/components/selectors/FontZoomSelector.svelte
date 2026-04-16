<script>
  import settings from "$lib/stores/settings";

  let zoom = $state(Number(settings.font_zoom.get()));

  $effect(() => {
    settings.font_zoom.set(zoom);
  });
</script>

<div class="flex w-full max-w-xs flex-row">
  <div class="join">
    <button
      class="btn join-item"
      onclick={() => {
        zoom = zoom >= 0.875 ? zoom - 0.125 : zoom;
      }}>-</button>
    <button class="btn join-item">{zoom}</button>
    <button
      class="btn join-item"
      onclick={() => {
        zoom = zoom <= 1.125 ? zoom + 0.125 : zoom;
      }}>+</button>
  </div>
</div>
