import type { Reroute } from "@sveltejs/kit";
import { deLocalizeUrl } from "$lib/i18n/paraglide/runtime";

export const reroute: Reroute = (request) => deLocalizeUrl(request.url).pathname;
