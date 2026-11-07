// The last few choices made in a dropdown, kept on this computer so the list can start on the
// last one used and show the recent ones first. Only values and labels are kept, never secrets.

export const KEEP = 5;
const PREFIX = 'cargoframe.recent.';

export interface Recent {
  value: string;
  label: string;
}

/** Where the posts used inside one choice are kept: a channel has its own, not shared with others. */
export const deeperKey = (blockId: string, parent: string) => `${blockId}@${parent}`;

const key = (layoutId: string, blockId: string) => `${PREFIX}${layoutId}.${blockId}`;

export function recentChoices(layoutId: string | null | undefined, blockId: string): Recent[] {
  if (!layoutId) return [];
  try {
    const raw = JSON.parse(localStorage.getItem(key(layoutId, blockId)) ?? '[]') as unknown;
    if (!Array.isArray(raw)) return [];
    return raw
      .filter((r): r is Recent => !!r && typeof r.value === 'string' && typeof r.label === 'string')
      .slice(0, KEEP);
  } catch {
    return [];
  }
}

/** Put a choice first; the same choice never appears twice, and only the last five stay. */
export function remember(layoutId: string | null | undefined, blockId: string, choice: Recent) {
  if (!layoutId || !choice.value) return;
  const next = [choice, ...recentChoices(layoutId, blockId).filter((r) => r.value !== choice.value)].slice(0, KEEP);
  try {
    localStorage.setItem(key(layoutId, blockId), JSON.stringify(next));
  } catch {
    /* storage can be unavailable: the list simply starts on its first entry */
  }
}


/** Forget every remembered choice (Settings, General, Your data). Returns how many lists went. */
export function forgetRecent(): number {
  try {
    const mine: string[] = [];
    for (let i = 0; i < localStorage.length; i++) {
      const k = localStorage.key(i);
      if (k?.startsWith(PREFIX)) mine.push(k);
    }
    for (const k of mine) localStorage.removeItem(k);
    return mine.length;
  } catch {
    return 0;
  }
}

/** How many dropdowns have remembered choices. */
export function remembered(): number {
  try {
    let n = 0;
    for (let i = 0; i < localStorage.length; i++) if (localStorage.key(i)?.startsWith(PREFIX)) n++;
    return n;
  } catch {
    return 0;
  }
}
