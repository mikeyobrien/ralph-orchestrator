# ralph-proto

すべての Ralph クレートで共有されるプロトコル型です。

## 概要

`ralph-proto` は、Ralph 全体で使われるコアデータ構造を定義します。

- 通信用の Event と Topic
- ペルソナ定義用の Hat
- ルーティング用の EventBus

## 主要な型

### Event

トピック・ペイロード・ルーティング情報を持つメッセージです。

```rust
pub struct Event {
    pub topic: Topic,
    pub payload: Option<String>,
    pub source_hat: Option<HatId>,
    pub target_hat: Option<HatId>,
    pub timestamp: DateTime<Utc>,
}
```

**イベントの作成:**

```rust
use ralph_proto::Event;

// シンプルなイベント
let event = Event::new("build.done");

// ペイロード付き
let event = Event::new("build.done")
    .with_payload("tests: pass, lint: pass, typecheck: pass, audit: pass, coverage: pass");

// 発行元ハット付き
let event = Event::new("build.done")
    .from_hat("builder");
```

### Topic

glob パターンマッチによるイベントルーティングです。

```rust
pub struct Topic(String);

impl Topic {
    pub fn matches(&self, pattern: &str) -> bool;
}
```

**パターンマッチ:**

```rust
let topic = Topic::new("build.done");

topic.matches("build.done");  // true
topic.matches("build.*");     // true
topic.matches("*.done");      // true
topic.matches("test.*");      // false
```

### Hat

特化した Ralph のペルソナです。

```rust
pub struct Hat {
    pub id: HatId,
    pub name: String,
    pub description: Option<String>,
    pub triggers: Vec<String>,      // 購読パターン
    pub publishes: Vec<String>,     // 許可されるイベント種別
    pub default_publishes: Option<String>,
    pub instructions: String,
    pub backend: Option<String>,
    pub max_activations: Option<usize>,
}
```

**ハットの作成:**

```rust
use ralph_proto::Hat;

let hat = Hat::builder("builder")
    .name("Builder")
    .triggers(vec!["task.start", "plan.ready"])
    .publishes(vec!["build.done", "build.failed"])
    .instructions("Implement the task...")
    .build();
```

### HatId

ハットの一意な識別子です。

```rust
pub struct HatId(String);

impl HatId {
    pub fn new(id: impl Into<String>) -> Self;
}
```

### EventBus

ハットの登録簿とイベントルーティングです。

```rust
pub struct EventBus {
    hats: HashMap<HatId, Hat>,
    pending_events: VecDeque<Event>,
    event_history: Vec<Event>,
}

impl EventBus {
    pub fn register_hat(&mut self, hat: Hat);
    pub fn publish(&mut self, event: Event);
    pub fn next_event(&mut self) -> Option<Event>;
    pub fn matching_hat(&self, event: &Event) -> Option<&Hat>;
}
```

**EventBus の使い方:**

```rust
use ralph_proto::{EventBus, Event, Hat};

let mut bus = EventBus::new();

// ハットを登録する
bus.register_hat(planner_hat);
bus.register_hat(builder_hat);

// イベントを発行する
bus.publish(Event::new("task.start"));

// 次のイベントと一致するハットを取得する
if let Some(event) = bus.next_event() {
    if let Some(hat) = bus.matching_hat(&event) {
        // イベントを使ってハットを実行する
    }
}
```

## UX イベント

TUI とのやり取りのためのイベントです。

```rust
pub enum UxEvent {
    TerminalWrite(String),
    Resize { width: u16, height: u16 },
    FrameCapture(Vec<u8>),
}
```

## エラー型

```rust
pub enum ProtoError {
    InvalidTopic(String),
    InvalidHat(String),
    EventRoutingError(String),
}
```

## フィーチャーフラグ

| フラグ | 説明 |
|------|-------------|
| `default` | 標準機能 |
| `serde` | シリアライズサポート |

## 例: イベントフロー

```rust
use ralph_proto::{EventBus, Event, Hat};

// セットアップ
let mut bus = EventBus::new();

let planner = Hat::builder("planner")
    .triggers(vec!["task.start"])
    .publishes(vec!["plan.ready"])
    .instructions("Create a plan")
    .build();

let builder = Hat::builder("builder")
    .triggers(vec!["plan.ready"])
    .publishes(vec!["build.done"])
    .instructions("Implement the plan")
    .build();

bus.register_hat(planner);
bus.register_hat(builder);

// フローを開始する
bus.publish(Event::new("task.start"));

// 最初のイベントは planner に一致する
let event = bus.next_event().unwrap();
let hat = bus.matching_hat(&event).unwrap();
assert_eq!(hat.id.as_str(), "planner");

// Planner が plan.ready を発行する
bus.publish(Event::new("plan.ready").from_hat("planner"));

// 2番目のイベントは builder に一致する
let event = bus.next_event().unwrap();
let hat = bus.matching_hat(&event).unwrap();
assert_eq!(hat.id.as_str(), "builder");
```
