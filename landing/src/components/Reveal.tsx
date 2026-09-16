import { useEffect, useRef, useState, type ReactNode } from "react";

/**
 * Fades its children up the first time they come into view, then forgets
 * about them. One observer per wrapper, disconnected on the first hit, so
 * nothing stays attached to the scroll for the life of the page.
 *
 * The initial state is the hidden one only when the browser is going to
 * animate at all: with `prefers-reduced-motion` the content is simply there.
 */
export function Reveal({
  children,
  delay = 0,
  className = "",
}: {
  children: ReactNode;
  delay?: number;
  className?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [shown, setShown] = useState(false);

  useEffect(() => {
    const node = ref.current;
    if (!node) return;

    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setShown(true);
      return;
    }

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) {
          setShown(true);
          observer.disconnect();
        }
      },
      { threshold: 0.12, rootMargin: "0px 0px -40px 0px" },
    );

    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  return (
    <div
      ref={ref}
      className={`transition-[opacity,transform] duration-[900ms] ease-out ${
        shown ? "translate-y-0 opacity-100" : "translate-y-[22px] opacity-0"
      } ${className}`}
      style={{ transitionDelay: `${delay}ms` }}
    >
      {children}
    </div>
  );
}
