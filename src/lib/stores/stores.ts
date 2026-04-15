import { get as getValue, writable, type Writable } from "svelte/store";

export interface StoresState<T> extends Writable<T> {
  get(): T;
  reset(): void;
  clear(): void;
}

/**
 * Create a persisted store.
 * @param key key in localStorage
 * @param defaultValue store default value
 * @param init init function
 */
export function persistedStore<T>(
  key: string,
  defaultValue: T,
  init?: (value: T) => void
): StoresState<T> {
  const isString = typeof defaultValue === "string";
  const persistedValue = localStorage.getItem(key);
  const initialValue: T = persistedValue
    ? isString
      ? persistedValue
      : { ...defaultValue, ...JSON.parse(persistedValue) }
    : defaultValue;
  const store = writable<T>(initialValue);
  init?.(initialValue);
  store.subscribe((value) => {
    localStorage.setItem(key, typeof value === "string" ? value : JSON.stringify(value));
  });
  const clear = () => {
    localStorage.removeItem(key);
  };
  const reset = () => {
    localStorage.setItem(key, isString ? defaultValue : JSON.stringify(defaultValue));
  };
  const get = () => {
    return getValue(store);
  };
  return {
    ...store,
    get,
    clear,
    reset
  };
}
