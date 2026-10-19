import { useReducedMotion } from "framer-motion"
import { useRef, type CSSProperties, type ReactNode } from "react"

const DOTS = Array.from({ length: 14 }, (_, i) => ({
  left: `${(i * 37 + 11) % 100}%`,
  s: `${2 + (i % 3)}px`,
  d: `${12 + ((i * 7) % 10)}s`,
  delay: `${-((i * 13) % 18)}s`,
  x: `${((i % 5) - 2) * 14}px`,
  o: `${0.25 + (i % 4) * 0.1}`,
}))

/** A sparse layer of slow rising dots. Hidden under prefers-reduced-motion by CSS. */
export function Particles() {
  return (
    <div className="particles" aria-hidden="true">
      {DOTS.map((dot, i) => (
        <span
          key={i}
          style={{ left: dot.left, "--s": dot.s, "--d": dot.d, "--delay": dot.delay, "--x": dot.x, "--o": dot.o } as CSSProperties}
        />
      ))}
    </div>
  )
}

/** Tilts its child a few degrees toward a fine pointer. No effect on touch or reduced motion. */
export function Tilt({ children, className = "", max = 5 }: { children: ReactNode; className?: string; max?: number }) {
  const ref = useRef<HTMLDivElement>(null)
  const reduce = useReducedMotion()
  const reset = () => {
    ref.current?.style.setProperty("--rx", "0deg")
    ref.current?.style.setProperty("--ry", "0deg")
  }
  return (
    <div
      ref={ref}
      className={`tilt ${className}`}
      onPointerMove={(event) => {
        if (reduce || event.pointerType !== "mouse" || !ref.current) return
        const box = ref.current.getBoundingClientRect()
        const px = (event.clientX - box.left) / box.width - 0.5
        const py = (event.clientY - box.top) / box.height - 0.5
        ref.current.style.setProperty("--ry", `${(px * max).toFixed(2)}deg`)
        ref.current.style.setProperty("--rx", `${(-py * max).toFixed(2)}deg`)
      }}
      onPointerLeave={reset}
    >
      {children}
    </div>
  )
}
