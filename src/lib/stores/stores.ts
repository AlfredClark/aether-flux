import { get as getValue, writable, type Writable } from "svelte/store";

export interface StoresState<T> extends Writable<T> {
  get(): T;
  reset(): void;
  clear(): void;
}

function initFromLocalStorage<T>(defaultValue: T, localStorageValue: string | null): T {
  const isPrimitive = defaultValue === null || typeof defaultValue !== "object";

  if (isPrimitive) {
    if (localStorageValue === null || localStorageValue === undefined) {
      return defaultValue;
    }

    if (defaultValue === null) {
      return (localStorageValue === "null" ? null : defaultValue) as T;
    }

    if (typeof defaultValue === "boolean") {
      if (localStorageValue === "true") return true as T;
      if (localStorageValue === "false") return false as T;
      return defaultValue;
    }

    if (typeof defaultValue === "number") {
      const parsed = Number(localStorageValue);
      return (Number.isNaN(parsed) ? defaultValue : parsed) as T;
    }

    if (typeof defaultValue === "bigint") {
      try {
        return BigInt(localStorageValue) as T;
      } catch {
        return defaultValue;
      }
    }

    if (typeof defaultValue === "string") {
      return localStorageValue as T;
    }

    return defaultValue;
  }

  try {
    JSON.stringify(defaultValue);
  } catch {
    return defaultValue;
  }

  let parsedLocal = null;
  if (typeof localStorageValue === "string") {
    try {
      parsedLocal = JSON.parse(localStorageValue);
    } catch {
      parsedLocal = null;
    }
  } else if (localStorageValue !== null && typeof localStorageValue === "object") {
    parsedLocal = localStorageValue;
  }

  if (parsedLocal !== null && typeof parsedLocal === "object") {
    return { ...defaultValue, ...parsedLocal };
  }

  return defaultValue;
}

function toLocalStorageString(value: unknown): string {
  if (value === null) return "null";
  if (value === undefined) return "undefined";

  const type = typeof value;

  if (
    type === "string" ||
    type === "number" ||
    type === "boolean" ||
    type === "bigint" ||
    type === "symbol"
  ) {
    return String(value);
  }

  try {
    const json = JSON.stringify(value);
    return json !== undefined ? json : String(value);
  } catch {
    return String(value);
  }
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
  const persistedValue = localStorage.getItem(key);
  const initialValue: T = initFromLocalStorage<T>(defaultValue, persistedValue);
  const store = writable<T>(initialValue);
  init?.(initialValue);
  store.subscribe((value) => {
    localStorage.setItem(key, toLocalStorageString(value));
  });
  const clear = () => {
    localStorage.removeItem(key);
  };
  const reset = () => {
    localStorage.setItem(key, toLocalStorageString(defaultValue));
  };
  const get = () => {
    return getValue(store) as T;
  };
  return {
    ...store,
    get,
    clear,
    reset
  };
}
