# GhostMesh — P2P Messenger

## Спецификация v0.2 (Tauri 2)

---

## 1. Видение продукта

GhostMesh — полностью децентрализованный P2P-мессенджер с BBS-философией: написал сообщение — опубликовал — ушёл. Получатель заберёт, когда появится в сети. Никаких серверов, никакой регистрации, никаких номеров телефона. Публичные ключи контактов хранятся только на устройстве пользователя.

### 1.1 Принципы

- **Нулевая инфраструктура.** Ни одного сервера. Каждое устройство — и клиент, и нода.
- **BBS-асинхронность.** Нет ожидания мгновенного ответа. Написал → отправил тем, кто онлайн → остальные получат позже через gossip.
- **Радикальная приватность.** E2E шифрование, отсутствие метаданных на внешних серверах, нет привязки к телефону/email.
- **Gossip-доставка.** Участники чата передают сообщения друг другу при встрече. Если Алиса видела Боба и Веру в разное время — она relay между ними.
- **Эфемерность.** Удалил приложение — исчез. Нигде не осталось аккаунта.

### 1.2 UX-метафора

Радиолюбительский эфир. Открыл приложение — вышел на частоту. Кто есть — поговорили. Кого нет — оставил сообщение. Закрыл приложение — ушёл из эфира.

### 1.3 Целевая аудитория

- **Разработчики** — ценят контроль, хотят видеть внутренности протокола (lamport clocks, fingerprints, sync-лог).
- **Радиолюбители по духу** — мыслят частотами, позывными, эфиром. Метафора TX/RX.
- **Параноики приватности** — минимум метаданных, минимум на экране, нигде не светиться.
- **Современные технари** — хотят чистый, функциональный интерфейс без лишнего.

### 1.4 Что изменилось в v0.2

Переход с Flutter Desktop на **Tauri 2**. Причины:
- Rust Core перестаёт быть отдельной библиотекой — он становится бэкендом Tauri. Никакого FFI, никаких биндингов.
- UI пишется на веб-технологиях (HTML/CSS/TypeScript + Svelte). Дизайн-концепции, созданные ранее в HTML, работают в Tauri без переделки.
- Финальный бинарник ~5–10 MB вместо ~25 MB (системный WebView, не свой движок).
- Один тулчейн: Rust + npm. Не нужен Dart SDK.
- Tauri 2 поддерживает мобильные платформы (Android/iOS) — путь к мобилке сохранён.

---

## 2. Глоссарий

| Термин | Определение |
|---|---|
| **Peer** | Экземпляр приложения на конкретном устройстве. Идентифицируется публичным ключом. |
| **Identity** | Пара ключей Ed25519 (подпись) + X25519 (обмен ключами). Генерируется при первом запуске. |
| **Chat** | Группа пиров, объединённых общим групповым ключом и chat_id. |
| **Message** | Неизменяемая подписанная запись в append-only логе чата. |
| **Frontier** | Вектор, описывающий, какие сообщения каждого автора известны данному пиру. |
| **Sync session** | Процесс обмена недостающими сообщениями между двумя подключёнными пирами. |
| **Outbox** | Локальная очередь сообщений, ещё не доставленных конкретным участникам. |
| **Command** | Tauri IPC: вызов Rust-функции из фронтенда через `invoke()`. |
| **Event** | Tauri IPC: push-уведомление из Rust-бэкенда во фронтенд через `emit()`. |

---

## 3. Архитектура

### 3.1 Слои системы

```
┌──────────────────────────────────────────────────┐
│              Svelte UI (WebView)                  │
│      components, stores, screens, CSS             │
├──────────────────────────────────────────────────┤
│           Tauri IPC (Commands + Events)           │
│     invoke("send_message", {...})                 │
│     listen("new_message", callback)               │
├──────────────────────────────────────────────────┤
│               Tauri Rust Backend                  │
│  ┌──────────┬──────────┬─────────┬─────────────┐ │
│  │ Commands │   Sync   │   Net   │   Crypto    │ │
│  │ (IPC API)│  Engine  │  Layer  │   Layer     │ │
│  ├──────────┴──────────┴─────────┴─────────────┤ │
│  │               Store (SQLite)                 │ │
│  └──────────────────────────────────────────────┘ │
│               Tauri Runtime                       │
│    (window mgmt, tray, system integration)        │
└──────────────────────────────────────────────────┘
```

### 3.2 Принцип разделения

**Tauri Rust Backend** — ВСЯ логика: криптография, сетевой протокол, синхронизация, хранение. Rust-функции помечаются `#[tauri::command]` и автоматически становятся доступны из фронтенда. Фоновые процессы (sync, discovery) запускаются через `tauri::async_runtime` и шлют события во фронтенд через `app.emit()`.

**Svelte UI** — тонкая оболочка в системном WebView. Вызывает Rust через `invoke()`, слушает события через `listen()`. Рисует экраны, обрабатывает ввод. Не содержит бизнес-логики. Вся криптография и сеть — только в Rust.

**CLI (dev-only)** — Rust-бинарник, переиспользующий тот же core-код. Для тестирования без UI.

### 3.3 IPC-контракт (Commands)

```rust
// === Identity ===
#[tauri::command]
async fn create_identity(name: String) -> Result<IdentityInfo, Error>;

#[tauri::command]
async fn get_identity() -> Result<IdentityInfo, Error>;

#[tauri::command]
async fn export_identity(password: String) -> Result<Vec<u8>, Error>;

#[tauri::command]
async fn import_identity(data: Vec<u8>, password: String) -> Result<IdentityInfo, Error>;

// === Chats ===
#[tauri::command]
async fn create_chat(name: String) -> Result<ChatInfo, Error>;

#[tauri::command]
async fn list_chats() -> Result<Vec<ChatInfo>, Error>;

#[tauri::command]
async fn get_chat(chat_id: String) -> Result<ChatDetail, Error>;

#[tauri::command]
async fn generate_invite(chat_id: String) -> Result<InviteCode, Error>;

#[tauri::command]
async fn join_chat(invite_code: String) -> Result<ChatInfo, Error>;

#[tauri::command]
async fn leave_chat(chat_id: String) -> Result<(), Error>;

// === Messages ===
#[tauri::command]
async fn send_message(chat_id: String, text: String) -> Result<MessageInfo, Error>;

#[tauri::command]
async fn get_messages(chat_id: String, before_lamport: Option<u64>, limit: u32) -> Result<Vec<MessageInfo>, Error>;

#[tauri::command]
async fn get_message_detail(message_id: String) -> Result<MessagePacket, Error>;

// === Network ===
#[tauri::command]
async fn get_peers() -> Result<Vec<PeerInfo>, Error>;

#[tauri::command]
async fn get_connections() -> Result<Vec<ConnectionInfo>, Error>;

#[tauri::command]
async fn get_outbox() -> Result<Vec<OutboxEntry>, Error>;

#[tauri::command]
async fn add_manual_peer(address: String) -> Result<(), Error>;

#[tauri::command]
async fn get_sync_log(limit: u32) -> Result<Vec<SyncLogEntry>, Error>;

// === Settings ===
#[tauri::command]
async fn get_settings() -> Result<Settings, Error>;

#[tauri::command]
async fn update_settings(settings: Settings) -> Result<(), Error>;
```

### 3.4 IPC-контракт (Events: Rust → Frontend)

```rust
// Новое сообщение в любом чате
app.emit("message:new", MessageInfo { ... });

// Пир подключился / отключился
app.emit("peer:connected", PeerEvent { ... });
app.emit("peer:disconnected", PeerEvent { ... });

// Sync-прогресс
app.emit("sync:progress", SyncProgress { chat_id, received, total });
app.emit("sync:complete", SyncComplete { chat_id, new_messages });

// Delivery ACK
app.emit("delivery:ack", DeliveryAck { message_id, peer_id });

// Системные
app.emit("network:status", NetworkStatus { connected_peers, outbox_size });
app.emit("chat:member_joined", MemberEvent { ... });
app.emit("chat:member_left", MemberEvent { ... });
app.emit("chat:key_rotated", KeyRotationEvent { ... });
```

### 3.5 Фронтенд (Svelte)

```typescript
// Отправка сообщения
import { invoke } from "@tauri-apps/api/core";

const result = await invoke("send_message", {
  chatId: "a7f3b291...",
  text: "Проверь frontier — lamport 52"
});

// Подписка на новые сообщения
import { listen } from "@tauri-apps/api/event";

const unlisten = await listen("message:new", (event) => {
  messages = [...messages, event.payload];
});

// Подписка на статус сети
await listen("network:status", (event) => {
  connectedPeers = event.payload.connected_peers;
  outboxSize = event.payload.outbox_size;
});
```

---

## 4. Identity (Идентификация)

### 4.1 Генерация ключей

При первом запуске приложение генерирует:

```
signing_keypair:    Ed25519 (64 bytes secret + 32 bytes public)
exchange_keypair:   X25519  (32 bytes secret + 32 bytes public)
peer_id:            SHA-256(signing_public_key)[:16] → 32 hex chars
```

- `signing_keypair` используется для подписи сообщений и аутентификации.
- `exchange_keypair` используется для Diffie-Hellman при установке сессии.
- `peer_id` — человекочитаемый идентификатор, производный от публичного ключа.

### 4.2 Хранение ключей

Секретные ключи хранятся в зашифрованном хранилище:
- **Desktop:** Файл в app data dir (`tauri::api::path::app_data_dir`), зашифрованный паролем пользователя (Argon2id → AES-256-GCM).
- **Android (V2):** Android Keystore → AES-256-GCM.
- **iOS (V2):** Secure Enclave / Keychain.

### 4.3 Профиль пользователя

```rust
#[derive(Serialize, Deserialize)]
struct Identity {
    peer_id:        PeerId,         // 16 bytes
    signing_pk:     Ed25519Public,  // 32 bytes
    exchange_pk:    X25519Public,   // 32 bytes
    display_name:   String,         // до 64 UTF-8 символов
    created_at:     u64,            // Unix timestamp
}
```

### 4.4 Экспорт / бэкап

Пользователь может экспортировать Identity как зашифрованный файл (пароль → Argon2id → AES-256-GCM). Tauri предоставляет нативный диалог сохранения файла через `tauri-plugin-dialog`. Потерял ключи — потерял аккаунт.

---

## 5. Чат (Chat)

### 5.1 Создание чата

Инициатор (owner) генерирует:

```rust
struct Chat {
    chat_id:        [u8; 16],           // random UUID
    chat_name:      String,             // до 128 UTF-8 символов
    group_key:      [u8; 32],           // ChaCha20-Poly1305 симметричный ключ
    owner_peer_id:  PeerId,
    created_at:     u64,
    members:        Vec<ChatMember>,
    key_epoch:      u64,                // счётчик ротаций ключа
}

struct ChatMember {
    peer_id:        PeerId,
    signing_pk:     Ed25519Public,
    exchange_pk:    X25519Public,
    display_name:   String,
    role:           MemberRole,         // Owner | Admin | Member
    added_at:       u64,
    added_by:       PeerId,
}
```

### 5.2 Вступление в чат (Invite-код)

#### Содержимое invite-кода:

```rust
struct ChatInvite {
    chat_id:            [u8; 16],
    chat_name:          String,
    owner_peer_id:      PeerId,
    owner_signing_pk:   Ed25519Public,
    owner_exchange_pk:  X25519Public,
    owner_addresses:    Vec<PeerAddress>,   // IP:port
    invite_token:       [u8; 32],           // одноразовый токен
    created_at:         u64,
}
```

Кодируется как CBOR → Base62 строка с префиксом `ghm://`. На десктопе QR-сканирование менее удобно, поэтому основной flow — копирование invite-кода текстом. QR доступен как опция (генерация через JS-библиотеку `qrcode`, сканирование через webcam + `jsQR`).

#### Процесс вступления:

1. Участник вставляет invite-код или сканирует QR.
2. Фронтенд вызывает `invoke("join_chat", { inviteCode })`.
3. Rust-бэкенд парсит код, устанавливает Noise-соединение с owner.
4. Отправляет `JoinRequest { invite_token, identity }`.
5. Owner проверяет токен, отвечает `{ encrypted_group_key, members, recent_messages }`.
6. Owner рассылает `MemberAdded` всем онлайн-участникам.
7. Бэкенд эмитит `chat:member_joined` во фронтенд.

### 5.3 Удаление участника и ротация ключей

1. Owner/Admin вызывает команду удаления.
2. Бэкенд генерирует новый `group_key`, рассылает `RekeyPackage` каждому оставшемуся.
3. `key_epoch` инкрементируется.
4. Сообщения после ротации шифруются новым ключом.
5. Старые ключи хранятся локально для чтения истории.

### 5.4 Прямой чат (1-на-1)

Invite содержит только Identity одного участника. Shared secret из X25519 DH вместо группового ключа.

---

## 6. Сообщения (Messages)

### 6.1 Структура сообщения

```rust
struct Message {
    // === Заголовок (открытый, подписанный) ===
    message_id:     [u8; 32],       // SHA-256(chat_id ‖ author_peer_id ‖ lamport_ts ‖ payload_hash)
    chat_id:        [u8; 16],
    author_peer_id: PeerId,
    lamport_ts:     u64,            // Lamport logical clock
    created_at:     u64,            // wall clock (информационно, не для упорядочивания)
    key_epoch:      u64,            // каким group_key зашифрован payload
    parent_ids:     Vec<[u8; 32]>,  // causal dependencies (max 8)
    signature:      Ed25519Sig,     // подпись заголовка + payload_ciphertext

    // === Payload (зашифрованный) ===
    payload_ciphertext: Vec<u8>,    // ChaCha20-Poly1305(group_key, nonce, plaintext)
    payload_nonce:      [u8; 24],
}
```

### 6.2 Типы payload

```rust
enum MessagePayload {
    Text { body: String },              // UTF-8, до 65536 байт
    SystemEvent(SystemEvent),
}

enum SystemEvent {
    MemberAdded { member: ChatMember, encrypted_group_key: Vec<u8> },
    MemberRemoved { peer_id: PeerId, new_key_epoch: u64, rekey_packages: Vec<RekeyPackage> },
    ChatRenamed { new_name: String },
    KeyRotation { new_key_epoch: u64, rekey_packages: Vec<RekeyPackage> },
}

struct RekeyPackage {
    target_peer_id:  PeerId,
    encrypted_key:   Vec<u8>,   // X25519 sealed box
}
```

### 6.3 Ограничения

- Максимальный размер `payload_ciphertext`: 64 KiB.
- Файлы/медиа: V1 не поддерживает. V2 — chunked transfer.
- Максимум `parent_ids`: 8.

### 6.4 Порядок сообщений (Lamport Clock)

```
При отправке:  lamport_counter = max(lamport_counter, max(known_remote_counters)) + 1
При получении: lamport_counter = max(lamport_counter, received_lamport_ts) + 1
```

Отображение: сортировка по `(lamport_ts, author_peer_id)` — детерминированный порядок у всех участников.

---

## 7. Хранение (Store)

### 7.1 SQLite схема

```sql
CREATE TABLE identity (
    peer_id             BLOB PRIMARY KEY,
    signing_sk_enc      BLOB NOT NULL,      -- зашифрованный секретный ключ
    signing_pk          BLOB NOT NULL,
    exchange_sk_enc     BLOB NOT NULL,
    exchange_pk         BLOB NOT NULL,
    display_name        TEXT NOT NULL,
    created_at          INTEGER NOT NULL
);

CREATE TABLE chats (
    chat_id             BLOB PRIMARY KEY,
    chat_name           TEXT NOT NULL,
    owner_peer_id       BLOB NOT NULL,
    created_at          INTEGER NOT NULL,
    my_lamport_counter  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE chat_members (
    chat_id             BLOB NOT NULL,
    peer_id             BLOB NOT NULL,
    signing_pk          BLOB NOT NULL,
    exchange_pk         BLOB NOT NULL,
    display_name        TEXT NOT NULL,
    role                TEXT NOT NULL,       -- 'owner' | 'admin' | 'member'
    added_at            INTEGER NOT NULL,
    added_by            BLOB NOT NULL,
    is_removed          INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (chat_id, peer_id)
);

CREATE TABLE chat_keys (
    chat_id             BLOB NOT NULL,
    key_epoch           INTEGER NOT NULL,
    group_key_enc       BLOB NOT NULL,
    created_at          INTEGER NOT NULL,
    PRIMARY KEY (chat_id, key_epoch)
);

CREATE TABLE messages (
    message_id          BLOB PRIMARY KEY,
    chat_id             BLOB NOT NULL,
    author_peer_id      BLOB NOT NULL,
    lamport_ts          INTEGER NOT NULL,
    created_at          INTEGER NOT NULL,
    key_epoch           INTEGER NOT NULL,
    parent_ids          BLOB,               -- CBOR-encoded
    signature           BLOB NOT NULL,
    payload_ciphertext  BLOB NOT NULL,
    payload_nonce       BLOB NOT NULL,
    received_at         INTEGER NOT NULL,
    UNIQUE(chat_id, lamport_ts, author_peer_id)
);
CREATE INDEX idx_messages_chat_lamport ON messages(chat_id, lamport_ts);

CREATE TABLE frontiers (
    chat_id             BLOB NOT NULL,
    author_peer_id      BLOB NOT NULL,
    max_lamport_ts      INTEGER NOT NULL,
    message_count       INTEGER NOT NULL,
    PRIMARY KEY (chat_id, author_peer_id)
);

CREATE TABLE outbox (
    message_id          BLOB NOT NULL,
    target_peer_id      BLOB NOT NULL,
    chat_id             BLOB NOT NULL,
    created_at          INTEGER NOT NULL,
    PRIMARY KEY (message_id, target_peer_id)
);
CREATE INDEX idx_outbox_target ON outbox(target_peer_id);

CREATE TABLE peer_addresses (
    peer_id             BLOB NOT NULL,
    address_type        TEXT NOT NULL,
    address             TEXT NOT NULL,
    last_seen           INTEGER NOT NULL,
    last_successful     INTEGER,
    fail_count          INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (peer_id, address_type, address)
);

CREATE TABLE sync_log (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp           INTEGER NOT NULL,
    peer_id             BLOB,
    event_type          TEXT NOT NULL,       -- 'connect' | 'disconnect' | 'sync' | 'send' | 'recv'
    detail              TEXT
);
CREATE INDEX idx_sync_log_ts ON sync_log(timestamp DESC);
```

### 7.2 Политика хранения

- Сообщения хранятся бессрочно (пока пользователь не очистит).
- Опциональный TTL на чат.
- Outbox записи удаляются после подтверждения доставки.
- `peer_addresses` с `fail_count > 10` и `last_seen` > 30 дней — автоудаление.
- `sync_log` — хранится последние 1000 записей (ring buffer).

---

## 8. Сетевой уровень (Net)

### 8.1 Транспорт

**Протокол:** TCP + Noise Protocol Framework (Noise_XX_25519_ChaChaPoly_BLAKE2b).
**Порт:** 9473 (по умолчанию, настраивается).

### 8.2 Handshake (Noise_XX)

```
1. I → R:  e
2. R → I:  e, ee, s, es
3. I → R:  s, se
4. I → R:  AuthHello { peer_id, signing_pk, protocol_version }
5. R → I:  AuthHello { peer_id, signing_pk, protocol_version }
```

### 8.3 Обнаружение пиров (Discovery)

**Уровень 1 — LAN (mDNS):** сервис `_ghostmesh._tcp.local`, TXT: `peer_id=<hex>`.
**Уровень 2 — Известные адреса:** из invite-кодов и gossip peer exchange.
**Уровень 3 — Ручной ввод:** пользователь вводит `IP:port`.

NAT traversal: вне скоупа V1.

### 8.4 Wire-протокол

Length-prefixed frames поверх Noise:

```
┌───────────┬──────────────────────────┐
│ length:u32│      payload (CBOR)      │
│ (4 bytes) │    (до 256 KiB)         │
└───────────┴──────────────────────────┘
```

### 8.5 Типы протокольных сообщений

```rust
enum WireMessage {
    // Sync
    SyncRequest { chat_id, frontier: Vec<FrontierEntry> },
    SyncResponse { chat_id, messages: Vec<Message>, frontier },
    SyncAck { chat_id, received: Vec<MessageId> },

    // Chat management
    JoinRequest { chat_id, invite_token, identity },
    JoinResponse { accepted, group_key_enc, members, recent_messages },

    // Peer discovery
    PeerExchange { chat_id, peers: Vec<PeerInfo> },

    // Keepalive
    Ping { timestamp },
    Pong { timestamp },
}
```

---

## 9. Протокол синхронизации (Sync)

### 9.1 Алгоритм

```
A подключился к B.
Для каждого общего чата:

  A → B:  SyncRequest { chat_id, frontier_A }
  B → A:  SyncResponse { diff_messages, frontier_B }
  A → B:  SyncResponse { diff_messages, frontier_A }
  B → A:  SyncAck { received_ids }
  A:      удаляет из outbox записи для B
```

### 9.2 Frontier

```rust
struct FrontierEntry {
    author_peer_id: PeerId,
    max_lamport_ts: u64,
    message_count:  u64,
}
```

### 9.3 Непрерывная синхронизация

Пока подключены: push новых сообщений, re-sync каждые 60 сек, PeerExchange каждые 5 мин.

### 9.4 Интеграция с Tauri

Sync engine работает в фоне через `tauri::async_runtime::spawn`. При получении нового сообщения — `app.emit("message:new", ...)`. При изменении статуса соединения — `app.emit("peer:connected", ...)`. Фронтенд реактивно обновляет UI через Svelte stores.

---

## 10. Криптография (Crypto)

### 10.1 Алгоритмы

| Назначение | Алгоритм | Crate |
|---|---|---|
| Подпись | Ed25519 | `ed25519-dalek` |
| Обмен ключами | X25519 DH | `x25519-dalek` |
| Симметричное шифрование | ChaCha20-Poly1305 | `chacha20poly1305` |
| Хэширование | BLAKE2b-256 | `blake2` |
| KDF | HKDF-BLAKE2b | `hkdf` |
| Шифрование хранилища | AES-256-GCM + Argon2id | `aes-gcm`, `argon2` |
| Транспорт | Noise_XX_25519_ChaChaPoly_BLAKE2b | `snow` |

### 10.2 Шифрование сообщений

```
Отправка:
  plaintext   = CBOR(MessagePayload)
  nonce       = random 24 bytes
  ciphertext  = ChaCha20-Poly1305(group_key[epoch], nonce, plaintext)
  header      = (message_id, chat_id, author, lamport_ts, ...)
  signature   = Ed25519_Sign(signing_sk, header ‖ ciphertext ‖ nonce)

Получение:
  1. Проверить signature.
  2. Найти group_key по key_epoch.
  3. Расшифровать.
  4. Декодировать CBOR.
```

### 10.3 Forward secrecy

V1: периодическая ротация group_key. V2: MLS для полного PFS.

---

## 11. UX / Экраны

### 11.1 Дизайн-направление

По итогам исследования 20 концепций, выбрано направление на пересечении "разработчик + радиолюбитель + параноик + современный технарь". Ключевые принципы:
- Информационная плотность: видны lamport clocks, fingerprints, sync-статусы.
- Темная тема по умолчанию.
- Моноширинный шрифт для технической информации.
- Возможность переключения между режимами: "чистый чат" ↔ "inspector" ↔ "dashboard".

### 11.2 Список экранов

**1. Главный экран — "Эфир"**
- Sidebar: список чатов с индикаторами `online/total`.
- Status bar: connected peers, outbox count, port.
- Глобальный индикатор сети.

**2. Чат**
- Лента сообщений с lamport-нумерацией.
- Статус доставки: `[ALL]` / `[2/4]` / `[queued]`.
- Inline metadata: timestamp, author fingerprint (short).
- Поле ввода.

**3. Inspector Panel (toggle)**
- Правая панель: детали выбранного сообщения (message_id, lamport, signature, encryption, delivery ACKs).
- Members с fingerprints.
- Outbox для текущего чата.

**4. Network / Dashboard**
- Метрики: peers, messages, outbox, lamport, epoch.
- Список активных соединений с адресами.
- Sync-лог.
- Опционально: граф топологии.

**5. Settings**
- Identity: display_name, fingerprint, export/import.
- Network: порт, mDNS toggle, manual peers.
- Storage: размер базы, TTL, очистка.

**6. Invite / Join**
- Генерация invite-кода (текст + QR).
- Вставка invite-кода для присоединения.

### 11.3 Статусы доставки

```
[queued]  В outbox (нет подключённых пиров)
[N/M]    Доставлено N из M участников
[ALL]    Доставлено всем
```

---

## 12. Технический стек

### 12.1 Rust Backend (Tauri + Core)

```toml
[package]
name = "ghostmesh"
version = "0.1.0"
edition = "2021"

[dependencies]
# Tauri
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-dialog = "2"           # нативные диалоги (save/open file)
tauri-plugin-clipboard-manager = "2" # копирование invite-кодов
tauri-plugin-notification = "2"     # уведомления о новых сообщениях

# Криптография
ed25519-dalek = "2"
x25519-dalek = "2"
chacha20poly1305 = "0.10"
blake2 = "0.10"
hkdf = "0.12"
argon2 = "0.5"
aes-gcm = "0.10"
snow = "0.9"                        # Noise Protocol
rand = "0.8"

# Сеть
tokio = { version = "1", features = ["full"] }
mdns-sd = "0.11"                    # mDNS discovery

# Хранение
rusqlite = { version = "0.31", features = ["bundled"] }

# Сериализация
serde = { version = "1", features = ["derive"] }
serde_json = "1"                    # для IPC
ciborium = "0.2"                    # CBOR для wire protocol

# Утилиты
thiserror = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
```

### 12.2 Frontend (Svelte + TypeScript)

```json
{
  "devDependencies": {
    "@sveltejs/vite-plugin-svelte": "^4.0",
    "@tauri-apps/cli": "^2",
    "svelte": "^5",
    "typescript": "^5",
    "vite": "^6"
  },
  "dependencies": {
    "@tauri-apps/api": "^2",
    "@tauri-apps/plugin-dialog": "^2",
    "@tauri-apps/plugin-clipboard-manager": "^2",
    "@tauri-apps/plugin-notification": "^2",
    "qrcode": "^1.5"
  }
}
```

### 12.3 Почему Svelte

- Минимальный runtime (~2 KB), всё компилируется в vanilla JS. Маленький WebView bundle.
- Reactivity из коробки через `$state` и `$derived` (Svelte 5 runes).
- Нативная работа с Tauri events — `listen()` → reactive store.
- Svelte 5 — зрелый фреймворк, хорошая документация.

Альтернативы рассмотренные и отклонённые:
- **React** — тяжелее, virtual DOM overhead в WebView, лишний для задачи.
- **Vanilla JS** — возможно для прототипа, но без reactivity управлять состоянием чатов/пиров мучительно.
- **SolidJS** — хорошая альтернатива, но экосистема и документация меньше.

### 12.4 Платформы сборки

| Платформа | Rust target | WebView |
|---|---|---|
| macOS | `aarch64-apple-darwin` / `x86_64-apple-darwin` | WKWebView |
| Linux | `x86_64-unknown-linux-gnu` | WebKitGTK |
| Windows | `x86_64-pc-windows-msvc` | WebView2 (Edge) |
| Android (V2) | `aarch64-linux-android` | Android WebView |
| iOS (V2) | `aarch64-apple-ios` | WKWebView |

---

## 13. Структура проекта

```
ghost-mesh/
├── src-tauri/                      # Rust backend (Tauri app)
│   ├── Cargo.toml
│   ├── tauri.conf.json             # Tauri конфигурация
│   ├── capabilities/               # Tauri permissions
│   │   └── default.json
│   ├── icons/                      # иконки приложения
│   └── src/
│       ├── main.rs                 # Tauri entry point, setup, command registration
│       ├── commands/               # #[tauri::command] функции
│       │   ├── mod.rs
│       │   ├── identity.rs
│       │   ├── chats.rs
│       │   ├── messages.rs
│       │   ├── network.rs
│       │   └── settings.rs
│       ├── core/                   # бизнес-логика (переиспользуется CLI)
│       │   ├── mod.rs
│       │   ├── crypto/
│       │   │   ├── mod.rs
│       │   │   ├── identity.rs     # генерация/хранение ключей
│       │   │   ├── encrypt.rs      # ChaCha20-Poly1305
│       │   │   ├── sign.rs         # Ed25519
│       │   │   ├── exchange.rs     # X25519
│       │   │   └── noise.rs        # Noise Protocol обёртка
│       │   ├── sync/
│       │   │   ├── mod.rs
│       │   │   ├── frontier.rs
│       │   │   ├── engine.rs
│       │   │   └── lamport.rs
│       │   ├── net/
│       │   │   ├── mod.rs
│       │   │   ├── transport.rs    # TCP + Noise
│       │   │   ├── discovery.rs    # mDNS
│       │   │   ├── peer_manager.rs
│       │   │   └── wire.rs         # wire protocol
│       │   └── store/
│       │       ├── mod.rs
│       │       ├── db.rs           # SQLite init, migrations
│       │       ├── messages.rs
│       │       ├── chats.rs
│       │       └── outbox.rs
│       ├── events.rs               # emit helpers
│       ├── state.rs                # AppState (shared across commands)
│       └── types.rs                # IPC types (Serialize/Deserialize)
├── src/                            # Svelte frontend
│   ├── App.svelte                  # root component
│   ├── main.ts                     # entry point
│   ├── lib/
│   │   ├── api.ts                  # typed invoke() wrappers
│   │   ├── events.ts               # typed listen() wrappers
│   │   └── stores/
│   │       ├── chats.ts            # reactive chat list
│   │       ├── messages.ts         # reactive message feed
│   │       ├── network.ts          # reactive peer/connection state
│   │       └── identity.ts         # current user
│   ├── components/
│   │   ├── Sidebar.svelte
│   │   ├── ChatView.svelte
│   │   ├── MessageRow.svelte
│   │   ├── InspectorPanel.svelte
│   │   ├── NetworkDashboard.svelte
│   │   ├── PeerIndicator.svelte
│   │   ├── InviteDialog.svelte
│   │   └── Settings.svelte
│   └── styles/
│       ├── global.css              # тема, переменные, шрифты
│       └── themes/                 # альтернативные темы
│           ├── default.css         # modern dark
│           ├── bbs.css             # ANSI BBS стиль
│           ├── amber.css           # cypherpunk amber
│           └── paranoid.css        # minimal/stealth
├── cli/                            # CLI для тестирования (отдельный binary)
│   ├── Cargo.toml                  # зависимость от ../src-tauri/src/core
│   └── src/
│       └── main.rs
├── package.json
├── vite.config.ts
├── svelte.config.js
├── tsconfig.json
└── README.md
```

### 13.1 Переиспользование core между Tauri и CLI

`src-tauri/src/core/` — чистая библиотека без зависимости от Tauri. CLI импортирует её напрямую. Tauri commands — тонкие обёртки, которые вызывают core и эмитят события.

```rust
// src-tauri/src/commands/messages.rs
#[tauri::command]
async fn send_message(
    state: State<'_, AppState>,
    app: AppHandle,
    chat_id: String,
    text: String,
) -> Result<MessageInfo, Error> {
    // Вызываем core
    let msg = state.core.send_message(&chat_id, &text).await?;

    // Эмитим событие во фронтенд
    app.emit("message:new", &msg)?;

    Ok(msg.into())
}
```

---

## 14. Tauri-специфичные фичи

### 14.1 System Tray

GhostMesh работает в tray: закрытие окна не убивает приложение. Sync продолжается в фоне. Tray-иконка показывает статус:
- Зелёная точка: есть подключённые пиры.
- Жёлтая: outbox не пуст.
- Серая: офлайн.

Tray-меню: "Открыть", "Статус: 3 пира / 12 в outbox", "Выход".

### 14.2 Notifications

При новом сообщении в неактивном чате — нативное уведомление через `tauri-plugin-notification`. Содержимое уведомления: "Dev Team: Bob — <текст>" (без раскрытия содержимого в paranoid-режиме).

### 14.3 Deep Links

Регистрация протокола `ghm://` через Tauri. Клик на invite-ссылку `ghm://a7f3b291c4e8...` открывает приложение и начинает процесс присоединения.

### 14.4 Auto-updater

Tauri встроенный updater (опционально, если будет механизм распространения). Для параноиков — отключается в настройках.

### 14.5 Темизация

CSS-переменные в `global.css`. Пользователь выбирает тему в настройках. Фронтенд переключает CSS-файл. Доступные темы:
- **Default** — современный тёмный (GitHub dark стиль).
- **BBS** — ANSI-цвета, VT323 шрифт.
- **Amber** — янтарный монохром.
- **Paranoid** — минимум информации, приглушённые цвета.

---

## 15. Безопасность: модель угроз

### 15.1 От чего защищаем

| Угроза | Защита |
|---|---|
| Перехват трафика | Noise Protocol |
| Чтение сообщений без ключа | ChaCha20-Poly1305 |
| Подделка сообщений | Ed25519 подпись |
| Подмена участника | Верификация ключей через invite-код / личная встреча |
| Утечка при компрометации устройства | Argon2id + AES-256-GCM для хранилища ключей |
| Бывший участник читает новые сообщения | Ротация group_key |

### 15.2 Tauri-специфичные аспекты

| Аспект | Решение |
|---|---|
| WebView XSS | Tauri CSP по умолчанию: `default-src 'self'`. Никаких внешних скриптов. |
| IPC attack surface | Tauri capabilities: каждая команда явно разрешена в `capabilities/default.json`. |
| Local file access | Запрещён из WebView. Все операции с файлами — через Rust-бэкенд. |
| Sensitive data в WebView | Ключи НИКОГДА не передаются во фронтенд. Только peer_id, fingerprints, зашифрованные данные. |

### 15.3 Вне скоупа V1

Metadata leakage, forward secrecy (MLS), DoS protection, onion routing — V2.

---

## 16. Этапы разработки

### Phase 1 — CLI Prototype (Rust only) [2-3 недели]

**Цель:** Рабочий sync-протокол между двумя нодами.

- [ ] Генерация Identity.
- [ ] SQLite схема и CRUD.
- [ ] Noise handshake по TCP.
- [ ] Создание чата, ручное добавление пира.
- [ ] Отправка текстовых сообщений.
- [ ] Frontier-based sync.
- [ ] CLI: `send <chat> <text>`, `read <chat>`, `peers`, `sync`.

### Phase 2 — Tauri Desktop [3-4 недели]

**Цель:** Работающий десктопный мессенджер.

- [ ] `cargo tauri init`, структура проекта.
- [ ] Перенос core в `src-tauri/src/core/`.
- [ ] Реализация всех `#[tauri::command]`.
- [ ] Реализация event emitters.
- [ ] Svelte UI: sidebar, chat view, input.
- [ ] mDNS discovery.
- [ ] System tray.
- [ ] Invite-код (генерация + вставка).
- [ ] Базовая тема (default dark).

### Phase 3 — Polish [2-3 недели]

- [ ] Inspector panel (packet details).
- [ ] Network dashboard.
- [ ] Settings screen.
- [ ] Notifications.
- [ ] Альтернативные темы (BBS, amber, paranoid).
- [ ] QR-код генерация/сканирование (webcam).
- [ ] Keyboard shortcuts.
- [ ] Deep links (`ghm://`).

### Phase 4 — Hardening [2-3 недели]

- [ ] Ротация ключей (автоматическая + при удалении).
- [ ] TTL сообщений.
- [ ] Экспорт/импорт Identity.
- [ ] Rate limiting.
- [ ] CSP hardening.
- [ ] Audit криптографии.

### Phase 5 — Mobile (Tauri 2 Mobile) [4-6 недель]

- [ ] Tauri mobile setup (Android + iOS).
- [ ] Адаптация UI для touch.
- [ ] Камера для QR (tauri-plugin-barcode-scanner).
- [ ] Background sync (platform-specific).
- [ ] Battery/traffic оптимизация.

### Phase 6 — V2 Features

- [ ] Медиа-сообщения (chunked transfer).
- [ ] Onion routing / Tor transport.
- [ ] MLS для forward secrecy.
- [ ] Relay-ноды (опциональные).
- [ ] Multi-device linking.

---

## 17. Open Questions

1. **Максимальный размер группы?** Лимит V1: 20 участников.

2. **Multi-device?** V1: один ключ = одно устройство. Multi-device — V2.

3. **WebView на Linux?** WebKitGTK может отличаться по поведению/рендерингу от WebView2/WKWebView. Нужно тестировать.

4. **Конфликт ротации ключей?** Два админа одновременно удаляют участников → конфликт эпох. Нужен протокол разрешения.

5. **Удаление сообщений?** Tombstone record в append-only log? Или чисто локальное?

6. **Svelte 5 vs 4?** Svelte 5 (runes) стабилен, но экосистема библиотек ещё мигрирует. Решение: Svelte 5, минимум внешних зависимостей.

7. **Offline-first UI?** При запуске показывать кэшированные данные из SQLite сразу, не дожидаясь сети. Svelte stores инициализируются из `invoke()` при маунте.

---

*Дата: 2026-04-01. Версия: 0.2-draft. Стек: Tauri 2 + Rust + Svelte 5.*