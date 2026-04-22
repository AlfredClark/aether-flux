export function parseShortcutFromKeyboardEvent(event: KeyboardEvent) {
  if (
    event.key === "Meta" ||
    event.key === "Control" ||
    event.key === "Alt" ||
    event.key === "Shift"
  ) {
    return null;
  }

  const modifiers: string[] = [];
  if (event.ctrlKey || event.metaKey) {
    modifiers.push("CommandOrControl");
  }
  if (event.altKey) {
    modifiers.push("Alt");
  }
  if (event.shiftKey) {
    modifiers.push("Shift");
  }

  if (modifiers.length === 0) {
    return null;
  }

  const normalizedKey = normalizeShortcutKey(event.key);
  if (!normalizedKey) {
    return null;
  }

  return [...modifiers, normalizedKey].join("+");
}

export function matchesShortcutEvent(event: KeyboardEvent, shortcut: string) {
  const parts = shortcut
    .split("+")
    .map((part) => part.trim())
    .filter(Boolean);

  if (parts.length === 0) {
    return false;
  }

  const mainKey = parts[parts.length - 1];
  const modifiers = new Set(parts.slice(0, -1));

  const expectsCommandOrControl = modifiers.has("CommandOrControl");
  const expectsAlt = modifiers.has("Alt");
  const expectsShift = modifiers.has("Shift");

  if (expectsCommandOrControl !== (event.ctrlKey || event.metaKey)) {
    return false;
  }
  if (expectsAlt !== event.altKey) {
    return false;
  }
  if (expectsShift !== event.shiftKey) {
    return false;
  }

  return normalizeShortcutKey(event.key) === mainKey;
}

function normalizeShortcutKey(key: string) {
  if (key === " ") return "Space";
  if (key === "ArrowUp") return "Up";
  if (key === "ArrowDown") return "Down";
  if (key === "ArrowLeft") return "Left";
  if (key === "ArrowRight") return "Right";
  if (key === "Escape") return "Escape";
  if (key === "Enter") return "Enter";
  if (key === "Tab") return "Tab";
  if (key === "Backspace") return "Backspace";
  if (key === "Delete") return "Delete";

  if (/^F\\d{1,2}$/i.test(key)) {
    return key.toUpperCase();
  }

  if (/^[a-zA-Z]$/.test(key)) {
    return key.toUpperCase();
  }

  if (/^[0-9]$/.test(key)) {
    return key;
  }

  return null;
}
