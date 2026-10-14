const PALETTES = ["lichess", "chess", "linear", "kaggle", "warm"] as const

export type Palette = (typeof PALETTES)[number]

function known(value: string | null): value is Palette {
  return !!value && (PALETTES as readonly string[]).includes(value)
}

/** Chess is the default. `?theme=lichess|linear|kaggle|warm` switches; `chess`, `default`, or `off` clears it. */
export function syncPalette() {
  const query = new URLSearchParams(location.search).get("theme")
  if (query === "default" || query === "off" || query === "chess") sessionStorage.removeItem("arena-palette")
  else if (known(query)) sessionStorage.setItem("arena-palette", query)
  const saved = sessionStorage.getItem("arena-palette")
  if (known(saved) && saved !== "chess") document.documentElement.dataset.palette = saved
  else delete document.documentElement.dataset.palette
}
