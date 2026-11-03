import type { ServiceKind } from './types';

/** What a sharing button looks like and is called, by the destination it sends to. */
export type KindKey = ServiceKind | 'local';

export const KIND_NAME: Record<KindKey, string> = {
  jira: 'Jira',
  google: 'Google',
  reddit: 'Reddit',
  claude: 'Claude',
  slack: 'Slack',
  discord: 'Discord',
  teams: 'Teams',
  custom: 'Custom API',
  local: 'This computer',
};

/** `null` is a button with no active connection: it saves a zip here. */
export function kindKey(kind: ServiceKind | null | undefined): KindKey {
  return kind ?? 'local';
}

/**
 * Inline style that gives an element its destination's colours: `--kind` for fills (white text
 * stays readable on it) and `--kind-line` for outlines that read on the page background.
 */
export function kindStyle(kind: ServiceKind | null | undefined): string {
  const k = kindKey(kind);
  return `--kind:var(--kind-${k});--kind-line:var(--kind-${k}-line);`;
}

/**
 * The destination a sharing button really sends to: its connection, when that exists and is
 * signed in (an expired sign-in still counts, as in the backend). `null`: it saves a zip here.
 */
export function buttonKind(
  connectionId: string | null | undefined,
  connections: { id: string; kind: ServiceKind; state: string }[],
): ServiceKind | null {
  const c = connections.find((x) => x.id === connectionId);
  return c && c.state !== 'disconnected' ? c.kind : null;
}
