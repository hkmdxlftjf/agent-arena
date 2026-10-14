import { Button } from "@/components/ui/button"
import { useState } from "react"

export function CopyButton({ text, label = "复制" }: { text: string; label?: string }) {
  const [done, setDone] = useState(false)
  return (
    <Button
      type="button"
      variant="outline"
      size="sm"
      onClick={() => {
        void navigator.clipboard.writeText(text).then(() => {
          setDone(true)
          window.setTimeout(() => setDone(false), 1600)
        })
      }}
    >
      {done ? "已复制" : label}
    </Button>
  )
}
