# InCacheV2

A Redis-compatible in-memory database written in Rust. Speaks the RESP2 protocol — connect with any Redis client, no configuration needed.

This is the Rust rewrite of [InCache](https://github.com/ImadRahhali/InCache) (Python). Same architecture, same protocol, same 149-test quality gate — different language.

```bash
cargo build --release
./target/release/incache_v2 --port 6399
```

```python
import redis
r = redis.Redis(host="localhost", port=6399, decode_responses=True)

r.set("hello", "world", ex=60)
r.get("hello")           # "world"
r.incr("counter")        # 1
r.lpush("queue", "job1")
r.hset("user:1", mapping={"name": "Imad", "role": "engineer"})
r.sadd("tags", "rust", "cache", "redis")
```

---

## Benchmarks

All benchmarks on **Apple M4 Pro**, macOS, 100k operations (10k for LRANGE), 10 parallel clients, using `valkey-benchmark`. All three tested in the same session for a fair comparison.

### Simple commands (ops/sec)

| Command | InCacheV2 (Rust) | InCache (Python) | Rust vs Python | Redis 8.6.1 (macOS) | Redis 8.6.1 (Linux)* |
|---------|-----------------|-----------------|----------------|---------------------|----------------------|
| SET     | **73,475**      | 55,991          | 1.3×           | 7,184               | ~110,000             |
| GET     | **78,064**      | 57,471          | 1.4×           | 3,903               | ~120,000             |
| INCR    | **78,064**      | 57,637          | 1.4×           | 4,849               | ~110,000             |
| LPUSH   | **77,519**      | 56,117          | 1.4×           | 4,480               | ~110,000             |
| HSET    | **78,186**      | 55,371          | 1.4×           | 6,538               | ~105,000             |

*Linux estimates from Redis official benchmarks on comparable hardware.

### LRANGE (ops/sec)

| Elements | InCacheV2 (Rust) | InCache (Python) | Rust vs Python |
|----------|-----------------|-----------------|----------------|
| 100      | **59,524**      | 24,213          | **2.5×**       |
| 300      | **40,161**      | 12,547          | **3.2×**       |
| 500      | **30,581**      | 8,584           | **3.6×**       |
| 600      | **27,933**      | 7,463           | **3.7×**       |

### Latency (p50 / p99, milliseconds)

| Command | InCacheV2 (Rust)    | InCache (Python)  | Redis 8.6.1 (macOS) |
|---------|---------------------|-------------------|---------------------|
| SET     | **0.119 / 0.159**   | 0.167 / 0.199     | 1.335 / 3.807       |
| GET     | **0.119 / 0.199**   | 0.167 / 0.191     | 2.023 / 7.079       |
| INCR    | **0.119 / 0.175**   | 0.167 / 0.239     | 1.415 / 6.511       |
| LPUSH   | **0.119 / 0.167**   | 0.175 / 0.215     | 1.511 / 6.759       |
| HSET    | **0.119 / 0.159**   | 0.175 / 0.215     | 1.335 / 4.775       |

### Analysis

**Rust vs Python — simple commands: ~1.4× faster.**

The gap is modest because both implementations are bottlenecked by the same thing: a single lock serialising all data access. In Python it's `asyncio.Lock`, in Rust it's `DashMap`'s per-shard locks. The Rust advantage comes from:
- No interpreter overhead, no GC pauses
- Zero-copy `Bytes` values instead of Python object allocations
- `memchr` + `itoa` for fast RESP parsing/encoding
- `BufWriter` batching TCP writes

**Rust vs Python — LRANGE: 2.5–3.7× faster.**

This is where Rust shines. Python's `itertools.islice` over a `deque` still has interpreter overhead per element. Rust's `VecDeque::iter().skip().take()` compiles to a tight pointer-chasing loop with zero allocation. The gap widens with more elements (3.7× at 600 elements).

**Why do both InCache versions beat Redis on macOS?**

They don't — not in any meaningful sense. **On Linux, Redis runs at 100k–120k ops/sec**, faster than both. Redis is optimised for Linux `epoll`; on macOS it falls back to `kqueue` and performs significantly worse. Both Tokio and Python's `asyncio` handle macOS `kqueue` efficiently for this workload.

### Optimisations in V2

Compared to the initial Rust implementation:

| Optimisation | Impact |
|---|---|
| `DashMap` (lock-free concurrent hashmap) | Replaces single `Mutex<Store>` — per-shard locking |
| `bytes::Bytes` (reference-counted) | Zero-copy value storage, no `Vec<u8>` cloning |
| `memchr` for CRLF scanning | SIMD-accelerated `\r\n` search in RESP parser |
| `itoa` for integer encoding | Avoids `format!()` allocation for `:42\r\n` |
| `encode_into(BytesMut)` | Encodes directly into write buffer, no intermediate `Vec` |
| `BufWriter<OwnedWriteHalf>` | Batches TCP writes, fewer syscalls |
| Stack-based command dispatch | `match` on `&[u8]` — no `String` allocation per command |
| `Box<str>` for hash/set keys | 1 word smaller than `String` |
| LTO + codegen-units=1 | Whole-program optimisation in release builds |

### Run benchmarks yourself

```bash
# InCacheV2 (Rust)
cargo build --release
./target/release/incache_v2 --port 6399 &
valkey-benchmark -p 6399 -t set,get,incr,lpush,hset -n 100000 -c 10
valkey-benchmark -p 6399 -t lrange -n 10000 -c 10

# InCache (Python)
pip install incache
python -m incache --port 6399 &
valkey-benchmark -p 6399 -t set,get,incr,lpush,hset -n 100000 -c 10
```

---

## Features

**Data structures**

- **Strings** — `SET`, `GET`, `MSET`, `MGET`, `GETSET`, `SETNX`, `SETEX`, `INCR`, `INCRBY`, `DECR`, `DECRBY`, `APPEND`, `STRLEN`
- **Lists** — `LPUSH`, `RPUSH`, `LPOP`, `RPOP`, `LRANGE`, `LLEN`, `LINDEX`, `LSET`, `LINSERT`, `LREM`
- **Hashes** — `HSET`, `HGET`, `HMSET`, `HMGET`, `HGETALL`, `HDEL`, `HEXISTS`, `HLEN`, `HKEYS`, `HVALS`, `HINCRBY`
- **Sets** — `SADD`, `SMEMBERS`, `SREM`, `SISMEMBER`, `SCARD`, `SUNION`, `SINTER`, `SDIFF`, `SMOVE`, `SPOP`

**TTL / expiry**

- `EXPIRE`, `TTL`, `PERSIST`, `SETEX`, `SET EX/PX/NX/XX`
- Lazy expiry — checked on every key access
- Active expiry sweep — background Tokio task runs every 100ms

**Key commands** — `TYPE`, `RENAME`, `KEYS` (glob patterns), `EXISTS`, `DEL`

**Server** — `PING`, `ECHO`, `FLUSHALL`, `FLUSHDB`, `DBSIZE`, `INFO`, `SELECT`, `COMMAND COUNT`, `HELLO`

**Protocol** — full RESP2, pipelining, partial frame reads, inline commands

---

## Architecture

```
src/
├── main.rs              # CLI entrypoint (--host, --port)
├── server.rs            # Tokio TCP server — one task per connection
│                        #   BufWriter for batched writes
├── protocol.rs          # RESP2 parser + serialiser
│                        #   memchr for CRLF scanning
│                        #   itoa for integer encoding
│                        #   encode_into() writes directly to BytesMut
├── store.rs             # In-memory store (DashMap — lock-free)
│                        #   lazy expiry on every key access
│                        #   active sweep task every 100ms
│                        #   closure-based API to minimise lock scope
└── commands/
    ├── mod.rs           # Command dispatcher — match on &[u8] slices
    ├── strings.rs       # String + key commands
    ├── lists.rs         # List commands (VecDeque)
    ├── hashes.rs        # Hash commands (HashMap)
    ├── sets.rs          # Set commands (HashSet)
    └── server.rs        # Server commands + HELLO handshake
```

| Concept | InCache (Python) | InCacheV2 (Rust) |
|---------|-----------------|-----------------|
| Async runtime | `asyncio` | `tokio` |
| Concurrency | `asyncio.Lock` (global) | `DashMap` (per-shard) |
| Value storage | Python objects | `bytes::Bytes` (zero-copy) |
| RESP parsing | byte string slicing | `memchr` SIMD + `BytesMut` |
| Integer encoding | `f":{n}\r\n"` | `itoa` (no allocation) |
| TCP writes | direct `writer.write()` | `BufWriter` (batched) |
| List values | `collections.deque` | `VecDeque` |
| Hash values | `dict` | `HashMap<Box<str>, Bytes>` |
| Set values | `set` | `HashSet<Box<str>>` |
| Command dispatch | `dict[name] → fn` | `match &[u8]` (zero-alloc) |

---

## Tests

149 tests — the same test suite that validates [InCache](https://github.com/ImadRahhali/InCache). Written in Python against the `redis-py` client, because the tests validate RESP protocol behaviour, not implementation internals.

```bash
pip install redis pytest pytest-asyncio
pytest tests/ -v
```

```
tests/test_strings.py   48 tests — SET/GET flags, TTL, INCR, APPEND, TYPE, KEYS, RENAME
tests/test_lists.py     30 tests — push/pop, LRANGE, LINDEX, LSET, LINSERT, LREM
tests/test_hashes.py    27 tests — HSET, HMGET, HGETALL, HINCRBY, HEXISTS
tests/test_sets.py      31 tests — SADD, set operations, SMOVE, SPOP
tests/test_server.py    13 tests — PING, ECHO, FLUSH, DBSIZE, SELECT, INFO
```

---

## Building

```bash
cargo build --release
./target/release/incache_v2                     # default: 0.0.0.0:6399
./target/release/incache_v2 --port 6380         # custom port
```

Dependencies: `tokio` · `bytes` · `dashmap` · `itoa` · `memchr`

---

## Limitations

InCacheV2 is a learning project, not a production database:

- No persistence — data is lost on restart
- No replication, clustering, Lua scripting, sorted sets, streams, or pub/sub
- No authentication, ACLs, or TLS

For production workloads, use [Redis](https://redis.io) or [Valkey](https://valkey.io).

---

## See Also

- [InCache](https://github.com/ImadRahhali/InCache) — the original Python implementation

---

## License

MIT
