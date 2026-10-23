import { resolvePage, type PageId } from './nav';

const KEY = 'cargoframe.settings.page';

/** Which Settings page is showing, and optionally which item on it (for example a layout id). */
export const route = $state<{ page: PageId; arg: string | null }>({ page: 'general', arg: null });

export function go(page: PageId, arg: string | null = null) {
  route.page = page;
  route.arg = arg;
  try { localStorage.setItem(KEY, page); } catch { /* storage can be unavailable */ }
}

/** `layouts/bug` or `rules` as sent by the backend or the address bar. */
export function parseTarget(target: string | null | undefined): { page: PageId; arg: string | null } | null {
  if (!target) return null;
  const [name, ...rest] = target.split('/');
  const page = resolvePage(name);
  if (!page) return null;
  return { page, arg: rest.join('/') || null };
}

/** The page to start on: the address (`?page=`), else the last page used, else General. */
export function restore(search: string) {
  const fromUrl = parseTarget(new URLSearchParams(search).get('page'));
  if (fromUrl) {
    route.page = fromUrl.page;
    route.arg = fromUrl.arg;
    return;
  }
  try {
    const saved = resolvePage(localStorage.getItem(KEY));
    if (saved) route.page = saved;
  } catch { /* ignore */ }
}
