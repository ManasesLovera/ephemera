// Categorical slot order from the dataviz reference palette — fixed, never cycled.
// A file's slot is bound to its id for the session so color follows the entity,
// never its rank in the list.
const SLOTS = ["s1", "s2", "s3", "s4", "s5", "s6", "s7", "s8"] as const;

const assigned = new Map<string, string>();
let nextSlot = 0;

export function colorSlotFor(id: string): string {
  const existing = assigned.get(id);
  if (existing) return existing;
  const slot = nextSlot < SLOTS.length ? SLOTS[nextSlot] : "other";
  if (nextSlot < SLOTS.length) nextSlot += 1;
  assigned.set(id, slot);
  return slot;
}

export function resetColors() {
  assigned.clear();
  nextSlot = 0;
}
