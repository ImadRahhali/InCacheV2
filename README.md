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

All benchmarks using `valkey-benchmark`, 100k operations (10k for LRANGE), 10 parallel clients. Redis 7.2.7 on both platforms.

- **Mac**: Apple M4 Pro, macOS
- **Linux**: Intel Xeon Platinum 8175M (8 cores / 16 threads), Amazon Linux 2

### Linux — Simple commands (ops/sec)

| Command | InCacheV2 (Rust) | InCache (Python) | Redis 7.2.7 |
|---------|-----------------|-----------------|-------------|
| SET     | **85,763**      | 36,337          | 90,992      |
| GET     | **83,472**      | 37,313          | 90,992      |
| INCR    | **84,818**      | 37,175          | 92,507      |
| LPUSH   | **82,850**      | 35,855          | 92,678      |
| HSET    | **85,690**      | 34,305          | 90,090      |

### Linux — LRANGE (ops/sec)

| Elements | InCacheV2 (Rust) | InCache (Python) | Redis 7.2.7 |
|----------|-----------------|-----------------|-------------|
| 100      | **46,948**      | 11,351          | 51,020      |
| 300      | **21,008**      | 5,061           | 21,186      |
| 500      | **13,928**      | 3,252           | 14,556      |
| 600      | **12,392**      | 2,761           | 12,438      |

### macOS — Simple commands (ops/sec)

| Command | InCacheV2 (Rust) | InCache (Python) | Redis 7.2.7 |
|---------|-----------------|-----------------|-------------|
| SET     | **74,460**      | 55,463          | 5,336       |
| GET     | **78,802**      | 58,343          | 5,772       |
| INCR    | **78,555**      | 59,524          | 4,164       |
| LPUSH   | **72,622**      | 53,850          | 3,510       |
| HSET    | **71,891**      | 56,180          | 2,285       |

### macOS — LRANGE (ops/sec)

| Elements | InCacheV2 (Rust) | InCache (Python) | Redis 7.2.7 |
|----------|-----------------|-----------------|-------------|
| 100      | **57,803**      | 24,331          | 4,399       |
| 300      | **40,000**      | 12,107          | 2,503       |
| 500      | **33,333**      | 8,467           | 3,678       |
| 600      | **28,736**      | 7,435           | 4,861       |

### Analysis

**Linux is the only honest comparison.** Redis is optimised for Linux `epoll` — on macOS it falls back to `kqueue` and performs 10–20× worse. The macOS numbers are included for completeness but should not be used to claim InCache "beats" Redis.

**On Linux, InCacheV2 reaches ~93% of Redis throughput:**

| | InCacheV2 (Rust) | Redis 7.2.7 | Ratio |
|---|---|---|---|
| Avg simple commands | ~84,500 | ~91,400 | 92% |
| LRANGE 100 | 46,948 | 51,020 | 92% |
| LRANGE 600 | 12,392 | 12,438 | **99.6%** |

**Rust vs Python — Linux:**

| | Rust | Python | Speedup |
|---|---|---|---|
| Simple commands | ~84,500 | ~36,200 | **2.3×** |
| LRANGE 100 | 46,948 | 11,351 | **4.1×** |
| LRANGE 600 | 12,392 | 2,761 | **4.5×** |

The Rust advantage is much larger on Linux than macOS because Python's `asyncio` happens to handle macOS `kqueue` efficiently, masking the interpreter overhead. On Linux with `epoll`, the raw per-command cost dominates — and Rust's zero-overhead abstractions shine.

### Optimisations in V2

| Optimisation | Impact |
|---|---|
| `DashMap` (lock-free concurrent hashmap) | Per-shard locking instead of single `Mutex` |
| `bytes::Bytes` (reference-counted) | Zero-copy value storage |
| `memchr` for CRLF scanning | SIMD-accelerated RESP parsing |
| `itoa` for integer encoding | No `format!()` allocation |
| `encode_into(BytesMut)` | Direct write-buffer encoding |
| `BufWriter<OwnedWriteHalf>` | Batched TCP writes |
| Stack-based command dispatch | `match` on `&[u8]` — zero allocation |
| `Box<str>` for hash/set keys | 1 word smaller than `String` |
| LTO + codegen-units=1 | Whole-program optimisation |

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
