import type { PathnameWithSearchOrHash } from "$app/types";
import { m } from "$lib/i18n/paraglide/messages";

export type Router = {
  name: string;
  path: PathnameWithSearchOrHash;
  default: boolean;
};

export function getRouter(): Router[] {
  return [
    { path: "/home", name: m.home(), default: true },
    { path: "/toolbox", name: m.toolbox(), default: false },
    { path: "/plugins", name: m.plugins(), default: false },
    { path: "/settings", name: m.settings(), default: false },
    { path: "/about", name: m.about(), default: false }
  ];
}
