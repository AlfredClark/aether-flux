import { type Tab } from "$lib/tabs";
import { m } from "$lib/i18n/paraglide/messages";

export default {
  name: "home",
  label: m.page_home(),
  path: "/",
  home: true
} as Tab;
