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
