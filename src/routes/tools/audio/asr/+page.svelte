<script>
  import { resolve } from "$app/paths";
  import { m } from "$lib/i18n/paraglide/messages";
  import MicrophoneTest from "$lib/components/widgets/MicrophoneTest.svelte";
  import MicRecorder from "$lib/components/widgets/MicRecorder.svelte";
</script>

<div class="tabs-lift tabs h-full w-full">
  <input type="radio" name="asr_tabs" class="tab px-10" aria-label={m.tools_audio_asr_use()} checked />
  <div class="tab-content border-base-300 bg-base-100 p-6">
    <MicrophoneTest />
    <MicRecorder />
  </div>

  <input type="radio" name="asr_tabs" class="tab px-10" aria-label={m.tools_audio_asr_word_bank()} />
  <div class="tab-content border-base-300 bg-base-100 p-6">Tab content 2</div>

  <input type="radio" name="asr_tabs" class="tab px-10" aria-label={m.tools_audio_asr_settings()} />
  <div class="tab-content border-base-300 bg-base-100 p-6">Tab content 3</div>

  <a class="btn ml-auto px-10 btn-error" href={resolve("/tools")}>{m.msg_back()}</a>
</div>
