import type { Api } from '../lib/ipc';
import type { DropdownOption, DropdownState, SessionView } from '../lib/types';

/** What a dropdown starts on, apart from its first option. */
export interface Memory {
  /** The values used before, newest first: the list starts on the first that is still offered. */
  prefer: string[];
  /** May be left empty: it starts on nothing, whatever was used before. */
  optional: boolean;
}

/**
 * A dropdown that has its list now: a choice already made is kept, else the last one used that is
 * still offered, else the first option. An optional one starts on nothing.
 */
export function withOptions(
  state: DropdownState,
  options: DropdownOption[],
  chosen: string | null,
  memory: Memory = { prefer: [], optional: false },
): DropdownState {
  const keep = chosen !== null && options.some((o) => o.value === chosen);
  const remembered = memory.prefer.find((v) => options.some((o) => o.value === v));
  const start = memory.optional ? '' : (remembered ?? options[0]?.value ?? null);
  return {
    status: 'ready',
    options,
    selected: keep ? chosen : start,
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
  memoryOf: (blockId: string) => Memory = () => ({ prefer: [], optional: false }),
): Promise<void> {
  const loading = Object.entries(view.dropdowns).filter(([, d]) => d.status === 'loading');
  await Promise.all(
    loading.map(async ([blockId]) => {
      try {
        const options = await api.fetchOptions(view.session_id, blockId);
        if (isCurrent()) apply(blockId, (cur) => withOptions(cur, options, cur.selected, memoryOf(blockId)));
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
