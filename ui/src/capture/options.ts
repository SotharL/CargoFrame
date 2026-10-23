import type { Api } from '../lib/ipc';
import type { DropdownOption, DropdownState, SessionView } from '../lib/types';

/** A dropdown that has its list now: first option chosen unless one was chosen already. */
export function withOptions(
  state: DropdownState,
  options: DropdownOption[],
  chosen: string | null,
): DropdownState {
  const keep = chosen !== null && options.some((o) => o.value === chosen);
  return {
    status: 'ready',
    options,
    selected: keep ? chosen : (options[0]?.value ?? null),
    message: options.length === 0 ? 'The service returned no options' : null,
  };
}

/** A dropdown whose list could not be loaded, with a sentence saying why. */
export function withFailure(
  state: DropdownState,
  message: string,
  reconnect: boolean,
): DropdownState {
  return {
    ...state,
    status: 'needs_connection',
    options: [],
    selected: null,
    message: reconnect ? `${message} Reconnect it in Settings.` : message,
  };
}

/**
 * Fetch the list of every dropdown that is still loading. `apply` is called once per dropdown as
 * its answer arrives (they load side by side); an answer for a capture that has since been
 * replaced is dropped.
 */
export async function loadDropdowns(
  api: Pick<Api, 'fetchOptions'>,
  view: SessionView,
  isCurrent: () => boolean,
  apply: (blockId: string, next: (current: DropdownState) => DropdownState) => void,
): Promise<void> {
  const loading = Object.entries(view.dropdowns).filter(([, d]) => d.status === 'loading');
  await Promise.all(
    loading.map(async ([blockId]) => {
      try {
        const options = await api.fetchOptions(view.session_id, blockId);
        if (isCurrent()) apply(blockId, (cur) => withOptions(cur, options, cur.selected));
      } catch (e) {
        const err =
          e && typeof e === 'object' && 'message' in e
            ? (e as { message: string; reconnect?: boolean })
            : { message: String(e), reconnect: false };
        if (isCurrent()) {
          apply(blockId, (cur) => withFailure(cur, err.message, err.reconnect === true));
        }
      }
    }),
  );
}
