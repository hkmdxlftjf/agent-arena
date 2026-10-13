import { readdir, readFile, stat, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { fileURLToPath } from "node:url"
import { brotliCompressSync, constants, gzipSync } from "node:zlib"

const root = fileURLToPath(new URL("../dist/", import.meta.url))

async function walk(dir) {
  const entries = await readdir(dir, { withFileTypes: true })
  const files = []
  for (const entry of entries) {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) files.push(...(await walk(path)))
    else files.push(path)
  }
  return files
}

const files = await walk(root)
let wrote = 0
for (const file of files) {
  if (file.endsWith(".br") || file.endsWith(".gz")) continue
  const info = await stat(file)
  if (info.size < 256) continue
  const raw = await readFile(file)
  const gz = gzipSync(raw, { level: 9 })
  const br = brotliCompressSync(raw, {
    params: { [constants.BROTLI_PARAM_QUALITY]: 11 },
  })
  if (gz.length < raw.length) {
    await writeFile(`${file}.gz`, gz)
    wrote += 1
  }
  if (br.length < raw.length) {
    await writeFile(`${file}.br`, br)
    wrote += 1
  }
}
console.log(`precompressed ${wrote} files under web/dist`)
