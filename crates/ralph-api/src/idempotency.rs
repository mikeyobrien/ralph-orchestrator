use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

#[derive(Debug, Clone)]
pub struct StoredResponse {
    pub status: u16,
    pub envelope: Value,
}

#[derive(Debug, Clone)]
pub enum IdempotencyCheck {
    New,
    Replay(StoredResponse),
    Conflict,
}

pub trait IdempotencyStore: Send + Sync {
    fn check(&self, method: &str, key: &str, params: &Value) -> IdempotencyCheck;
    fn store(&self, method: &str, key: &str, params: &Value, response: &StoredResponse);
}

#[derive(Debug, Clone)]
pub struct InMemoryIdempotencyStore {
    entries: Arc<Mutex<HashMap<String, Entry>>>,
    ttl: Duration,
}

#[derive(Debug, Clone)]
struct Entry {
    params: Value,
    response: StoredResponse,
    created_at: Instant,
}

impl InMemoryIdempotencyStore {
    pub fn new(ttl: Duration) -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            ttl,
        }
    }

    fn cleanup(&self, entries: &mut HashMap<String, Entry>) {
        let ttl = self.ttl;
        entries.retain(|_, entry| entry.created_at.elapsed() <= ttl);
    }

    fn make_key(method: &str, key: &str) -> String {
        format!("{method}:{key}")
    }
}

impl IdempotencyStore for InMemoryIdempotencyStore {
    fn check(&self, method: &str, key: &str, params: &Value) -> IdempotencyCheck {
        let mut guard = self
            .entries
            .lock()
            .expect("idempotency store mutex should not be poisoned");
        self.cleanup(&mut guard);

        let store_key = Self::make_key(method, key);
        match guard.get(&store_key) {
            None => IdempotencyCheck::New,
            Some(entry) if entry.params == *params => {
                IdempotencyCheck::Replay(entry.response.clone())
            }
            Some(_) => IdempotencyCheck::Conflict,
        }
    }

    fn store(&self, method: &str, key: &str, params: &Value, response: &StoredResponse) {
        let mut guard = self
            .entries
            .lock()
            .expect("idempotency store mutex should not be poisoned");
        self.cleanup(&mut guard);
        guard.insert(
            Self::make_key(method, key),
            Entry {
                params: params.clone(),
                response: response.clone(),
                created_at: Instant::now(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;

    use super::{IdempotencyCheck, IdempotencyStore, InMemoryIdempotencyStore, StoredResponse};

    #[test]
    fn replays_same_method_key_and_params() {
        let store = InMemoryIdempotencyStore::new(Duration::from_secs(60));
        let params = json!({ "value": 1 });
        let response = StoredResponse {
            status: 200,
            envelope: json!({ "ok": true }),
        };

        assert!(matches!(
            store.check("task.create", "idem-1", &params),
            IdempotencyCheck::New
        ));

        store.store("task.create", "idem-1", &params, &response);

        let replay = store.check("task.create", "idem-1", &params);
        match replay {
            IdempotencyCheck::Replay(actual) => assert_eq!(actual.envelope, response.envelope),
            _ => panic!("expected replay"),
        }
    }

    #[test]
    fn detects_conflict_for_same_key_with_different_params() {
        let store = InMemoryIdempotencyStore::new(Duration::from_secs(60));
        store.store(
            "task.create",
            "idem-2",
            &json!({ "value": 1 }),
            &StoredResponse {
                status: 200,
                envelope: json!({ "ok": true }),
            },
        );

        assert!(matches!(
            store.check("task.create", "idem-2", &json!({ "value": 2 })),
            IdempotencyCheck::Conflict
        ));
    }
}
