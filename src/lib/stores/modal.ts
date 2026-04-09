import { writable } from "svelte/store";

export type ModalType =
  | "info"
  | "warning"
  | "error"
  | "success"
  | "failed"
  | "suspend"
  | "complete";

export interface ModalOptions {
  title?: string;
  message: string;
  type?: ModalType;
  backdrop?: boolean;
  confirmText?: string;
  cancelText?: string;
  onConfirm?: () => void;
  onCancel?: () => void;
}

type ModalState = {
  isOpen: boolean;
  options: ModalOptions | null;
};

const initialState: ModalState = {
  isOpen: false,
  options: null
};

export const modalStore = writable<ModalState>(initialState);

export function openModal(options: ModalOptions) {
  modalStore.set({
    isOpen: true,
    options
  });
}

export function closeModal() {
  modalStore.update((state) => ({
    ...state,
    isOpen: false
  }));
}
