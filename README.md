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

All benchmarks using `valkey-benchmark`, 100k operations (10k for LRANGE), 10 parallel clients, Redis 7.2.7.

- **Mac**: Apple M4 Pro, macOS
- **Linux**: Intel Xeon Platinum 8175M (8c/16t @ 2.50GHz), Amazon Linux 2

### Linux — Simple commands (ops/sec)

| Command | InCacheV2 (Rust) | InCache (Python) | Redis 7.2.7 | Rust vs Redis |
|---------|-----------------|-----------------|-------------|---------------|
| SET     | **86,730**      | 36,337          | 90,992      | 95%           |
| GET     | **86,356**      | 37,313          | 92,851      | 93%           |
| INCR    | **87,873**      | 37,175          | 91,241      | 96%           |
| LPUSH   | **84,531**      | 35,855          | 89,767      | 94%           |
| HSET    | **86,655**      | 34,305          | 92,081      | 94%           |

### Linux — LRANGE (ops/sec)

| Elements | InCacheV2 (Rust) | InCache (Python) | Redis 7.2.7 | Rust vs Redis |
|----------|-----------------|-----------------|-------------|---------------|
| 100      | **47,847**      | 11,351          | 50,251      | 95%           |
| 300      | **20,964**      | 5,061           | 20,964      | **100%**      |
| 500      | **14,493**      | 3,252           | 14,451      | **100%**      |
| 600      | **12,438**      | 2,761           | 12,376      | **100%**      |

### macOS — Simple commands (ops/sec)

| Command | InCacheV2 (Rust) | InCache (Python) | Redis 7.2.7 |
|---------|-----------------|-----------------|-------------|
| SET     | **81,566**      | 55,463          | 5,336       |
| GET     | **83,472**      | 58,343          | 5,772       |
| INCR    | **85,179**      | 59,524          | 4,164       |
| LPUSH   | **85,251**      | 53,850          | 3,510       |
| HSET    | **85,397**      | 56,180          | 2,285       |

### macOS — LRANGE (ops/sec)

| Elements | InCacheV2 (Rust) | InCache (Python) | Redis 7.2.7 |
|----------|-----------------|-----------------|-------------|
| 100      | **69,444**      | 24,331          | 4,399       |
| 300      | **45,872**      | 12,107          | 2,503       |
| 500      | **35,336**      | 8,467           | 3,678       |
| 600      | **31,056**      | 7,435           | 4,861       |

### Analysis

**On Linux, InCacheV2 matches Redis on LRANGE and reaches 93–96% on simple commands.**

| Metric | InCacheV2 | Redis 7.2.7 |
|---|---|---|
| Avg simple commands | ~86,400 | ~91,400 |
| LRANGE 300 | 20,964 | 20,964 |
| LRANGE 500 | 14,493 | 14,451 |
| LRANGE 600 | 12,438 | 12,376 |

The remaining ~5% gap on simple commands is Redis's hand-tuned `ae` event loop vs Tokio's general-purpose reactor. On LRANGE, Rust's `VecDeque` iterator matches Redis's linked-list traversal exactly.

**Rust vs Python on Linux:**

| | Rust | Python | Speedup |
|---|---|---|---|
| Simple commands | ~86,400 | ~36,200 | **2.4×** |
| LRANGE 100 | 47,847 | 11,351 | **4.2×** |
| LRANGE 600 | 12,438 | 2,761 | **4.5×** |

**Why does Redis perform poorly on macOS?**

Redis is optimised for Linux `epoll`. On macOS it falls back to `kqueue` and performs 10–20× worse. Both Tokio and Python's `asyncio` handle macOS `kqueue` efficiently. The macOS numbers should not be used to claim InCache "beats" Redis.

### Optimisations applied

| Optimisation | Impact |
|---|---|
| Single-threaded Tokio runtime | Zero locking — `Rc<RefCell<Store>>` like Redis's single-threaded model |
| `mimalloc` global allocator | Faster small allocations than system malloc |
| `TCP_NODELAY` | Disables Nagle's algorithm — lower latency |
| `bytes::Bytes` (reference-counted) | Zero-copy value storage |
| `memchr` for CRLF scanning | SIMD-accelerated RESP parsing |
| `itoa` for integer encoding | No `format!()` allocation |
| `encode_into(BytesMut)` | Direct write-buffer encoding |
| 64KB `BufWriter` | Batched TCP writes, fewer syscalls |
| `match &[u8]` command dispatch | Zero-allocation command routing |
| `unsafe from_utf8_unchecked` | Skip UTF-8 validation on trusted RESP input |
| Fat LTO + codegen-units=1 | Whole-program optimisation |

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

# Redis
redis-server --port 6399 --save "" --appendonly no --daemonize yes
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
├── protocol.rs          # RESP2 parser + serialiser
│                        #   memchr SIMD, itoa, encode_into()
├── store.rs             # In-memory store — plain HashMap, zero locking
│                        #   lazy + active TTL expiry
└── commands/
    ├── mod.rs           # Dispatcher — match on &[u8] slices
    ├── strings.rs       # String + key commands
    ├── lists.rs         # List commands (VecDeque)
    ├── hashes.rs        # Hash commands (HashMap)
    ├── sets.rs          # Set commands (HashSet)
    └── server.rs        # Server commands + HELLO
```

| Concept | InCache (Python) | InCacheV2 (Rust) |
|---------|-----------------|-----------------|
| Async runtime | `asyncio` | `tokio` (current_thread) |
| Concurrency | `asyncio.Lock` | None — single-threaded |
| Allocator | CPython pymalloc | `mimalloc` |
| Value storage | Python objects | `bytes::Bytes` (zero-copy) |
| RESP parsing | byte slicing | `memchr` SIMD + `BytesMut` |
| TCP writes | `writer.write()` | 64KB `BufWriter` |
| List values | `collections.deque` | `VecDeque` |
| Hash values | `dict` | `HashMap<Box<str>, Bytes>` |
| Set values | `set` | `HashSet<Box<str>>` |

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

Dependencies: `tokio` · `bytes` · `itoa` · `memchr` · `mimalloc`

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
