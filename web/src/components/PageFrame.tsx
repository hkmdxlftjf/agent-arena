import { motion, useReducedMotion } from "framer-motion"
import type { ReactNode } from "react"
import { useLocation } from "react-router-dom"

export function PageFrame({ children }: { children: ReactNode }) {
  const location = useLocation()
  const reduce = useReducedMotion()
  return (
    <motion.div
      key={location.pathname}
      initial={reduce ? false : { opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.22, ease: "easeOut" }}
    >
      {children}
    </motion.div>
  )
}
