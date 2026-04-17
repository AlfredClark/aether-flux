<script>
  import { resolve } from "$app/paths";

  let { data } = $props();
</script>

{#each data.toolsRouter as category (category.name)}
  <section class="w-full p-10 not-first:pt-5 not-last:pb-5 2xl:p-20 2xl:not-first:pt-10 2xl:not-last:pb-10">
    <div class="mb-4 flex items-end justify-between gap-4">
      <div>
        <h2 class="text-xl font-bold">{category.label}</h2>
        <p class="mt-1 text-sm text-base-content/75">{category.desc}</p>
      </div>
    </div>
    <div class="grid gap-4 xl:grid-cols-4 2xl:grid-cols-5">
      {#each category.tools as tool (tool.name)}
        {@const Icon = tool.icon}
        <a
          href={resolve(tool.path)}
          class="group rounded-2xl border border-base-300 bg-base-100 p-5 text-left transition hover:-translate-y-0.5 hover:border-primary/50 hover:shadow-md">
          <div class="flex items-start gap-4">
            <div
              class="flex aspect-square w-1/4 shrink-0 items-center justify-center rounded-xl bg-base-200 text-sm font-semibold">
              <Icon className="w-4/5" />
            </div>
            <div class="min-w-0 flex-1">
              <div class="font-medium transition-colors group-hover:text-primary">{tool.label}</div>
              <div class="mt-1 text-sm leading-6 text-base-content/60">{tool.desc}</div>
            </div>
          </div>
        </a>
      {/each}
    </div>
  </section>
{/each}
