<script lang="ts">
  // The guide on top of Settings: a box in the accent colour around what the step points at, and
  // one short sentence under it. It opens the page the step belongs to, waits for the thing to
  // appear, and follows it if the page moves. Esc closes it.
  import { onDestroy, untrack } from 'svelte';
  import { go, route } from '../route.svelte';
  import { guide, stopTour } from './tourState.svelte';
  import { selectorOf } from './tours';

  /** How long to wait for the target to appear before saying it is not on the screen yet. */
  const WAIT_MS = 2500;
  const PAD = 5;
  /** How long a step insists on its own page after it starts. */
  const SETTLE_MS = 1500;

  const tour = $derived(guide.tour);
  const step = $derived(tour ? tour.steps[guide.index] : undefined);
  const last = $derived(tour ? guide.index === tour.steps.length - 1 : false);

  let box = $state<{ left: number; top: number; width: number; height: number } | null>(null);
  let waiting = $state(false);
  let el: Element | null = null;
  let timer: ReturnType<typeof setInterval> | undefined;

  function measure() {
    if (!el || !el.isConnected) {
      el = step ? document.querySelector(selectorOf(step.target)) : null;
    }
    if (!el) {
      box = null;
      return;
    }
    const r = el.getBoundingClientRect();
    box = { left: r.left - PAD, top: r.top - PAD, width: r.width + 2 * PAD, height: r.height + 2 * PAD };
  }

  function stopWatching() {
    if (timer) clearInterval(timer);
    timer = undefined;
    el = null;
    box = null;
    waiting = false;
  }

  // For each step: open its page, wait for its target, bring it into view, then follow it.
  $effect(() => {
    const s = step;
    stopWatching();
    if (!s) return;
    untrack(() => {
      if (s.page && route.page !== s.page) go(s.page);
    });
    const started = Date.now();
    waiting = true;
    let scrolled = false;
    timer = setInterval(() => {
      measure();
      // A button pressed by the step can move the page a moment later (a save that shows the
      // layout it made): for the first moments the step's own page wins.
      if (!el && s.page && Date.now() - started < SETTLE_MS) {
        untrack(() => {
          if (route.page !== s.page) go(s.page!);
        });
      }
      if (el && !scrolled) {
        scrolled = true;
        waiting = false;
        el.scrollIntoView?.({ block: 'center', behavior: 'smooth' });
      }
      if (!el && Date.now() - started > WAIT_MS) waiting = false;
    }, 100);
    measure();
    return stopWatching;
  });
  onDestroy(stopWatching);

  let pressing = false;
  async function next() {
    if (!tour || !step || pressing) return;
    // looked up now: the timer may not have found it yet
    const target = document.querySelector(selectorOf(step.target));
    if (step.click && target instanceof HTMLElement) {
      pressing = true;
      target.click();
      // what the button does (a save that shows what it made) settles before the next step looks
      await new Promise((done) => setTimeout(done, SETTLE_MS / 5));
      pressing = false;
    }
    if (guide.tour !== tour) return;
    if (last) stopTour();
    else guide.index += 1;
  }
  const back = () => {
    if (guide.index > 0) guide.index -= 1;
  };
  function onKey(e: KeyboardEvent) {
    if (tour && e.key === 'Escape') {
      e.preventDefault();
      stopTour();
    }
  }

  // The sentence goes under the box, or above it when there is no room below.
  const caption = $derived.by(() => {
    if (!box) return null;
    const below = box.top + box.height + 10;
    const room = typeof window === 'undefined' ? 800 : window.innerHeight - below;
    const left = Math.max(8, Math.min(box.left, (typeof window === 'undefined' ? 1000 : window.innerWidth) - 340));
    return room > 90 ? { left, top: below, above: false } : { left, top: box.top - 10, above: true };
  });
</script>

<svelte:window onkeydown={onKey} />

{#if tour && step}
  {#if box}
    <div class="box" style:left="{box.left}px" style:top="{box.top}px" style:width="{box.width}px" style:height="{box.height}px"></div>
  {/if}
  <div
    class="say"
    class:above={caption?.above}
    class:fixed={!caption}
    role="dialog"
    aria-label={tour.title}
    style:left={caption ? `${caption.left}px` : undefined}
    style:top={caption ? `${caption.top}px` : undefined}
  >
    <p>{step.say}{#if !box && !waiting} <span class="later">(Not on the screen yet: do the step before, then press Next.)</span>{/if}</p>
    <div class="row">
      <span class="count">{guide.index + 1} / {tour.steps.length}</span>
      {#if guide.index > 0}<button type="button" onclick={back}>Back</button>{/if}
      <button type="button" class="go" onclick={next}>{last ? 'Done' : 'Next'}</button>
      <button type="button" aria-label="Close the guide" onclick={stopTour}>✕</button>
    </div>
  </div>
{/if}

<style>
  .box {
    position: fixed; z-index: 100; pointer-events: none; box-sizing: border-box;
    border: 2px solid var(--accent); border-radius: 8px;
    box-shadow: 0 0 0 4px color-mix(in srgb, var(--accent) 28%, transparent);
    transition: left 120ms, top 120ms, width 120ms, height 120ms;
  }
  .say {
    position: fixed; z-index: 101; width: 320px; max-width: calc(100vw - 16px); box-sizing: border-box;
    padding: 8px 12px; border-radius: 8px; background: var(--accent); color: var(--on-accent);
    box-shadow: 0 6px 18px color-mix(in srgb, #000 25%, transparent);
    font-size: 13px; display: grid; gap: 6px;
  }
  .say.above { transform: translateY(-100%); }
  .say.fixed { left: 50%; bottom: 24px; transform: translateX(-50%); }
  p { margin: 0; }
  .later { opacity: 0.85; }
  .row { display: flex; gap: 6px; align-items: center; justify-content: flex-end; }
  .count { margin-right: auto; font-size: 12px; opacity: 0.85; }
  button {
    border: 1px solid color-mix(in srgb, var(--on-accent) 55%, transparent);
    background: transparent; color: inherit; padding: 2px 10px; border-radius: 6px; font-size: 12.5px;
  }
  button:hover { background: color-mix(in srgb, var(--on-accent) 16%, transparent); }
  .go { background: var(--on-accent); color: var(--accent); font-weight: 600; }
  .go:hover { background: var(--on-accent); opacity: 0.9; }
</style>
