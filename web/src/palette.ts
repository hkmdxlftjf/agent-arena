const PALETTES = ["lichess", "chess", "linear", "kaggle"] as const

export type Palette = (typeof PALETTES)[number]

function known(value: string | null): value is Palette {
  return !!value && (PALETTES as readonly string[]).includes(value)
}

/** Hidden switch: `?theme=lichess|chess|linear|kaggle`. `?theme=default` clears it. */
export function syncPalette() {
  const query = new URLSearchParams(location.search).get("theme")
  if (query === "default" || query === "off") sessionStorage.removeItem("arena-palette")
  else if (known(query)) sessionStorage.setItem("arena-palette", query)
  const saved = sessionStorage.getItem("arena-palette")
  if (known(saved)) document.documentElement.dataset.palette = saved
  else delete document.documentElement.dataset.palette
}
