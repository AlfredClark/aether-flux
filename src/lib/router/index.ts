import type { Pathname } from "$app/types";
import { m } from "$lib/i18n/paraglide/messages";

type RouterItem = {
  label: string;
  path: Pathname;
  home: boolean;
};

type Router = RouterItem[];

export function getRouter(): Router {
  return [
    { path: "/", label: m.home(), home: true },
    { path: "/settings", label: m.settings(), home: false },
    { path: "/about", label: m.about(), home: false }
  ];
}
