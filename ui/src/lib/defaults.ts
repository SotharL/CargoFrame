// The settings a new install starts with, as Rust has them: `cargo test` writes
// generated/defaults.json from `General::default()`, `Video::default()` and `Safe::default()`
// (core/config/src/models.rs), so the window never keeps a copy of its own. Each call is a new
// object, safe to change.
import raw from './generated/defaults.json';
import type { General, Safe, Video } from './types';

const all = raw as unknown as { general: General; video: Video; safe: Safe };
const copy = <T,>(v: T): T => JSON.parse(JSON.stringify(v)) as T;

export const defaultGeneral = (): General => copy(all.general);
export const defaultVideo = (): Video => copy(all.video);
export const defaultSafe = (): Safe => copy(all.safe);
