import { getApi } from './ipc';
import type { ItemEntry, ItemKind } from './types';

/** One kind of configuration item as the Settings pages see it: the list, and how to change it. */
export function createItems<T>(kind: ItemKind) {
  let items = $state<ItemEntry<T>[]>([]);
  let problems = $state<string[]>([]);
  let loaded = $state(false);
  let failure = $state('');

  async function reload() {
    try {
      const list = await (await getApi()).listItems<T>(kind);
      items = list.items;
      problems = list.problems;
      failure = '';
    } catch (e) {
      failure = String(e);
    }
    loaded = true;
  }

  return {
    get items() { return items; },
    get problems() { return problems; },
    get loaded() { return loaded; },
    get failure() { return failure; },
    reload,
    /** Save under `id`, or under a new id made from the item's name when `id` is null. */
    async save(id: string | null, item: T & { name: string }): Promise<string> {
      const api = await getApi();
      const target = id ?? (await api.newItemId(kind, item.name));
      await api.saveItem(kind, target, item);
      await reload();
      return target;
    },
    async remove(id: string) {
      await (await getApi()).deleteItem(kind, id);
      await reload();
    },
    find(id: string | null | undefined): T | undefined {
      return id ? items.find((i) => i.id === id)?.item : undefined;
    },
  };
}
