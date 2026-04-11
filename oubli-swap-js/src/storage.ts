// Custom storage adapter that delegates to Rust host functions.
// Replaces browser's LocalStorage / IndexedDB.

import { IStorageManager, StorageObject } from "@atomiqlabs/base";

declare function __oubli_storage_get(key: string): string | null;
declare function __oubli_storage_set(key: string, value: string): void;
declare function __oubli_storage_remove(key: string): void;

/**
 * In-memory + Rust-backed storage manager for the Atomiq SDK.
 * Swap state is persisted through the Rust host so it survives app restarts.
 */
export class OubliStorageManager<T extends StorageObject> {
  data: { [key: string]: T } = {};
  private readonly prefix: string;

  constructor(name: string) {
    this.prefix = `atomiq.${name}.`;
  }

  async init(): Promise<void> {
    // Load all entries with our prefix from Rust storage
    const indexKey = this.prefix + "__index";
    const indexRaw = __oubli_storage_get(indexKey);
    if (indexRaw) {
      try {
        const keys: string[] = JSON.parse(indexRaw);
        for (const key of keys) {
          const raw = __oubli_storage_get(this.prefix + key);
          if (raw) {
            this.data[key] = JSON.parse(raw);
          }
        }
      } catch (e) {
        console.warn("Failed to load storage index:", e);
      }
    }
  }

  async saveData(hash: string, object: T): Promise<void> {
    this.data[hash] = object;
    __oubli_storage_set(this.prefix + hash, JSON.stringify(object));
    this._saveIndex();
  }

  async removeData(hash: string): Promise<void> {
    delete this.data[hash];
    __oubli_storage_remove(this.prefix + hash);
    this._saveIndex();
  }

  async loadData(hash: string): Promise<T | null> {
    return this.data[hash] ?? null;
  }

  async loadAll(): Promise<T[]> {
    return Object.values(this.data);
  }

  async loadAllByIndex?(indexName: string, indexValue: any): Promise<T[]> {
    return Object.values(this.data).filter(
      (obj: any) => obj[indexName] === indexValue,
    );
  }

  private _saveIndex(): void {
    const indexKey = this.prefix + "__index";
    __oubli_storage_set(indexKey, JSON.stringify(Object.keys(this.data)));
  }
}

/**
 * Factory function that creates storage managers with our custom backend.
 */
export function oubliStorageCtor<T extends StorageObject>(
  name: string,
): IStorageManager<T> {
  return new OubliStorageManager<T>(name) as unknown as IStorageManager<T>;
}
