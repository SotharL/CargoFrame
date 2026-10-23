import { isPageId, type PageId } from './nav';

const KEY = 'cargoframe.settings.page';

/** Which Settings page is showing, and optionally which item on it (for example a layout id). */
export const route = $state<{ page: PageId; arg: string | null }>({ page: 'triggers', arg: null });

export function go(page: PageId, arg: string | null = null) {
  route.page = page;
  route.arg = arg;
  try { localStorage.setItem(KEY, page); } catch { /* storage can be unavailable */ }
}

/** `layouts/bug` or `rules` as sent by the backend or the address bar. */
export function parseTarget(target: string | null | undefined): { page: PageId; arg: string | null } | null {
  if (!target) return null;
  const [page, ...rest] = target.split('/');
  if (!isPageId(page ?? null)) return null;
  return { page: page as PageId, arg: rest.join('/') || null };
}

/** The page to start on: the address (`?page=`), else the last page used, else the first. */
export function restore(search: string) {
  const fromUrl = parseTarget(new URLSearchParams(search).get('page'));
  if (fromUrl) {
    route.page = fromUrl.page;
    route.arg = fromUrl.arg;
    return;
  }
  try {
    const saved = localStorage.getItem(KEY);
    if (isPageId(saved)) route.page = saved;
  } catch { /* ignore */ }
}
