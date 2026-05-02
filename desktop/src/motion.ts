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
