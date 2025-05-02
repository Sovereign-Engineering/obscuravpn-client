export class KeyedSet<V extends L, L = V, K = unknown> {
  #key: (v: L) => K;
  #map = new Map<K, V>;

  constructor(
    key: (v: L) => K,
    entries?: Iterable<V>,
  ) {
    this.#key = key;
    if (entries) {
      this.extend(entries);
    }
  }

  [Symbol.iterator](): Iterator<V> {
    return this.#map.values();
  }

  /// Add an item to the set.
  ///
  /// Always updates the stored item to the new value.
  add(v: V): V | undefined {
    let k = this.#key(v);
    let existing = this.#map.get(k);

    // Note: Skip second lookup in common case where value is not undefined.
    if (existing || this.#map.has(k)) {
      return existing;
    }

    this.#map.set(k, v);
  }

  extend(values: Iterable<V>) {
    for (let v of values) {
      this.add(v);
    }
  }

  get(v: L): V | undefined {
    return this.getKey(this.#key(v));
  }

  getKey(k: K): V | undefined {
    return this.#map.get(k);
  }

  has(v: L): boolean {
    return this.hasKey(this.#key(v));
  }

  hasKey(k: K): boolean {
    return this.#map.has(k);
  }

  get size(): number {
    return this.#map.size
  }
}
