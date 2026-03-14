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

All benchmarks on **Apple M4 Pro**, macOS, 100k operations (10k for LRANGE), 10 parallel clients, using `valkey-benchmark`.

### Simple commands (ops/sec)

| Command | InCacheV2 (Rust) | InCache (Python) | Redis 8.6.1 (macOS) | Redis 8.6.1 (Linux)* |
|---------|-----------------|-----------------|---------------------|----------------------|
| SET     | **76,746**      | 57,142          | 7,184               | ~110,000             |
| GET     | **77,220**      | 59,241          | 3,903               | ~120,000             |
| INCR    | **78,064**      | 61,236          | 4,849               | ~110,000             |
| LPUSH   | **78,740**      | 59,808          | 4,480               | ~110,000             |
| HSET    | **78,431**      | 58,139          | 6,538               | ~105,000             |

*Linux estimates from Redis official benchmarks on comparable hardware.

### LRANGE (ops/sec)

| Elements | InCacheV2 (Rust) | InCache (Python) | Redis 8.6.1 (macOS) |
|----------|-----------------|-----------------|---------------------|
| 100      | **63,291**      | 24,038          | 2,868               |
| 300      | **35,714**      | 12,690          | 5,213               |
| 500      | **23,981**      | 8,568           | 5,393               |
| 600      | **20,492**      | 7,342           | 4,494               |

### Latency (p50 / p99, milliseconds)

| Command | InCacheV2 (Rust) | InCache (Python) | Redis 8.6.1 (macOS) |
|---------|-----------------|-----------------|---------------------|
| SET     | **0.119 / 0.175** | 0.167 / 0.263 | 1.335 / 3.807       |
| GET     | **0.119 / 0.191** | 0.159 / 0.223 | 2.023 / 7.079       |
| INCR    | **0.119 / 0.167** | 0.159 / 0.191 | 1.415 / 6.511       |
| LPUSH   | **0.119 / 0.159** | 0.159 / 0.199 | 1.511 / 6.759       |
| HSET    | **0.119 / 0.159** | 0.167 / 0.207 | 1.335 / 4.775       |

### Analysis

**Rust vs Python (InCacheV2 vs InCache):**
- Simple commands: Rust is **~35% faster** (77k vs 58k ops/sec)
- LRANGE: Rust is **2.6–2.8× faster** — the biggest win, thanks to zero-copy iteration over `VecDeque` vs Python's `itertools.islice` over `deque`
- Latency: Rust shaves ~30% off p50 and p99 across the board

**Why isn't Rust 10× faster?**

The bottleneck isn't CPU — it's the `tokio::sync::Mutex` serialising all commands through a single lock. Every connection awaits the same mutex, which means the server is effectively single-threaded for data access. This is the same design as the Python version (single `asyncio.Lock`). The Rust advantage shows up in:
1. Lower per-command overhead (no interpreter, no GC)
2. More efficient memory layout (no Python object headers)
3. Much faster LRANGE (VecDeque iterator vs deque-to-list conversion)

To reach 300k+ ops/sec, the next step would be replacing `Mutex<Store>` with a lock-free concurrent hashmap like `DashMap`.

**Why do both InCache versions beat Redis on macOS?**

They don't — not in any meaningful sense. **On Linux, Redis runs at 100k–120k ops/sec**, faster than both. Redis is optimised for Linux `epoll`; on macOS it falls back to `kqueue` and performs significantly worse. Both Tokio and Python's `asyncio` handle macOS `kqueue` efficiently for this workload.

### Run benchmarks yourself

```bash
# InCacheV2
./target/release/incache_v2 --port 6399 &
valkey-benchmark -p 6399 -t set,get,incr,lpush,hset -n 100000 -c 10
valkey-benchmark -p 6399 -t lrange -n 10000 -c 10
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

InCacheV2 mirrors the [InCache](https://github.com/ImadRahhali/InCache) Python architecture 1:1, translated to idiomatic Rust:

```
src/
├── main.rs              # CLI entrypoint (--host, --port)
├── server.rs            # Tokio TCP server — one task per connection
├── protocol.rs          # RESP2 parser + serialiser (bytes crate)
├── store.rs             # In-memory store with TTL (HashMap + Mutex)
│                        #   lazy expiry on every key access
│                        #   active sweep task every 100ms
└── commands/
    ├── mod.rs           # Command dispatcher — match name → handler
    ├── strings.rs       # String + key commands
    ├── lists.rs         # List commands (VecDeque for O(1) push/pop)
    ├── hashes.rs        # Hash commands (HashMap)
    ├── sets.rs          # Set commands (HashSet)
    └── server.rs        # Server commands + HELLO handshake
```

| Concept | InCache (Python) | InCacheV2 (Rust) |
|---------|-----------------|-----------------|
| Async runtime | `asyncio` | `tokio` |
| TCP server | `asyncio.start_server` | `TcpListener::bind` |
| Concurrency lock | `asyncio.Lock` | `tokio::sync::Mutex` |
| Buffer management | `bytes` concatenation | `bytes::BytesMut` |
| List values | `collections.deque` | `VecDeque` |
| Hash values | `dict` | `HashMap` |
| Set values | `set` | `HashSet` |
| LRANGE optimisation | `itertools.islice` | `.iter().skip().take()` |
| Glob matching | `fnmatch.fnmatch` | Custom recursive matcher |

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

Dependencies: `tokio` (async runtime) + `bytes` (buffer management). Nothing else.

---

## Limitations

InCacheV2 is a learning project, not a production database:

- No persistence — data is lost on restart
- No replication, clustering, Lua scripting, sorted sets, streams, or pub/sub
- No authentication, ACLs, or TLS
- Single mutex — all commands serialised through one lock

For production workloads, use [Redis](https://redis.io) or [Valkey](https://valkey.io).

---

## See Also

- [InCache](https://github.com/ImadRahhali/InCache) — the original Python implementation

---

## License

MIT
