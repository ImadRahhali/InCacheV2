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

All benchmarks using `redis-benchmark` (from Redis 7.2.7), 100k operations (10k for LRANGE), 10 parallel clients.

- **Mac**: Apple M4 Pro, macOS
- **Linux**: Intel Xeon Platinum 8175M (8c/16t @ 2.50GHz), Amazon Linux 2

### Linux — Simple commands (ops/sec)

| Command | InCacheV2 (Rust) | InCache (Python) | Redis 7.2.7 | Rust vs Redis |
|---------|-----------------|-----------------|-------------|---------------|
| SET     | **80,192**      | 36,657          | 88,106      | 91%           |
| GET     | **80,257**      | 36,873          | 86,580      | 93%           |
| INCR    | **80,515**      | 35,907          | 86,730      | 93%           |
| LPUSH   | **81,699**      | 34,614          | 87,566      | 93%           |
| HSET    | **81,037**      | 33,933          | 87,413      | 93%           |

### Linux — LRANGE (ops/sec)

| Elements | InCacheV2 (Rust) | InCache (Python) | Redis 7.2.7 | Rust vs Redis |
|----------|-----------------|-----------------|-------------|---------------|
| 100      | **46,296**      | 11,261          | 49,020      | 94%           |
| 300      | **20,408**      | 5,015           | 20,450      | **100%**      |
| 500      | **14,085**      | 3,118           | 14,065      | **100%**      |
| 600      | **12,195**      | 2,740           | 12,255      | **100%**      |

### macOS — Simple commands (ops/sec)

| Command | InCacheV2 (Rust) | InCache (Python) | Redis 7.2.7 |
|---------|-----------------|-----------------|-------------|
| SET     | **78,125**      | 55,432          | 7,689       |
| GET     | **83,612**      | 56,275          | 2,664       |
| INCR    | **84,818**      | 52,938          | 3,297       |
| LPUSH   | **83,752**      | 54,377          | 7,337       |
| HSET    | **84,459**      | 54,083          | 2,682       |

### macOS — LRANGE (ops/sec)

| Elements | InCacheV2 (Rust) | InCache (Python) | Redis 7.2.7 |
|----------|-----------------|-----------------|-------------|
| 100      | **70,922**      | 24,213          | 2,789*      |
| 300      | **46,948**      | 12,346          | —           |
| 500      | **35,842**      | 8,382           | —           |
| 600      | **31,546**      | 7,299           | —           |

*Redis on macOS is too slow for LRANGE benchmarks to complete in reasonable time.

### Analysis

**On Linux, InCacheV2 matches Redis on LRANGE and reaches 91–93% on simple commands.**

| Metric | InCacheV2 | Redis 7.2.7 |
|---|---|---|
| Avg simple commands | ~80,700 | ~87,300 |
| LRANGE 300 | 20,408 | 20,450 |
| LRANGE 500 | 14,085 | 14,065 |
| LRANGE 600 | 12,195 | 12,255 |

The remaining ~7% gap on simple commands is the cost of Tokio's async runtime (future polling, waker registration, task scheduling) vs Redis's bare `ae.c` event loop which does raw `epoll_wait` → `read` → process → `write` with zero abstraction. On LRANGE, the per-command overhead is amortised by the larger response payload, so both converge to identical throughput.

**Rust vs Python on Linux:**

| | Rust | Python | Speedup |
|---|---|---|---|
| Simple commands | ~80,700 | ~35,600 | **2.3×** |
| LRANGE 100 | 46,296 | 11,261 | **4.1×** |
| LRANGE 600 | 12,195 | 2,740 | **4.5×** |

**Why does Redis perform poorly on macOS?**

Redis is optimised for Linux `epoll`. On macOS it falls back to `kqueue` and performs 10–30× worse. Both Tokio and Python's `asyncio` handle macOS `kqueue` efficiently. The macOS numbers should not be used to claim InCache "beats" Redis.

### Optimisations applied

| Optimisation | Impact |
|---|---|
| Single-threaded Tokio runtime | Zero locking — `Rc<RefCell<Store>>` like Redis |
| `mimalloc` global allocator | Faster small allocations than system malloc |
| `TCP_NODELAY` | Disables Nagle's algorithm |
| Zero-alloc RESP parser | Commands parsed as `(offset, len)` into read buffer — no `Vec` per command |
| Stack-allocated `Command` | Up to 8 args inline, no heap allocation |
| `FxHashMap` / `FxHashSet` | Fast non-cryptographic hash (same family as Redis's hash) |
| `bytes::Bytes` | Reference-counted zero-copy values |
| `memchr` SIMD | Accelerated `\r\n` scanning |
| `itoa` | Allocation-free integer encoding |
| `encode_into(BytesMut)` | Direct write-buffer encoding |
| 64KB `BufWriter` | Batched TCP writes |
| `match &[u8]` dispatch | Zero-allocation command routing |
| `unsafe from_utf8_unchecked` | Skip validation on trusted RESP input |
| Fat LTO + codegen-units=1 | Whole-program optimisation |

### Run benchmarks yourself

```bash
# InCacheV2 (Rust)
cargo build --release
./target/release/incache_v2 --port 6399 &
redis-benchmark -p 6399 -t set,get,incr,lpush,hset -n 100000 -c 10
redis-benchmark -p 6399 -t lrange -n 10000 -c 10

# InCache (Python)
pip install incache
python -m incache --port 6399 &
redis-benchmark -p 6399 -t set,get,incr,lpush,hset -n 100000 -c 10

# Redis
redis-server --port 6399 --save "" --appendonly no --daemonize yes
redis-benchmark -p 6399 -t set,get,incr,lpush,hset -n 100000 -c 10
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
- Active expiry sweep — background task every 100ms

**Key commands** — `TYPE`, `RENAME`, `KEYS` (glob patterns), `EXISTS`, `DEL`

**Server** — `PING`, `ECHO`, `FLUSHALL`, `FLUSHDB`, `DBSIZE`, `INFO`, `SELECT`, `COMMAND COUNT`, `HELLO`

**Protocol** — full RESP2, pipelining, partial frame reads, inline commands

---

## Architecture

```
src/
├── main.rs              # CLI entrypoint — single-threaded Tokio runtime
├── server.rs            # TCP server — Rc<RefCell<Store>>, spawn_local
│                        #   TCP_NODELAY, 64KB BufWriter
├── protocol.rs          # RESP2 zero-alloc parser + serialiser
│                        #   memchr SIMD, itoa, encode_into()
│                        #   Command struct: stack-allocated arg ranges
├── store.rs             # In-memory store — FxHashMap, zero locking
│                        #   lazy + active TTL expiry
└── commands/
    ├── mod.rs           # Dispatcher — match on &[u8] slices
    ├── strings.rs       # String + key commands
    ├── lists.rs         # List commands (VecDeque)
    ├── hashes.rs        # Hash commands (FxHashMap)
    ├── sets.rs          # Set commands (FxHashSet)
    └── server.rs        # Server commands + HELLO
```

| Concept | InCache (Python) | InCacheV2 (Rust) |
|---------|-----------------|-----------------|
| Async runtime | `asyncio` | `tokio` (current_thread) |
| Concurrency | `asyncio.Lock` | None — single-threaded |
| Allocator | CPython pymalloc | `mimalloc` |
| Hash function | Python built-in | `FxHash` (non-cryptographic) |
| RESP parsing | `bytes` slicing → `list` | Zero-alloc `(offset, len)` ranges |
| Value storage | Python objects | `bytes::Bytes` (zero-copy) |
| TCP writes | `writer.write()` | 64KB `BufWriter` |
| List values | `collections.deque` | `VecDeque` |
| Hash values | `dict` | `FxHashMap<Box<str>, Bytes>` |
| Set values | `set` | `FxHashSet<Box<str>>` |

---

## Tests

149 tests — the same test suite that validates [InCache](https://github.com/ImadRahhali/InCache). Written in Python against `redis-py`, because the tests validate RESP protocol behaviour, not implementation internals.

```bash
pip install redis pytest pytest-asyncio
pytest tests/ -v
```

```
tests/test_strings.py   48 tests
tests/test_lists.py     30 tests
tests/test_hashes.py    27 tests
tests/test_sets.py      31 tests
tests/test_server.py    13 tests
```

---

## Building

```bash
cargo build --release
./target/release/incache_v2                     # default: 0.0.0.0:6399
./target/release/incache_v2 --port 6380         # custom port
```

Dependencies: `tokio` · `bytes` · `itoa` · `memchr` · `mimalloc` · `rustc-hash`

---

## Limitations

InCacheV2 is a learning project, not a production database:

- No persistence — data is lost on restart
- No replication, clustering, Lua scripting, sorted sets, streams, or pub/sub
- No authentication, ACLs, or TLS
- Single-threaded — one CPU core only

For production workloads, use [Redis](https://redis.io) or [Valkey](https://valkey.io).

---

## See Also

- [InCache](https://github.com/ImadRahhali/InCache) — the original Python implementation

---

## License

MIT
