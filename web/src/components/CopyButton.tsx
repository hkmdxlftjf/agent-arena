import { Button } from "@/components/ui/button"
import { Check, Copy } from "lucide-react"
import { useState } from "react"

export function CopyButton({ text, label = "复制" }: { text: string; label?: string }) {
  const [done, setDone] = useState(false)
  return (
    <Button
      type="button"
      variant="outline"
      size="sm"
      aria-live="polite"
      onClick={() => {
        void navigator.clipboard.writeText(text).then(() => {
          setDone(true)
          window.setTimeout(() => setDone(false), 1600)
        })
      }}
    >
      {done ? <Check className="text-live" aria-hidden /> : <Copy aria-hidden />}
      {done ? "已复制" : label}
    </Button>
  )
}
