import bash from "highlight.js/lib/languages/bash"
import javascript from "highlight.js/lib/languages/javascript"
import json from "highlight.js/lib/languages/json"
import markdown from "highlight.js/lib/languages/markdown"
import "highlight.js/styles/github-dark.css"
import ReactMarkdown from "react-markdown"
import rehypeHighlight from "rehype-highlight"
import remarkGfm from "remark-gfm"

export function MarkdownView({ source }: { source: string }) {
  return (
    <div className="rules-md">
      <ReactMarkdown
        skipHtml
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[[rehypeHighlight, { languages: { javascript, json, markdown, bash } }]]}
      >
        {source}
      </ReactMarkdown>
    </div>
  )
}
