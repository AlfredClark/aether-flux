<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import PageDialog from "$lib/components/dialogs/PageDialog.svelte";
  import { m } from "$lib/i18n/paraglide/messages";
  import { openModal } from "$lib/stores/modal";

  type WordbankSummary = {
    id: string;
    name: string;
    description: string | null;
    prefix: string | null;
    suffix: string | null;
    sortOrder: number;
    isDefault: boolean;
    isEnabled: boolean;
    entryTotal: number;
  };

  type WordbankEntry = {
    key: string;
    values: string[];
  };

  type WordbankListResult = {
    entries: WordbankEntry[];
    total: number;
  };

  type WordbankBatchResult = {
    acceptedCount: number;
  };

  let wordBanks: WordbankSummary[] = [];
  let selectedWordbankId = "";
  let wordbankEntries: WordbankEntry[] = [];
  let wordbankTotal = 0;
  let wordbankQuery = "";
  let wordbankLoading = false;
  let wordbankSaving = false;
  let wordbankError = "";

  let createWordbankName = "";
  let createWordbankDescription = "";
  let createWordbankPrefix = "";
  let createWordbankSuffix = "";
  let editWordbankName = "";
  let editWordbankDescription = "";
  let editWordbankPrefix = "";
  let editWordbankSuffix = "";
  let createDialogOpen = false;
  let editDialogOpen = false;
  let entryEditDialogOpen = false;

  let wordbankDraft = "";
  let wordbankEditOriginal = "";
  let wordbankEditValue = "";
  let importInputElement: HTMLInputElement | undefined;
  let draggedWordbankId = "";
  let dropTargetWordbankId = "";
  let dropTargetWordbankPosition: "before" | "after" | "" = "";
  let draggedEntryKey = "";
  let draggedValue = "";
  let dropTargetEntryKey = "";
  let dropTargetValue = "";
  let dropTargetPosition: "before" | "after" | "" = "";
  let entriesScrollContainer: HTMLDivElement | undefined;

  function selectedWordbank() {
    return wordBanks.find((bank) => bank.id === selectedWordbankId) ?? null;
  }

  function syncSelectedWordbankForm() {
    const bank = selectedWordbank();
    editWordbankName = bank?.name ?? "";
    editWordbankDescription = bank?.description ?? "";
    editWordbankPrefix = bank?.prefix ?? "";
    editWordbankSuffix = bank?.suffix ?? "";
  }

  function openCreateDialog() {
    createWordbankName = "";
    createWordbankDescription = "";
    createWordbankPrefix = "";
    createWordbankSuffix = "";
    createDialogOpen = true;
  }

  // function openEditDialog() {
  //   if (!selectedWordbank()) return;
  //   syncSelectedWordbankForm();
  //   editDialogOpen = true;
  // }

  function openEditDialogFor(wordbankId: string) {
    selectWordbank(wordbankId);
    syncSelectedWordbankForm();
    editDialogOpen = true;
  }

  function selectWordbank(wordbankId: string) {
    selectedWordbankId = wordbankId;
    syncSelectedWordbankForm();
    cancelWordbankEdit();
    void loadWordbankEntries(wordbankQuery, wordbankId);
  }

  function beginWordbankEdit(value: string) {
    wordbankEditOriginal = value;
    wordbankEditValue = value;
    entryEditDialogOpen = true;
  }

  function cancelWordbankEdit() {
    wordbankEditOriginal = "";
    wordbankEditValue = "";
    entryEditDialogOpen = false;
  }

  function confirmByModal(options: { title: string; message: string }) {
    return new Promise<boolean>((resolve) => {
      openModal({
        title: options.title,
        message: options.message,
        type: "warning",
        backdrop: true,
        cancelText: m.msg_cancel(),
        confirmText: m.msg_confirm(),
        onConfirm: () => resolve(true),
        onCancel: () => resolve(false)
      });
    });
  }

  async function loadWordbanks(preferredId = selectedWordbankId) {
    wordbankLoading = true;
    wordbankError = "";

    try {
      const result = await invoke<WordbankSummary[]>("list_wordbanks");
      wordBanks = result;

      const nextWordbank =
        result.find((item) => item.id === preferredId) ?? result.find((item) => item.isDefault) ?? result[0] ?? null;

      selectedWordbankId = nextWordbank?.id ?? "";
      syncSelectedWordbankForm();
      await loadWordbankEntries(wordbankQuery, selectedWordbankId);
    } catch (e) {
      wordbankError = String(e);
      wordBanks = [];
      selectedWordbankId = "";
      wordbankEntries = [];
      wordbankTotal = 0;
    } finally {
      wordbankLoading = false;
    }
  }

  async function loadWordbankEntries(query = wordbankQuery, wordbankId = selectedWordbankId) {
    if (!wordbankId) {
      wordbankEntries = [];
      wordbankTotal = 0;
      return;
    }

    wordbankLoading = true;
    wordbankError = "";

    try {
      const normalizedQuery = query.trim();
      const result = await invoke<WordbankListResult>("list_wordbank_entries", {
        wordbankId,
        query: normalizedQuery || null
      });
      wordbankEntries = result.entries;
      wordbankTotal = result.total;
    } catch (e) {
      wordbankError = String(e);
      wordbankEntries = [];
      wordbankTotal = 0;
    } finally {
      wordbankLoading = false;
    }
  }

  async function createBank() {
    if (!createWordbankName.trim()) return;

    wordbankSaving = true;
    wordbankError = "";
    try {
      const created = await invoke<WordbankSummary>("create_wordbank", {
        name: createWordbankName,
        description: createWordbankDescription || null,
        prefix: createWordbankPrefix || null,
        suffix: createWordbankSuffix || null
      });
      createWordbankName = "";
      createWordbankDescription = "";
      createWordbankPrefix = "";
      createWordbankSuffix = "";
      createDialogOpen = false;
      await loadWordbanks(created.id);
    } catch (e) {
      wordbankError = String(e);
    } finally {
      wordbankSaving = false;
    }
  }

  async function saveSelectedBank() {
    if (!selectedWordbankId || !editWordbankName.trim()) return;

    wordbankSaving = true;
    wordbankError = "";
    try {
      const updated = await invoke<WordbankSummary>("update_wordbank", {
        wordbankId: selectedWordbankId,
        name: editWordbankName,
        description: editWordbankDescription || null,
        prefix: editWordbankPrefix || null,
        suffix: editWordbankSuffix || null
      });
      editDialogOpen = false;
      await loadWordbanks(updated.id);
    } catch (e) {
      wordbankError = String(e);
    } finally {
      wordbankSaving = false;
    }
  }

  async function clearBank(wordbankId = selectedWordbankId) {
    const bank = wordBanks.find((item) => item.id === wordbankId) ?? null;
    if (!bank) return;

    const confirmed = await confirmByModal({
      title: m.tools_audio_asr_word_bank_management_clear_confirm_title(),
      message: m.tools_audio_asr_word_bank_management_clear_confirm_message({ name: bank.name })
    });
    if (!confirmed) return;

    wordbankSaving = true;
    wordbankError = "";
    try {
      await invoke("clear_wordbank", { wordbankId: bank.id });
      cancelWordbankEdit();
      await loadWordbanks(bank.id);
    } catch (e) {
      wordbankError = String(e);
    } finally {
      wordbankSaving = false;
    }
  }

  async function deleteBank(wordbankId = selectedWordbankId) {
    const bank = wordBanks.find((item) => item.id === wordbankId) ?? null;
    if (!bank || bank.isDefault) return;

    const confirmed = await confirmByModal({
      title: m.tools_audio_asr_word_bank_management_delete_confirm_title(),
      message: m.tools_audio_asr_word_bank_management_delete_confirm_message({ name: bank.name })
    });
    if (!confirmed) return;

    wordbankSaving = true;
    wordbankError = "";
    try {
      await invoke("delete_wordbank", { wordbankId: bank.id });
      cancelWordbankEdit();
      await loadWordbanks();
    } catch (e) {
      wordbankError = String(e);
    } finally {
      wordbankSaving = false;
    }
  }

  async function toggleWordbankEnabled(wordbankId: string, enabled: boolean) {
    wordbankSaving = true;
    wordbankError = "";
    try {
      const updated = await invoke<WordbankSummary>("set_wordbank_enabled", {
        wordbankId,
        enabled
      });
      await loadWordbanks(updated.id);
    } catch (e) {
      wordbankError = String(e);
    } finally {
      wordbankSaving = false;
    }
  }

  async function addWordbankEntries() {
    if (!selectedWordbankId || !wordbankDraft.trim()) return;

    wordbankSaving = true;
    wordbankError = "";
    try {
      await invoke<WordbankBatchResult>("add_wordbank_entries_from_text", {
        wordbankId: selectedWordbankId,
        text: wordbankDraft
      });
      wordbankDraft = "";
      await loadWordbanks(selectedWordbankId);
    } catch (e) {
      wordbankError = String(e);
    } finally {
      wordbankSaving = false;
    }
  }

  async function importWordbankText(text: string) {
    if (!selectedWordbankId || !text.trim()) return;

    wordbankSaving = true;
    wordbankError = "";
    try {
      await invoke<WordbankBatchResult>("add_wordbank_entries_from_text", {
        wordbankId: selectedWordbankId,
        text
      });
      await loadWordbanks(selectedWordbankId);
    } catch (e) {
      wordbankError = String(e);
    } finally {
      wordbankSaving = false;
    }
  }

  async function handleImportFileChange(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;

    try {
      const text = await file.text();
      await importWordbankText(text);
    } catch (e) {
      wordbankError = String(e);
    } finally {
      input.value = "";
    }
  }

  async function saveWordbankEdit() {
    if (!selectedWordbankId || !wordbankEditOriginal || !wordbankEditValue.trim()) return;

    wordbankSaving = true;
    wordbankError = "";
    try {
      await invoke<WordbankEntry>("update_wordbank_entry", {
        wordbankId: selectedWordbankId,
        originalValue: wordbankEditOriginal,
        newValue: wordbankEditValue
      });
      cancelWordbankEdit();
      await loadWordbanks(selectedWordbankId);
    } catch (e) {
      wordbankError = String(e);
    } finally {
      wordbankSaving = false;
    }
  }

  async function deleteWordbankEntry(value: string) {
    if (!selectedWordbankId) return;

    const confirmed = await confirmByModal({
      title: m.tools_audio_asr_word_bank_management_entry_delete_confirm_title(),
      message: m.tools_audio_asr_word_bank_management_entry_delete_confirm_message({ value })
    });
    if (!confirmed) return;

    wordbankSaving = true;
    wordbankError = "";
    try {
      await invoke("delete_wordbank_entry", {
        wordbankId: selectedWordbankId,
        value
      });
      if (wordbankEditOriginal === value) {
        cancelWordbankEdit();
      }
      await loadWordbanks(selectedWordbankId);
    } catch (e) {
      wordbankError = String(e);
    } finally {
      wordbankSaving = false;
    }
  }

  async function deleteWordbankEntryGroup(key: string) {
    if (!selectedWordbankId) return;

    const confirmed = await confirmByModal({
      title: m.tools_audio_asr_word_bank_management_group_delete_confirm_title(),
      message: m.tools_audio_asr_word_bank_management_group_delete_confirm_message({ key })
    });
    if (!confirmed) return;

    wordbankSaving = true;
    wordbankError = "";
    try {
      await invoke("delete_wordbank_entry_group", {
        wordbankId: selectedWordbankId,
        key
      });
      await loadWordbanks(selectedWordbankId);
    } catch (e) {
      wordbankError = String(e);
    } finally {
      wordbankSaving = false;
    }
  }

  function handleEntryDragStart(event: DragEvent, entryKey: string, value: string) {
    if (event.dataTransfer) {
      event.dataTransfer.setData("text/plain", `${entryKey}:${value}`);
      event.dataTransfer.setDragImage(event.currentTarget as Element, 12, 12);
      event.dataTransfer.effectAllowed = "move";
    }
    draggedEntryKey = entryKey;
    draggedValue = value;
  }

  function handleEntryDragEnd() {
    draggedEntryKey = "";
    draggedValue = "";
    dropTargetEntryKey = "";
    dropTargetValue = "";
    dropTargetPosition = "";
  }

  function updateDropTarget(event: DragEvent, entryKey: string, value: string) {
    if (draggedEntryKey !== entryKey) return;

    const currentTarget = event.currentTarget as HTMLDivElement;
    const rect = currentTarget.getBoundingClientRect();
    const offsetY = event.clientY - rect.top;
    const position = offsetY < rect.height / 2 ? "before" : "after";

    dropTargetEntryKey = entryKey;
    dropTargetValue = value;
    dropTargetPosition = position;
  }

  async function reorderWordbankGroup(entryKey: string, targetValue: string, position: "before" | "after") {
    if (!selectedWordbankId || !draggedEntryKey || !draggedValue) return;
    if (draggedEntryKey !== entryKey) return;

    const entry = wordbankEntries.find((item) => item.key === entryKey);
    if (!entry) return;

    const nextValues = [...entry.values];
    const fromIndex = nextValues.indexOf(draggedValue);
    const targetIndex = nextValues.indexOf(targetValue);
    if (fromIndex === -1 || targetIndex === -1) return;

    let insertIndex = position === "after" ? targetIndex + 1 : targetIndex;
    nextValues.splice(fromIndex, 1);
    if (fromIndex < insertIndex) {
      insertIndex -= 1;
    }
    if (insertIndex < 0) insertIndex = 0;
    if (insertIndex > nextValues.length) insertIndex = nextValues.length;
    if (insertIndex === fromIndex) {
      handleEntryDragEnd();
      return;
    }

    nextValues.splice(insertIndex, 0, draggedValue);

    wordbankSaving = true;
    wordbankError = "";
    const scrollTop = entriesScrollContainer?.scrollTop ?? 0;
    try {
      await invoke<WordbankEntry>("reorder_wordbank_entry_group", {
        wordbankId: selectedWordbankId,
        key: entryKey,
        values: nextValues
      });
      await loadWordbankEntries(wordbankQuery, selectedWordbankId);
      await tick();
      if (entriesScrollContainer) {
        entriesScrollContainer.scrollTop = scrollTop;
      }
    } catch (e) {
      wordbankError = String(e);
    } finally {
      wordbankSaving = false;
      handleEntryDragEnd();
    }
  }

  onMount(() => {
    void loadWordbanks();

    const handleWordbankReset = () => {
      wordbankError = "";
      createWordbankName = "";
      createWordbankDescription = "";
      createWordbankPrefix = "";
      createWordbankSuffix = "";
      editWordbankName = "";
      editWordbankDescription = "";
      editWordbankPrefix = "";
      editWordbankSuffix = "";
      wordbankDraft = "";
      cancelWordbankEdit();
      void loadWordbanks();
    };

    window.addEventListener("asr:wordbank-reset", handleWordbankReset);

    return () => {
      window.removeEventListener("asr:wordbank-reset", handleWordbankReset);
    };
  });

  function renderWordbankName(bank: WordbankSummary) {
    return `${bank.prefix ?? ""}${bank.name}${bank.suffix ?? ""}`;
  }

  function handleWordbankDragStart(event: DragEvent, wordbankId: string) {
    const bank = wordBanks.find((item) => item.id === wordbankId);
    if (!bank || bank.isDefault) return;

    if (event.dataTransfer) {
      event.dataTransfer.setData("text/plain", wordbankId);
      event.dataTransfer.setDragImage(event.currentTarget as Element, 12, 12);
      event.dataTransfer.effectAllowed = "move";
    }
    draggedWordbankId = wordbankId;
  }

  function handleWordbankDragEnd() {
    draggedWordbankId = "";
    dropTargetWordbankId = "";
    dropTargetWordbankPosition = "";
  }

  function updateWordbankDropTarget(event: DragEvent, wordbankId: string) {
    if (!draggedWordbankId || draggedWordbankId === wordbankId) return;

    const currentTarget = event.currentTarget as HTMLDivElement;
    const rect = currentTarget.getBoundingClientRect();
    const offsetY = event.clientY - rect.top;
    dropTargetWordbankId = wordbankId;
    dropTargetWordbankPosition = offsetY < rect.height / 2 ? "before" : "after";
  }

  async function reorderWordbanks(targetWordbankId: string, position: "before" | "after") {
    if (!draggedWordbankId || !targetWordbankId || draggedWordbankId === targetWordbankId) {
      handleWordbankDragEnd();
      return;
    }

    const movableBanks = wordBanks.filter((bank) => !bank.isDefault);
    const nextIds = movableBanks.map((bank) => bank.id);
    const fromIndex = nextIds.indexOf(draggedWordbankId);
    const targetIndex = nextIds.indexOf(targetWordbankId);
    if (fromIndex === -1 || targetIndex === -1) {
      handleWordbankDragEnd();
      return;
    }

    let insertIndex = position === "after" ? targetIndex + 1 : targetIndex;
    nextIds.splice(fromIndex, 1);
    if (fromIndex < insertIndex) {
      insertIndex -= 1;
    }
    if (insertIndex < 0) insertIndex = 0;
    if (insertIndex > nextIds.length) insertIndex = nextIds.length;
    if (insertIndex === fromIndex) {
      handleWordbankDragEnd();
      return;
    }

    nextIds.splice(insertIndex, 0, draggedWordbankId);

    wordbankSaving = true;
    wordbankError = "";
    try {
      await invoke<WordbankSummary[]>("reorder_wordbanks", {
        wordbankIds: nextIds
      });
      await loadWordbanks(selectedWordbankId);
    } catch (e) {
      wordbankError = String(e);
    } finally {
      wordbankSaving = false;
      handleWordbankDragEnd();
    }
  }
</script>

<div class="mx-auto flex h-full min-h-0 w-full max-w-7xl flex-col pb-10">
  <div class="grid h-full min-h-0 gap-4 xl:grid-cols-2">
    <div class="flex min-h-0 flex-col gap-4">
      <div class="card border border-base-300 bg-base-100 shadow-md">
        <div class="card-body gap-4">
          <div class="text-sm font-medium">{m.tools_audio_asr_word_bank_management_entry_add_title()}</div>
          <div class="text-xs text-base-content/60">
            {m.tools_audio_asr_word_bank_management_entry_add_hint()}
          </div>
          <form class="flex flex-col gap-3" on:submit|preventDefault={() => void addWordbankEntries()}>
            <textarea
              class="textarea-bordered textarea min-h-28 w-full resize-none"
              bind:value={wordbankDraft}
              placeholder={m.tools_audio_asr_word_bank_management_entry_add_placeholder()}
              disabled={wordbankSaving || !selectedWordbankId}></textarea>
            <div class="flex flex-wrap gap-3">
              <button
                class="btn btn-primary"
                type="submit"
                disabled={wordbankSaving || !selectedWordbankId || !wordbankDraft.trim()}>
                {m.tools_audio_asr_word_bank_management_entry_add_submit()}
              </button>
              <button
                class="btn btn-secondary"
                type="button"
                on:click={() => importInputElement?.click()}
                disabled={wordbankSaving || !selectedWordbankId}>
                {m.tools_audio_asr_word_bank_management_entry_import_action()}
              </button>
              <input
                bind:this={importInputElement}
                class="hidden"
                type="file"
                accept=".txt,text/plain"
                on:change={handleImportFileChange}
                disabled={wordbankSaving || !selectedWordbankId} />
            </div>
          </form>
        </div>
      </div>

      <div class="card min-h-0 flex-1 border border-base-300 bg-base-100 shadow-md">
        <div class="card-body min-h-0 gap-4 overflow-hidden">
          <div class="flex items-center justify-between gap-3">
            <div class="text-base font-semibold">{m.tools_audio_asr_word_bank_management_list_title()}</div>
            <button class="btn btn-sm btn-primary" type="button" on:click={openCreateDialog} disabled={wordbankSaving}>
              {m.tools_audio_asr_word_bank_management_create_action()}
            </button>
          </div>
          <div class="min-h-0 flex-1 overflow-auto rounded-box border border-base-300 bg-base-100 p-3">
            {#if wordBanks.length === 0}
              <div class="text-sm text-base-content/60">{m.tools_audio_asr_word_bank_management_list_empty()}</div>
            {:else}
              <div class="space-y-2">
                {#each wordBanks as bank (bank.id)}
                  <div
                    class:opacity-80={draggedWordbankId === bank.id}
                    class={`rounded-xl border p-3 transition ${
                      bank.id === selectedWordbankId
                        ? "border-primary bg-primary/10"
                        : "border-base-300 bg-base-100 hover:bg-base-200"
                    }`}
                    role="listitem"
                    on:dragover={(event) => {
                      if (!bank.isDefault && draggedWordbankId && draggedWordbankId !== bank.id) {
                        event.preventDefault();
                        updateWordbankDropTarget(event, bank.id);
                      }
                    }}
                    on:dragleave={() => {
                      if (dropTargetWordbankId === bank.id) {
                        dropTargetWordbankId = "";
                        dropTargetWordbankPosition = "";
                      }
                    }}
                    on:drop={(event) => {
                      event.preventDefault();
                      if (!bank.isDefault) {
                        void reorderWordbanks(bank.id, dropTargetWordbankPosition || "before");
                      }
                    }}>
                    {#if dropTargetWordbankId === bank.id && dropTargetWordbankPosition === "before"}
                      <div class="pointer-events-none -mt-4 mb-3 h-0.5 rounded-full bg-primary"></div>
                    {/if}
                    <div class="flex items-start justify-between gap-3">
                      <button class="min-w-0 flex-1 text-left" type="button" on:click={() => selectWordbank(bank.id)}>
                        <span class="font-medium">{renderWordbankName(bank)}</span>
                      </button>
                      <div class="flex shrink-0 flex-wrap items-center justify-end gap-2">
                        {#if !bank.isDefault}
                          <button
                            class="btn cursor-grab btn-ghost btn-xs active:cursor-grabbing"
                            type="button"
                            aria-label={m.tools_audio_asr_word_bank_management_bank_drag_handle_label()}
                            title={m.tools_audio_asr_word_bank_management_bank_drag_handle_label()}
                            draggable={!wordbankSaving}
                            on:dragstart={(event) => handleWordbankDragStart(event, bank.id)}
                            on:dragend={handleWordbankDragEnd}
                            disabled={wordbankSaving}>
                            ::
                          </button>
                        {/if}
                        <button
                          class="btn btn-outline btn-xs"
                          type="button"
                          on:click|stopPropagation={() => openEditDialogFor(bank.id)}
                          disabled={wordbankSaving}>
                          {m.tools_audio_asr_word_bank_management_edit_action()}
                        </button>
                        <button
                          class="btn btn-outline btn-xs btn-warning"
                          type="button"
                          on:click|stopPropagation={() => void clearBank(bank.id)}
                          disabled={wordbankSaving}>
                          {m.tools_audio_asr_word_bank_management_clear_action()}
                        </button>
                        <button
                          class="btn btn-outline btn-xs btn-error"
                          type="button"
                          on:click|stopPropagation={() => void deleteBank(bank.id)}
                          disabled={wordbankSaving || bank.isDefault}>
                          {m.tools_audio_asr_word_bank_management_delete_action()}
                        </button>
                      </div>
                    </div>
                    <div class="mt-1 flex items-center justify-between gap-3">
                      <div class="flex min-w-0 flex-1 items-center gap-2 text-xs text-base-content/60">
                        <span class="min-w-0 flex-1 truncate">
                          {bank.description || m.tools_audio_asr_word_bank_management_description_empty()}
                        </span>
                        <span class="shrink-0 font-mono text-base-content/50">
                          {m.tools_audio_asr_word_bank_management_group_count({ count: bank.entryTotal })}
                        </span>
                      </div>
                      <div class="flex shrink-0 flex-wrap items-center justify-end gap-2">
                        {#if bank.isDefault}
                          <div class="badge badge-outline badge-primary">
                            {m.tools_audio_asr_word_bank_management_default_badge()}
                          </div>
                        {/if}
                        <button
                          class={`badge h-auto badge-outline px-3 py-1 ${
                            bank.isEnabled ? "badge-success" : "badge-neutral"
                          } ${bank.isDefault ? "cursor-default" : "cursor-pointer"}`}
                          type="button"
                          on:click|stopPropagation={() => {
                            if (!bank.isDefault) {
                              void toggleWordbankEnabled(bank.id, !bank.isEnabled);
                            }
                          }}
                          disabled={wordbankSaving || bank.isDefault}>
                          {bank.isEnabled
                            ? m.tools_audio_asr_word_bank_management_enabled_badge()
                            : m.tools_audio_asr_word_bank_management_disabled_badge()}
                        </button>
                      </div>
                    </div>
                    {#if dropTargetWordbankId === bank.id && dropTargetWordbankPosition === "after"}
                      <div class="pointer-events-none mt-3 h-0.5 rounded-full bg-primary"></div>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        </div>
      </div>
    </div>

    <div class="card min-h-0 border border-base-300 bg-base-100 shadow-md">
      <div class="card-body min-h-0 gap-4 overflow-hidden">
        <div class="flex flex-wrap items-end gap-3">
          <form class="flex min-w-0 flex-1 gap-3" on:submit|preventDefault={() => void loadWordbankEntries(wordbankQuery)}>
            <div class="form-control flex-1">
              <div class="label">
                <span class="label-text">{m.tools_audio_asr_word_bank_management_search_label()}</span>
              </div>
              <input
                class="input-bordered input w-full"
                bind:value={wordbankQuery}
                placeholder={m.tools_audio_asr_word_bank_management_search_placeholder()}
                disabled={wordbankLoading || wordbankSaving || !selectedWordbankId} />
            </div>
            <button
              class="btn self-end btn-secondary"
              type="submit"
              disabled={wordbankLoading || wordbankSaving || !selectedWordbankId}>
              {m.tools_audio_asr_word_bank_management_refresh_action()}
            </button>
          </form>

          <div class="badge badge-outline p-4">
            {m.tools_audio_asr_word_bank_management_group_count({ count: wordbankTotal })}
          </div>
        </div>

        {#if wordbankError}
          <div class="alert shrink-0 alert-error">
            <span>{wordbankError}</span>
          </div>
        {/if}

        <div
          bind:this={entriesScrollContainer}
          class="min-h-0 flex-1 overflow-auto rounded-box border border-base-300 bg-base-100 p-4">
          {#if !selectedWordbankId}
            <div class="text-sm text-base-content/60">{m.tools_audio_asr_word_bank_management_select_hint()}</div>
          {:else if wordbankLoading}
            <div class="flex items-center gap-3 text-sm text-base-content/60">
              <span class="loading loading-sm loading-spinner"></span>
              <span>{m.tools_audio_asr_word_bank_management_loading()}</span>
            </div>
          {:else if wordbankEntries.length === 0}
            <div class="text-sm text-base-content/60">{m.tools_audio_asr_word_bank_management_entries_empty()}</div>
          {:else}
            <div class="space-y-3">
              {#each wordbankEntries as entry (entry.key)}
                <div class="rounded-xl border border-base-300 bg-base-100 p-4 shadow-sm">
                  <div class="flex flex-wrap items-center justify-between gap-3">
                    <div>
                      <div class="text-xs tracking-[0.2em] text-base-content/50 uppercase">
                        {m.tools_audio_asr_word_bank_management_pinyin_label()}
                      </div>
                      <div class="mt-1 font-mono text-sm">{entry.key}</div>
                    </div>
                    <div class="flex items-center gap-2">
                      <div class="badge badge-outline">{entry.values.length}</div>
                      <button
                        class="btn btn-outline btn-xs btn-error"
                        type="button"
                        on:click={() => void deleteWordbankEntryGroup(entry.key)}
                        disabled={wordbankSaving}>
                        {m.tools_audio_asr_word_bank_management_group_delete_action()}
                      </button>
                    </div>
                  </div>

                  <div class="mt-4 space-y-2">
                    {#each entry.values as value (`${entry.key}-${value}`)}
                      <div
                        class={`relative flex flex-wrap items-center justify-between gap-3 rounded-lg border px-3 py-2 ${
                          draggedEntryKey === entry.key && draggedValue === value
                            ? "border-primary bg-primary/10"
                            : "border-base-300 bg-base-200"
                        }`}
                        role="listitem"
                        aria-grabbed={draggedEntryKey === entry.key && draggedValue === value}
                        on:dragover={(event) => {
                          if (draggedEntryKey === entry.key) {
                            event.preventDefault();
                            updateDropTarget(event, entry.key, value);
                          }
                        }}
                        on:dragleave={() => {
                          if (dropTargetEntryKey === entry.key && dropTargetValue === value) {
                            dropTargetEntryKey = "";
                            dropTargetValue = "";
                            dropTargetPosition = "";
                          }
                        }}
                        on:drop={(event) => {
                          event.preventDefault();
                          void reorderWordbankGroup(entry.key, value, dropTargetPosition || "before");
                        }}>
                        {#if dropTargetEntryKey === entry.key && dropTargetValue === value && dropTargetPosition === "before"}
                          <div
                            class="pointer-events-none absolute inset-x-3 top-0 h-0.5 -translate-y-1/2 rounded-full bg-primary">
                          </div>
                        {/if}
                        {#if dropTargetEntryKey === entry.key && dropTargetValue === value && dropTargetPosition === "after"}
                          <div
                            class="pointer-events-none absolute inset-x-3 bottom-0 h-0.5 translate-y-1/2 rounded-full bg-primary">
                          </div>
                        {/if}
                        <div class="min-w-0 flex-1 text-sm font-medium wrap-break-word">{value}</div>
                        <div class="flex items-center gap-2">
                          <button
                            class="btn cursor-grab btn-ghost btn-xs active:cursor-grabbing"
                            type="button"
                            aria-label={m.tools_audio_asr_word_bank_management_entry_drag_handle_label()}
                            title={m.tools_audio_asr_word_bank_management_entry_drag_handle_label()}
                            draggable={!wordbankSaving}
                            on:dragstart={(event) => handleEntryDragStart(event, entry.key, value)}
                            on:dragend={handleEntryDragEnd}
                            disabled={wordbankSaving}>
                            ::
                          </button>
                          <button
                            class="btn btn-ghost btn-xs"
                            type="button"
                            on:click={() => beginWordbankEdit(value)}
                            disabled={wordbankSaving}>
                            {m.tools_audio_asr_word_bank_management_entry_edit_title()}
                          </button>
                          <button
                            class="btn btn-outline btn-xs btn-error"
                            type="button"
                            on:click={() => void deleteWordbankEntry(value)}
                            disabled={wordbankSaving}>
                            {m.tools_audio_asr_word_bank_management_entry_delete_action()}
                          </button>
                        </div>
                      </div>
                    {/each}
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      </div>
    </div>
  </div>
</div>

<PageDialog
  bind:open={createDialogOpen}
  title={m.tools_audio_asr_word_bank_management_create_title()}
  showActions={false}
  backdrop={false}
  boxClass="max-w-2xl">
  <form class="space-y-4 pt-4" on:submit|preventDefault={() => void createBank()}>
    <div class="form-control">
      <div class="label">
        <span class="label-text">{m.tools_audio_asr_word_bank_management_name_label()}</span>
      </div>
      <input
        class="input-bordered input w-full"
        bind:value={createWordbankName}
        placeholder={m.tools_audio_asr_word_bank_management_name_placeholder()}
        disabled={wordbankSaving} />
    </div>
    <div class="form-control">
      <div class="label">
        <span class="label-text">{m.tools_audio_asr_word_bank_management_description_label()}</span>
      </div>
      <textarea
        class="textarea-bordered textarea min-h-28 w-full resize-none"
        bind:value={createWordbankDescription}
        placeholder={m.tools_audio_asr_word_bank_management_description_placeholder()}
        disabled={wordbankSaving}></textarea>
    </div>
    <div class="grid gap-4 md:grid-cols-2">
      <div class="form-control">
        <div class="label">
          <span class="label-text">{m.tools_audio_asr_word_bank_management_prefix_label()}</span>
        </div>
        <input
          class="input-bordered input w-full"
          bind:value={createWordbankPrefix}
          placeholder={m.tools_audio_asr_word_bank_management_prefix_placeholder()}
          disabled={wordbankSaving} />
      </div>
      <div class="form-control">
        <div class="label">
          <span class="label-text">{m.tools_audio_asr_word_bank_management_suffix_label()}</span>
        </div>
        <input
          class="input-bordered input w-full"
          bind:value={createWordbankSuffix}
          placeholder={m.tools_audio_asr_word_bank_management_suffix_placeholder()}
          disabled={wordbankSaving} />
      </div>
    </div>
    <div class="modal-action">
      <button class="btn" type="button" on:click={() => (createDialogOpen = false)} disabled={wordbankSaving}>
        {m.msg_cancel()}
      </button>
      <button class="btn btn-primary" type="submit" disabled={wordbankSaving || !createWordbankName.trim()}>
        {m.tools_audio_asr_word_bank_management_create_submit()}
      </button>
    </div>
  </form>
</PageDialog>

<PageDialog
  bind:open={editDialogOpen}
  title={m.tools_audio_asr_word_bank_management_current_title()}
  showActions={false}
  backdrop={false}
  boxClass="max-w-2xl">
  {#if selectedWordbank()}
    <form class="space-y-4 pt-4" on:submit|preventDefault={() => void saveSelectedBank()}>
      <div class="form-control">
        <div class="label">
          <span class="label-text">{m.tools_audio_asr_word_bank_management_name_label()}</span>
        </div>
        <input
          class="input-bordered input w-full"
          bind:value={editWordbankName}
          placeholder={m.tools_audio_asr_word_bank_management_name_placeholder()}
          disabled={wordbankSaving} />
      </div>
      <div class="form-control">
        <div class="label">
          <span class="label-text">{m.tools_audio_asr_word_bank_management_description_label()}</span>
        </div>
        <textarea
          class="textarea-bordered textarea min-h-28 w-full resize-none"
          bind:value={editWordbankDescription}
          placeholder={m.tools_audio_asr_word_bank_management_description_placeholder()}
          disabled={wordbankSaving}></textarea>
      </div>
      <div class="grid gap-4 md:grid-cols-2">
        <div class="form-control">
          <div class="label">
            <span class="label-text">{m.tools_audio_asr_word_bank_management_prefix_label()}</span>
          </div>
          <input
            class="input-bordered input w-full"
            bind:value={editWordbankPrefix}
            placeholder={m.tools_audio_asr_word_bank_management_prefix_placeholder()}
            disabled={wordbankSaving} />
        </div>
        <div class="form-control">
          <div class="label">
            <span class="label-text">{m.tools_audio_asr_word_bank_management_suffix_label()}</span>
          </div>
          <input
            class="input-bordered input w-full"
            bind:value={editWordbankSuffix}
            placeholder={m.tools_audio_asr_word_bank_management_suffix_placeholder()}
            disabled={wordbankSaving} />
        </div>
      </div>
      <div class="modal-action">
        <button class="btn" type="button" on:click={() => (editDialogOpen = false)} disabled={wordbankSaving}>
          {m.msg_cancel()}
        </button>
        <button class="btn btn-secondary" type="submit" disabled={wordbankSaving || !editWordbankName.trim()}>
          {m.tools_audio_asr_word_bank_management_save_submit()}
        </button>
      </div>
    </form>
  {:else}
    <div class="rounded-lg border border-dashed border-base-300 px-4 py-6 text-sm text-base-content/60">
      {m.tools_audio_asr_word_bank_management_select_hint()}
    </div>
  {/if}
</PageDialog>

<PageDialog
  bind:open={entryEditDialogOpen}
  title={m.tools_audio_asr_word_bank_management_entry_edit_title()}
  showActions={false}
  backdrop={false}
  boxClass="max-w-xl">
  {#if wordbankEditOriginal}
    <form class="space-y-4 pt-4" on:submit|preventDefault={() => void saveWordbankEdit()}>
      <div class="rounded-lg border border-base-300 bg-base-200 px-3 py-2 text-sm">
        {wordbankEditOriginal}
      </div>
      <input
        class="input-bordered input w-full"
        bind:value={wordbankEditValue}
        placeholder={m.tools_audio_asr_word_bank_management_entry_edit_placeholder()}
        disabled={wordbankSaving} />
      <div class="modal-action">
        <button class="btn" type="button" on:click={cancelWordbankEdit} disabled={wordbankSaving}>
          {m.tools_audio_asr_word_bank_management_entry_edit_cancel()}
        </button>
        <button class="btn btn-secondary" type="submit" disabled={wordbankSaving || !wordbankEditValue.trim()}>
          {m.tools_audio_asr_word_bank_management_entry_edit_submit()}
        </button>
      </div>
    </form>
  {/if}
</PageDialog>
