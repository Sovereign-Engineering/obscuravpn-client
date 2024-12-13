export class KeyedSet<V, K> {
  #key: (v: V) => K;
  #map = new Map<K, V>;

  constructor(
    key: (v: V) => K,
    entries?: Iterable<V>,
  ) {
    this.#key = key;
    if (entries) {
      this.extend(entries);
    }
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

  get(v: V): V | undefined {
    return this.getKey(this.#key(v));
  }

  getKey(k: K): V | undefined {
    return this.#map.get(k);
  }

  has(v: V): boolean {
    return this.hasKey(this.#key(v));
  }

  hasKey(k: K): boolean {
    return this.#map.has(k);
  }
}
