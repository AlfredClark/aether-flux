import type { Handle } from "@sveltejs/kit";
import { getTextDirection } from "$lib/i18n/paraglide/runtime";
import { paraglideMiddleware } from "$lib/i18n/paraglide/server";

const handleParaglide: Handle = ({ event, resolve }) =>
  paraglideMiddleware(event.request, ({ request, locale }) => {
    event.request = request;

    return resolve(event, {
      transformPageChunk: ({ html }) =>
        html
          .replace("%paraglide.lang%", locale)
          .replace('dir="ltr" data-paraglide-dir', `dir="${getTextDirection(locale)}"`)
    });
  });

export const handle: Handle = handleParaglide;
