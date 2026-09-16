import { useEffect, useRef } from "react";

/** How many stars per million device-independent pixels of viewport. */
const DENSITY = 90;

interface Star {
  x: number;
  y: number;
  r: number;
  base: number;
  /** Radians per frame of the twinkle, and where in it this star starts. */
  speed: number;
  phase: number;
}

/**
 * The sky the page sits on.
 *
 * A canvas rather than a few hundred absolutely positioned divs: the stars
 * twinkle, and a compositor asked to animate the opacity of six hundred
 * elements spends the whole scroll doing it. Sizes follow the device pixel
 * ratio so the points stay round on a retina screen, and a reduced-motion
 * preference gets a single still frame instead of a loop.
 */
export function Starfield() {
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = ref.current;
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx) return;

    let stars: Star[] = [];
    let frame = 0;
    let width = 0;
    let height = 0;

    const still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    function seed() {
      if (!canvas || !ctx) return;
      const ratio = Math.min(window.devicePixelRatio || 1, 2);
      width = window.innerWidth;
      height = window.innerHeight;
      canvas.width = width * ratio;
      canvas.height = height * ratio;
      ctx.setTransform(ratio, 0, 0, ratio, 0, 0);

      const count = Math.round(((width * height) / 1_000_000) * DENSITY);
      stars = Array.from({ length: count }, () => ({
        x: Math.random() * width,
        y: Math.random() * height,
        r: Math.random() * 1.1 + 0.25,
        base: Math.random() * 0.5 + 0.2,
        speed: Math.random() * 0.012 + 0.004,
        phase: Math.random() * Math.PI * 2,
      }));
    }

    function draw() {
      if (!ctx) return;
      ctx.clearRect(0, 0, width, height);
      for (const star of stars) {
        const twinkle = still ? 0 : Math.sin(star.phase + frame * star.speed) * 0.35;
        ctx.globalAlpha = Math.max(0.05, Math.min(1, star.base + twinkle));
        // The faintest stars stay white; the larger ones take the accent,
        // which is what keeps the field from reading as grey noise.
        ctx.fillStyle = star.r > 1 ? "#a9c4ff" : "#ffffff";
        ctx.beginPath();
        ctx.arc(star.x, star.y, star.r, 0, Math.PI * 2);
        ctx.fill();
      }
      ctx.globalAlpha = 1;
    }

    let raf = 0;
    function loop() {
      frame += 1;
      draw();
      raf = requestAnimationFrame(loop);
    }

    seed();
    if (still) draw();
    else loop();

    let resizeTimer = 0;
    function onResize() {
      window.clearTimeout(resizeTimer);
      resizeTimer = window.setTimeout(() => {
        seed();
        draw();
      }, 200);
    }

    window.addEventListener("resize", onResize);
    return () => {
      cancelAnimationFrame(raf);
      window.clearTimeout(resizeTimer);
      window.removeEventListener("resize", onResize);
    };
  }, []);

  return (
    <div className="pointer-events-none fixed inset-0 -z-10 overflow-hidden" aria-hidden="true">
      <canvas ref={ref} className="absolute inset-0 size-full opacity-70" />

      {/* Two nebulae, drifting slowly enough to be felt rather than seen.
          Nothing else is layered on top: this element is fixed, so any band
          drawn in viewport units would hang a hard edge across the middle of
          the screen and stay there while the page scrolled past it. What
          fades the sky out belongs to the section that needs it, in the flow
          of the document. */}
      <div className="absolute -top-[18vh] -left-[12vw] size-[58vw] animate-drift rounded-full bg-[radial-gradient(circle,var(--color-accent)_0%,transparent_65%)] opacity-[0.24] blur-[90px]" />
      <div className="absolute top-[40vh] -right-[16vw] size-[50vw] animate-drift rounded-full bg-[radial-gradient(circle,var(--color-nebula)_0%,transparent_65%)] opacity-[0.20] blur-[100px] [animation-delay:-12s]" />
    </div>
  );
}
