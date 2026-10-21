// Authored inputs deliberately change scalar and rank-table values. Expected
// effects are still computed by the unmodified Website kernels, not this helper.
export function authoredStatBook(source) {
  const book = structuredClone(source)
  for (const entry of book.entries) {
    entry.description = `Authored row ${entry.id}`
    entry.quickDescription = `Override ${entry.id}`
    for (const [key,value] of Object.entries(entry.numericProperties)) {
      if (key === 'mCapLevel' || key === 'mMaxLevel') continue
      const change = value => value * 1.25 + 0.125
      entry.numericProperties[key] = Array.isArray(value) ? value.map(change) : change(value)
    }
  }
  // Mindstar still uses the shared stock maximum; Creativity reads this row.
  book.entries[56].maximumLevel = 1
  book.entries[57].maximumLevel = 2
  return book
}
