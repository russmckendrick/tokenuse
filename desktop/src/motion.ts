import { animate, stagger } from 'motion';

type MotionControls = {
  cancel?: () => void;
  stop?: () => void;
};

type RevealParams = {
  delay?: number;
  duration?: number;
  y?: number;
};

type StaggerParams = RevealParams & {
  selector?: string;
  stagger?: number;
};

type BarParams = {
  value: number;
  duration?: number;
};

type ChartRefreshParams = {
  duration?: number;
  y?: number;
};

type PillParams = {
  duration?: number;
};

type SegmentIndicatorParams = {
  active: HTMLElement | null;
  duration?: number;
};

const reduceQuery = '(prefers-reduced-motion: reduce)';

function reducedMotion() {
  return typeof window !== 'undefined' && window.matchMedia(reduceQuery).matches;
}

function stopAnimation(animation: MotionControls | null) {
  animation?.cancel?.();
  animation?.stop?.();
}

function frame(callback: () => void) {
  const id = window.requestAnimationFrame(callback);
  return () => window.cancelAnimationFrame(id);
}

export function reveal(node: HTMLElement, params: RevealParams = {}) {
  if (reducedMotion()) return {};

  const y = params.y ?? 5;
  const cancelFrame = frame(() => {
    animate(
      node,
      { opacity: [0, 1], transform: [`translateY(${y}px)`, 'translateY(0px)'] },
      { delay: params.delay ?? 0, duration: params.duration ?? 0.24, ease: 'easeOut' }
    );
  });

  return {
    destroy() {
      cancelFrame();
    }
  };
}

export function fadeIn(node: HTMLElement, params: RevealParams = {}) {
  if (reducedMotion()) return {};

  let animation: MotionControls | null = null;
  const cancelFrame = frame(() => {
    animation = animate(
      node,
      { opacity: [0, 1] },
      { delay: params.delay ?? 0, duration: params.duration ?? 0.18, ease: 'easeOut' }
    ) as MotionControls;
  });

  return {
    destroy() {
      cancelFrame();
      stopAnimation(animation);
    }
  };
}

export function staggeredReveal(node: HTMLElement, params: StaggerParams = {}) {
  if (reducedMotion()) return {};

  let animation: MotionControls | null = null;
  const cancelFrame = frame(() => {
    const targets = Array.from(node.querySelectorAll<HTMLElement>(params.selector ?? ':scope > *'));
    if (!targets.length) return;

    animation = animate(
      targets,
      {
        opacity: [0, 1],
        transform: [`translateY(${params.y ?? 4}px)`, 'translateY(0px)']
      },
      {
        delay: stagger(params.stagger ?? 0.025, { startDelay: params.delay ?? 0 }),
        duration: params.duration ?? 0.22,
        ease: 'easeOut'
      }
    ) as MotionControls;
  });

  return {
    destroy() {
      cancelFrame();
      stopAnimation(animation);
    }
  };
}

export function animatedBar(node: HTMLElement, params: BarParams) {
  let previous = Math.max(0, Math.min(100, params.value));
  node.style.transformOrigin = 'left center';
  node.style.transform = `scaleX(${previous / 100})`;

  return {
    update(next: BarParams) {
      const value = Math.max(0, Math.min(100, next.value));
      if (reducedMotion()) {
        node.style.transform = `scaleX(${value / 100})`;
        previous = value;
        return;
      }

      animate(
        node,
        { transform: [`scaleX(${previous / 100})`, `scaleX(${value / 100})`] },
        { duration: next.duration ?? 0.28, ease: 'easeOut' }
      );
      previous = value;
    }
  };
}

export function chartRefresh(node: HTMLElement, params: ChartRefreshParams = {}) {
  if (reducedMotion()) return {};

  const y = params.y ?? 3;
  let animation: MotionControls | null = null;

  node.style.opacity = '0';
  node.style.transform = `translateY(${y}px)`;

  const cancelFrame = frame(() => {
    animation = animate(
      node,
      { opacity: [0, 1], transform: [`translateY(${y}px)`, 'translateY(0px)'] },
      { duration: params.duration ?? 0.18, ease: 'easeOut' }
    ) as MotionControls;
  });

  return {
    destroy() {
      cancelFrame();
      stopAnimation(animation);
    }
  };
}

/**
 * Page-level cross-fade applied to the active view container when the user
 * switches pages. Short, calm, no slide — matches the "decel" easing.
 */
export function pageTransition(node: HTMLElement, params: RevealParams = {}) {
  if (reducedMotion()) return {};

  let animation: MotionControls | null = null;
  const cancelFrame = frame(() => {
    animation = animate(
      node,
      { opacity: [0, 1] },
      { duration: params.duration ?? 0.12, ease: [0, 0, 0.2, 1] }
    ) as MotionControls;
  });

  return {
    destroy() {
      cancelFrame();
      stopAnimation(animation);
    }
  };
}

/**
 * Enter/exit for status pills, inline notices, and toast-like surfaces.
 * Slides in 4px and fades; reduced-motion strips the translate.
 */
export function pill(node: HTMLElement, params: PillParams = {}) {
  if (reducedMotion()) {
    node.style.opacity = '1';
    return {};
  }

  let animation: MotionControls | null = null;
  node.style.opacity = '0';
  node.style.transform = 'translateY(4px)';

  const cancelFrame = frame(() => {
    animation = animate(
      node,
      { opacity: [0, 1], transform: ['translateY(4px)', 'translateY(0px)'] },
      { duration: (params.duration ?? 180) / 1000, ease: [0, 0, 0.2, 1] }
    ) as MotionControls;
  });

  return {
    destroy() {
      cancelFrame();
      stopAnimation(animation);
    }
  };
}

/**
 * Animate a positioned indicator element to overlay the currently active
 * segment button. Caller is responsible for re-invoking via `update` whenever
 * the active element changes (Svelte's `use:action={params}` does this).
 */
export function segmentIndicator(node: HTMLElement, params: SegmentIndicatorParams) {
  const baseDuration = (params.duration ?? 180) / 1000;
  node.style.transition = reducedMotion()
    ? 'none'
    : `transform ${baseDuration}s cubic-bezier(.2,.8,.2,1), width ${baseDuration}s cubic-bezier(.2,.8,.2,1), opacity 120ms ease`;

  function syncTo(target: HTMLElement | null) {
    if (!target || !target.offsetParent) {
      node.style.opacity = '0';
      return;
    }
    const parent = node.offsetParent as HTMLElement | null;
    if (!parent) return;
    const parentRect = parent.getBoundingClientRect();
    const targetRect = target.getBoundingClientRect();
    const x = targetRect.left - parentRect.left;
    const width = targetRect.width;

    node.style.opacity = '1';
    node.style.transform = `translateX(${x}px)`;
    node.style.width = `${width}px`;
  }

  // Sync on the next frame so the layout has settled.
  const cancelFrame = frame(() => syncTo(params.active));

  return {
    update(next: SegmentIndicatorParams) {
      syncTo(next.active);
    },
    destroy() {
      cancelFrame();
    }
  };
}

/**
 * Tween a formatted-number string. Use as a Svelte action on any element
 * whose text content is a metric value like `£64.50`, `1,300`, `5.9M`,
 * `96.5%`, or `220.2M Cached`. On update, the action parses the leading
 * prefix (currency), the numeric core (with optional thousands separators
 * and decimals), and the trailing suffix (%, K/M/B unit, label) — then
 * animates the number from the previously displayed value to the new one,
 * re-formatting on every frame. If the prefix or suffix doesn't match
 * between old and new (e.g. crossing the K→M threshold) it falls back to
 * an instant swap so the unit stays accurate.
 */
type FormatSpec = {
  prefix: string;
  suffix: string;
  number: number;
  decimals: number;
  hasThousandsSep: boolean;
};

const NUMBER_PATTERN = /^([^\d\-+]*?)(-?(?:\d+(?:,\d{3})*(?:\.\d+)?|\.\d+))(.*)$/;

function parseFormat(text: string): FormatSpec | null {
  const match = text.match(NUMBER_PATTERN);
  if (!match) return null;
  const [, prefix, numStr, suffix] = match;
  const hasThousandsSep = numStr.includes(',');
  const cleaned = numStr.replace(/,/g, '');
  const number = parseFloat(cleaned);
  if (!Number.isFinite(number)) return null;
  const dotIdx = cleaned.indexOf('.');
  const decimals = dotIdx >= 0 ? cleaned.length - dotIdx - 1 : 0;
  return { prefix, suffix, number, decimals, hasThousandsSep };
}

function formatNumber(spec: FormatSpec, value: number): string {
  const fixed = value.toFixed(spec.decimals);
  const [rawInt, fracPart] = fixed.split('.');
  let intPart = rawInt;
  let sign = '';
  if (intPart.startsWith('-')) {
    sign = '-';
    intPart = intPart.slice(1);
  }
  if (spec.hasThousandsSep) {
    intPart = intPart.replace(/\B(?=(\d{3})+(?!\d))/g, ',');
  }
  const body = fracPart !== undefined ? `${sign}${intPart}.${fracPart}` : `${sign}${intPart}`;
  return `${spec.prefix}${body}${spec.suffix}`;
}

export function countUp(node: HTMLElement, value: string) {
  let displayed = value;
  let raf: number | null = null;

  function cancelFrameLoop() {
    if (raf !== null) {
      window.cancelAnimationFrame(raf);
      raf = null;
    }
  }

  node.textContent = value;

  return {
    update(next: string) {
      if (next === displayed) return;
      const fromSpec = parseFormat(displayed);
      const toSpec = parseFormat(next);
      cancelFrameLoop();

      const incompatible =
        !fromSpec ||
        !toSpec ||
        fromSpec.prefix !== toSpec.prefix ||
        fromSpec.suffix !== toSpec.suffix;
      if (reducedMotion() || incompatible) {
        node.textContent = next;
        displayed = next;
        return;
      }

      const from = fromSpec!.number;
      const to = toSpec!.number;
      const duration = 520;
      const start = performance.now();

      function step(now: number) {
        const elapsed = now - start;
        const t = Math.min(1, elapsed / duration);
        const eased = 1 - Math.pow(1 - t, 3);
        const current = from + (to - from) * eased;
        node.textContent = formatNumber(toSpec!, current);
        if (t < 1) {
          raf = window.requestAnimationFrame(step);
        } else {
          node.textContent = next;
          displayed = next;
          raf = null;
        }
      }

      raf = window.requestAnimationFrame(step);
      displayed = next;
    },
    destroy() {
      cancelFrameLoop();
    }
  };
}

/**
 * Subtle hover-state toggle. Adds `is-hover` while the pointer is over the
 * element. CSS owns the actual transition (background tint, etc.). Respecting
 * reduced motion is the CSS author's responsibility for this one.
 */
export function hoverLift(node: HTMLElement) {
  function onEnter() {
    node.classList.add('is-hover');
  }
  function onLeave() {
    node.classList.remove('is-hover');
  }
  node.addEventListener('pointerenter', onEnter);
  node.addEventListener('pointerleave', onLeave);

  return {
    destroy() {
      node.removeEventListener('pointerenter', onEnter);
      node.removeEventListener('pointerleave', onLeave);
    }
  };
}
