import type { SessionView } from '../lib/types';

/** The second step of a path: empty `value` is "start something new", else a post's id or link. */
export interface DeeperPick {
  value: string;
  label: string;
}

/** What the person has typed or chosen in the window so far. */
export interface FormValues {
  /** Text block id to its current text. */
  texts: Record<string, string>;
  /** Dropdown block id to the chosen option value. */
  dropdowns: Record<string, string | null>;
  /** Dropdown block id to its second step, for a dropdown that goes deeper. */
  deeper: Record<string, DeeperPick>;
  /** Files block id to the ticked file paths. */
  files: Record<string, string[]>;
}

/** Starting values: rendered default texts, first dropdown options, preselected files. */
export function initialValues(view: SessionView): FormValues {
  const texts: Record<string, string> = {};
  for (const [id, t] of Object.entries(view.texts)) texts[id] = t.value;
  const dropdowns: Record<string, string | null> = {};
  for (const [id, d] of Object.entries(view.dropdowns)) dropdowns[id] = d.selected;
  const files: Record<string, string[]> = {};
  for (const [id, f] of Object.entries(view.files)) {
    files[id] = f.entries.filter((e) => e.selected).map((e) => e.path);
  }
  return { texts, dropdowns, deeper: {}, files };
}

export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(bytes < 10 * 1024 ? 1 : 0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Tick or untick one file. */
export function toggleFile(selected: string[], path: string, on: boolean): string[] {
  const without = selected.filter((p) => p !== path);
  return on ? [...without, path] : without;
}
