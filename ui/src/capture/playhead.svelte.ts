// Where the clip being watched is, so a log viewer can show the lines of that moment. The Video
// block sets it while the clip plays (and when it is moved); `at` stays null until it has been
// played once.

export const playhead = $state<{ at: number | null; playing: boolean }>({ at: null, playing: false });

/** The clip moved to `at` (ms since 1970). Small steps are let go, so the log is not read again
 *  for every picture. */
export function movePlayhead(at: number, playing: boolean) {
  if (playhead.playing !== playing || playhead.at === null || Math.abs(playhead.at - at) >= 100) {
    playhead.at = at;
    playhead.playing = playing;
  }
}

export function clearPlayhead() {
  playhead.at = null;
  playhead.playing = false;
}
