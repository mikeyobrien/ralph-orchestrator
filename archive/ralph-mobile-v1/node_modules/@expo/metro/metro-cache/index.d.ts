import Cache from "./Cache";
import stableHash from "./stableHash";
import AutoCleanFileStore from "./stores/AutoCleanFileStore";
import FileStore from "./stores/FileStore";
import HttpGetStore from "./stores/HttpGetStore";
import HttpStore from "./stores/HttpStore";
export type { Options as FileOptions } from "./stores/FileStore";
export type { Options as HttpOptions } from "./stores/HttpStore";
export type { CacheStore } from "./types";
export { AutoCleanFileStore, Cache, FileStore, HttpGetStore, HttpStore, stableHash };
export interface MetroCache {
  readonly AutoCleanFileStore: typeof AutoCleanFileStore;
  readonly Cache: typeof Cache;
  readonly FileStore: typeof FileStore;
  readonly HttpGetStore: typeof HttpGetStore;
  readonly HttpStore: typeof HttpStore;
  readonly stableHash: typeof stableHash;
}
declare const $$EXPORT_DEFAULT_DECLARATION$$: MetroCache;
export default $$EXPORT_DEFAULT_DECLARATION$$;