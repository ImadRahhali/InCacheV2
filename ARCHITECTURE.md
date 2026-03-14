# InCacheV2 — Architecture Reference for Diagram Generation

Use this document to generate a detailed architecture diagram of InCacheV2, a Redis-compatible in-memory database written in Rust.

---

## High-Level Overview

InCacheV2 is a **single-threaded, event-driven TCP server** that speaks the Redis RESP2 protocol. Any Redis client can connect to it. The architecture mirrors Redis's `ae.c` event loop — no async runtime, no threads, no locks.

```
Redis Client (redis-py, Jedis, etc.)
        │
        │  TCP connection (RESP2 protocol)
        ▼
┌─────────────────────────────────────────────┐
│              Event Loop (server.rs)          │
│  raw epoll (Linux) / kqueue (macOS)         │
│                                             │
│  loop {                                     │
│    epoll_wait() → ready file descriptors    │
│    for each ready fd:                       │
│      if LISTENER → accept new connection    │
│      if CLIENT   → read → parse → exec →   │
│                    encode → write           │
│  }                                          │
│                                             │
│  Every 100ms: sweep expired keys            │
└─────────────────────────────────────────────┘
```

---

## File Structure and Responsibilities

```
src/
├── main.rs              # Entry point
│                        #   Parses CLI args: --port, --host, --requirepass
│                        #   Sets mimalloc as global allocator
│                        #   Calls server::run_server()
│
├── server.rs            # Event loop + connection management
│                        #   Contains: Poller (epoll/kqueue), Conn, Slowlog
│                        #   Handles: AUTH, MULTI/EXEC/DISCARD, SLOWLOG
│                        #   Delegates normal commands to commands::dispatch()
│
├── protocol.rs          # RESP2 parser + encoder
│                        #   Contains: RespValue enum, Command struct
│                        #   parse_commands() → zero-alloc parsing
│                        #   encode_into() → direct buffer encoding
│
├── store.rs             # In-memory data store
│                        #   Contains: Store, Entry, Value enum
│                        #   FxHashMap<String, Entry> as main keyspace
│                        #   Lazy + active TTL expiry
│
└── commands/
    ├── mod.rs           # Command dispatcher
    │                    #   match on &[u8] → handler function
    ├── strings.rs       # SET, GET, INCR, APPEND, EXPIRE, TTL, KEYS, etc.
    ├── lists.rs         # LPUSH, RPUSH, LPOP, RPOP, LRANGE, LINSERT, etc.
    ├── hashes.rs        # HSET, HGET, HMGET, HGETALL, HINCRBY, etc.
    ├── sets.rs          # SADD, SMEMBERS, SINTER, SUNION, SDIFF, SMOVE, etc.
    └── server.rs        # PING, ECHO, FLUSHALL, DBSIZE, INFO, SELECT, HELLO
```

---

## Data Flow — Single Command (e.g., `SET foo bar`)

```
1. Client sends:  *3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n
                          │
                          ▼
2. epoll_wait() returns client fd as readable
                          │
                          ▼
3. server.rs::handle()
   ├── read() syscall → bytes into conn.read_buf (BytesMut)
   │
   ├── protocol::parse_commands(&conn.read_buf)
   │   └── Returns Vec<Command> where each Command holds
   │       (offset, len) pairs pointing into read_buf
   │       ┌──────────────────────────────────┐
   │       │ Command {                        │
   │       │   ranges: [(12,3), (21,3), (30,3)] │  ← "SET", "foo", "bar"
   │       │   count: 3                       │     as offsets into read_buf
   │       │ }                                │
   │       └──────────────────────────────────┘
   │       Zero heap allocation for typical commands (≤8 args)
   │
   ├── Check AUTH state → if not authenticated, reject
   ├── Check MULTI state → if in transaction, queue command
   │
   ├── commands::dispatch(&mut store, &cmd, buf)
   │   ├── Uppercase first arg on stack: [0u8; 16]
   │   ├── match b"SET" → strings::cmd_set()
   │   │   ├── Parse key as &str (zero-copy from buf)
   │   │   ├── Parse value as Bytes::copy_from_slice (one allocation)
   │   │   ├── Parse optional EX/PX/NX/XX flags
   │   │   ├── store.set_value(key, Value::String(bytes), expires_at)
   │   │   │   └── FxHashMap::insert(key, Entry { value, expires_at })
   │   │   └── Return RespValue::ok()  →  SimpleString("OK")
   │   └── Return RespValue to server.rs
   │
   ├── protocol::encode_into(&result, &mut conn.write_buf)
   │   └── Writes "+OK\r\n" directly into BytesMut
   │
   └── write() syscall → send conn.write_buf to client
```

---

## Data Store (store.rs)

```
Store {
    data: FxHashMap<String, Entry>
}

Entry {
    value: Value,              // the actual data
    expires_at: Option<f64>,   // absolute timestamp or None
}

Value (enum) {
    String(Bytes)                          // GET/SET
    List(VecDeque<Bytes>)                  // LPUSH/RPUSH/LPOP/RPOP
    Hash(FxHashMap<Box<str>, Bytes>)       // HSET/HGET
    Set(FxHashSet<Box<str>>)               // SADD/SMEMBERS
}
```

**TTL expiry — two mechanisms:**
```
Lazy expiry:
  Every key access calls check_expired(key)
  If expires_at <= now() → delete key, return None

Active expiry:
  Every 100ms in the event loop:
    store.sweep_expired()
    → HashMap::retain(|_, e| e.expires_at > now())
  Prevents unbounded memory growth from unread expired keys
```

---

## Connection State (server.rs)

```
Conn {
    stream: TcpStream,          // the TCP socket
    read_buf: BytesMut,         // incoming data buffer (32KB initial)
    write_buf: BytesMut,        // outgoing response buffer (4KB initial)
    authenticated: bool,        // AUTH state
    in_multi: bool,             // inside MULTI transaction?
    queue: Vec<(cmd, args)>,    // queued commands for EXEC
}
```

---

## RESP2 Protocol (protocol.rs)

**Parsing — zero allocation:**
```
Input buffer:  *3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n

Command struct stores offsets into the buffer:
  arg(0) → buf[12..15] = "SET"
  arg(1) → buf[21..24] = "foo"
  arg(2) → buf[30..33] = "bar"

No Vec<Bytes> created. No heap allocation for commands with ≤8 args.
Stack array: [(u32, u32); 8] holds the offset/length pairs.
```

**Encoding — direct to buffer:**
```
RespValue::ok()        → writes "+OK\r\n" (5 bytes)
RespValue::Null        → writes "$-1\r\n" (5 bytes)
RespValue::Integer(42) → writes ":42\r\n" (5 bytes, itoa crate, no format!())
RespValue::BulkString  → writes "$N\r\n{data}\r\n"
RespValue::Array       → writes "*N\r\n" + recursive encode of each element
```

---

## Command Dispatcher (commands/mod.rs)

```
dispatch(&mut Store, &Command, &[u8]) → RespValue

Uppercase the first arg on a stack buffer [0u8; 16]
Match on byte slice:

  b"SET"  → strings::cmd_set()
  b"GET"  → strings::cmd_get()
  b"PING" → server::cmd_ping()
  ...45 commands total...
  _       → RespValue::Error("unknown command")

Compiled to a jump table by LLVM — O(1) dispatch.
```

---

## Event Loop Detail (server.rs)

```
run_server(host, port, password):
    listener = TcpListener::bind(addr)
    listener.set_nonblocking(true)
    poller = Poller::new()           // epoll_create1() or kqueue()
    poller.add(listener_fd, TOKEN=0)

    store = Store::new()
    slowlog = Slowlog::new()
    connections = HashMap<u64, Conn>

    loop:
        n = poller.poll(events, timeout=100ms)

        if 100ms elapsed:
            store.sweep_expired()    // active TTL cleanup

        for event in events[0..n]:
            if event.token == LISTENER:
                loop:
                    stream = listener.accept()  // accept all pending
                    stream.set_nonblocking(true)
                    stream.set_nodelay(true)     // disable Nagle
                    poller.add(stream_fd, new_token)
                    connections.insert(token, Conn::new(stream))

            else:
                handle(&mut conn, &mut store, &mut slowlog, &password)
                if closed: connections.remove(token)
```

---

## AUTH Flow

```
Server started with --requirepass secret123

New connection: conn.authenticated = false

Any command except PING/HELLO/CLIENT/AUTH:
  → "-NOAUTH Authentication required.\r\n"

AUTH secret123:
  → conn.authenticated = true
  → "+OK\r\n"

AUTH wrongpassword:
  → "-ERR invalid password\r\n"
```

---

## MULTI/EXEC Flow

```
Client sends MULTI:
  → conn.in_multi = true
  → "+OK\r\n"

Client sends SET foo bar:
  → command queued in conn.queue
  → "+QUEUED\r\n"

Client sends INCR counter:
  → command queued in conn.queue
  → "+QUEUED\r\n"

Client sends EXEC:
  → execute all queued commands sequentially
  → return results as RESP array:
    *2\r\n+OK\r\n:1\r\n

Client sends DISCARD:
  → clear queue, exit multi
  → "+OK\r\n"
```

---

## SLOWLOG

```
Every command is timed with Instant::now()

If elapsed >= threshold (default 10ms):
  → log entry pushed to front of VecDeque (max 128 entries)
  → entry = { id, timestamp, duration_us, command_string }

SLOWLOG GET [count]  → return latest entries as RESP array
SLOWLOG LEN          → return count
SLOWLOG RESET        → clear all entries
```

---

## Performance-Critical Design Choices

```
1. Raw epoll/kqueue     — no Tokio, no async, no futures, no wakers
2. Zero-alloc parser    — Command holds (offset, len) into read buffer
3. FxHashMap            — non-cryptographic hash, ~3× faster than SipHash
4. mimalloc             — faster than system malloc for small allocations
5. TCP_NODELAY          — disable Nagle's algorithm
6. Stack command match  — match &[u8] compiles to jump table
7. itoa + memchr        — no format!() for integers, SIMD for \r\n scan
8. Single-threaded      — zero locking, zero contention, zero cache bouncing
```

---

## Dependency Graph

```
incache_v2
├── libc          — raw epoll/kqueue syscalls
├── bytes         — BytesMut for read/write buffers, Bytes for values
├── memchr        — SIMD-accelerated \r\n scanning in RESP parser
├── itoa          — fast integer-to-string for RESP encoding
├── mimalloc      — global allocator (replaces system malloc)
└── rustc-hash    — FxHashMap/FxHashSet (fast non-crypto hash)
```

No async runtime. No framework. 6 dependencies total.

---

## Test Architecture

```
tests/ (Python, using redis-py client)
├── conftest.py        — starts the Rust binary as subprocess
│                        waits for TCP port, creates redis.Redis client
│                        FLUSHALL before each test
├── test_strings.py    — 48 tests
├── test_lists.py      — 30 tests
├── test_hashes.py     — 27 tests
├── test_sets.py       — 31 tests
├── test_server.py     — 13 tests
├── test_stress.py     — 14 tests (concurrency, large values, malformed input)
└── test_features.py   — 12 tests (AUTH, MULTI/EXEC, SLOWLOG)

Total: 175 tests
All tests connect over TCP using redis-py — they validate protocol behavior,
not implementation internals. The same tests pass against real Redis.
```
