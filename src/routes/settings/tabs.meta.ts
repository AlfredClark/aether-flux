import { type Tab } from "$lib/tabs";
import { m } from "$lib/i18n/paraglide/messages";

export default {
  name: "settings",
  label: m.page_settings(),
  path: "/settings",
  home: false
} as Tab;
