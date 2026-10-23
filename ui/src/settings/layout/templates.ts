import type { ConnectionView, LayoutTemplate } from '../../lib/types';

/** One template offered for one connection (or for none, for the local one). */
export interface TemplateOffer {
  key: string;
  name: string;
  description: string;
  connectionId: string | null;
}

/** Templates grouped under the connection they would be built for. */
export interface TemplateGroup {
  id: string;
  title: string;
  /** `ready`: signed in. `needs_sign_in`: exists but cannot send yet. `local`: no connection. */
  state: 'ready' | 'needs_sign_in' | 'local';
  /** Said next to the title: the service, and why it cannot send yet. */
  detail: string;
  offers: TemplateOffer[];
}

const STATE_DETAIL: Record<ConnectionView['state'], string> = {
  connected: 'signed in',
  disconnected: 'not signed in yet',
  expired: 'sign-in expired',
  disabled: 'turned off',
};

/**
 * The starting points that suit the connections the person has. Signed-in connections come
 * first, then the one that saves locally (which needs no account), then connections that cannot
 * send yet: a layout built for one of those works as soon as it is signed in, and until then
 * its button reads "Save locally".
 */
export function offersFor(templates: LayoutTemplate[], connections: ConnectionView[]): TemplateGroup[] {
  const byName = (a: ConnectionView, b: ConnectionView) => a.name.localeCompare(b.name);
  const forConnection = (c: ConnectionView): TemplateGroup => ({
    id: c.id,
    title: c.name || c.id,
    state: c.state === 'connected' ? 'ready' : 'needs_sign_in',
    detail: `${c.kind_label}, ${STATE_DETAIL[c.state]}`,
    offers: templates
      .filter((t) => t.service === c.kind)
      .map((t) => ({ key: t.key, name: t.name, description: t.description, connectionId: c.id })),
  });
  const local: TemplateGroup = {
    id: '',
    title: 'No connection',
    state: 'local',
    detail: 'saves a zip on this computer',
    offers: templates
      .filter((t) => t.service === null)
      .map((t) => ({ key: t.key, name: t.name, description: t.description, connectionId: null })),
  };
  const ready = connections.filter((c) => c.state === 'connected').sort(byName).map(forConnection);
  const waiting = connections.filter((c) => c.state !== 'connected').sort(byName).map(forConnection);
  return [...ready, local, ...waiting].filter((g) => g.offers.length > 0);
}
