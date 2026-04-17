import { type Tab } from "$lib/tabs";
import { m } from "$lib/i18n/paraglide/messages";

export default {
  name: "about",
  label: m.page_about(),
  path: "/about",
  home: false
} as Tab;
