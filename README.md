# InCacheV2

A Redis-compatible in-memory database written in Rust. Speaks the RESP2 protocol — connect with any Redis client, no configuration needed.

Started as a rewrite of [InCache](https://github.com/ImadRahhali/InCache), a Python prototype that topped out at ~36k ops/sec on Linux — well short of Redis. InCacheV2 rewrites the same architecture in Rust to see how close we can get to Redis's C implementation, and where we can beat it.

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

All benchmarks using `redis-benchmark` (from Redis 7.2.7), on **Linux** (Intel Xeon Platinum 8175M, 8c/16t @ 2.50GHz, Amazon Linux 2). macOS numbers included separately — Redis performs poorly on macOS due to `kqueue` fallback.

### Pipelined workloads — InCacheV2 beats Redis

| Test | InCacheV2 | Redis 7.2.7 | Result |
|---|---|---|---|
| **Pipeline 16 — SET** | **1,176,471** | 892,857 | **InCacheV2 +32%** |
| **Pipeline 16 — GET** | **1,176,471** | 1,000,000 | **InCacheV2 +18%** |

This is the most realistic benchmark. Production Redis clients (Jedis, Lettuce, redis-py) pipeline commands by default. InCacheV2's zero-alloc batch parser + buffered writes amortize per-command overhead across 16 commands per round-trip.

### Simple commands — 10 clients, 100k ops (ops/sec)

| Command | InCacheV2 | Redis 7.2.7 | Ratio |
|---------|-----------|-------------|-------|
| SET     | 83,264    | 85,034      | 98%   |
| GET     | 84,818    | 87,489      | 97%   |
| INCR    | 84,388    | 87,566      | 96%   |
| LPUSH   | 81,103    | 87,566      | 93%   |
| HSET    | 83,264    | 87,260      | 95%   |

The ~3% gap on single commands is the cost of `Bytes::copy_from_slice` when storing values vs Redis's zero-copy `sds` strings.

### LRANGE — identical throughput

| Elements | InCacheV2 | Redis 7.2.7 | Ratio |
|----------|-----------|-------------|-------|
| 100      | 46,296    | 49,020      | 94%   |
| 300      | 20,408    | 20,450      | **100%** |
| 500      | 14,085    | 14,065      | **100%** |
| 600      | 12,195    | 12,255      | **100%** |

Once the response payload dominates (300+ elements), per-command overhead becomes irrelevant and both converge.

### Concurrency — 50 clients (ops/sec)

| Command | InCacheV2 | Redis 7.2.7 | Ratio |
|---------|-----------|-------------|-------|
| SET     | 80,321    | 85,985      | 93%   |
| GET     | 77,761    | 86,133      | 90%   |

### Large values (ops/sec)

| Payload | InCacheV2 SET | Redis SET | InCacheV2 GET | Redis GET |
|---------|--------------|-----------|--------------|-----------|
| 3B (default) | 80,192 | 88,106 | 80,257 | 86,580 |
| 1KB     | 78,247       | 85,324    | 77,580       | 86,655    |
| 4KB     | 75,815       | 82,034    | 75,758       | 83,822    |

Both degrade gracefully with larger values. The ratio stays consistent (~92%).

### macOS (Apple M4 Pro) — for reference only

| Command | InCacheV2 | Redis 7.2.7 |
|---------|-----------|-------------|
| SET     | 78,125    | 7,689       |
| GET     | 83,612    | 2,664       |
| INCR    | 84,818    | 3,297       |
| LPUSH   | 83,752    | 7,337       |
| HSET    | 84,459    | 2,682       |

Redis is optimised for Linux `epoll`. On macOS it falls back to `kqueue` and performs 10–30× worse. These numbers should not be used to claim InCacheV2 "beats" Redis — the Linux numbers are the honest comparison.

### Run benchmarks yourself

```bash
# InCacheV2
cargo build --release
./target/release/incache_v2 --port 6399 &

# Simple
redis-benchmark -p 6399 -t set,get,incr,lpush,hset -n 100000 -c 10

# Pipelined (the headline number)
redis-benchmark -p 6399 -t set,get -n 100000 -c 10 -P 16

# LRANGE
redis-benchmark -p 6399 -t lrange -n 10000 -c 10

# Large values
redis-benchmark -p 6399 -t set,get -n 100000 -c 10 -d 1024
redis-benchmark -p 6399 -t set,get -n 100000 -c 10 -d 4096
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
├── main.rs              # CLI entrypoint — mimalloc global allocator
├── server.rs            # Raw epoll/kqueue event loop — no async runtime
│                        #   TCP_NODELAY, direct read→parse→execute→write
├── protocol.rs          # RESP2 zero-alloc parser + serialiser
│                        #   Commands parsed as (offset, len) into read buffer
│                        #   memchr SIMD for \r\n scanning, itoa for integers
├── store.rs             # In-memory store — FxHashMap, zero locking
│                        #   lazy + active TTL expiry
└── commands/
    ├── mod.rs           # Dispatcher — match on &[u8] slices, zero allocation
    ├── strings.rs       # String + key commands
    ├── lists.rs         # List commands (VecDeque)
    ├── hashes.rs        # Hash commands (FxHashMap)
    ├── sets.rs          # Set commands (FxHashSet)
    └── server.rs        # Server commands + HELLO
```

**Design decisions that match Redis:**

| | Redis (C) | InCacheV2 (Rust) |
|---|---|---|
| Threading | Single-threaded `ae.c` event loop | Single-threaded raw `epoll`/`kqueue` loop |
| Locking | None | None |
| Async runtime | None — bare event loop | None — bare event loop |
| Allocator | jemalloc | mimalloc |
| Hash function | SipHash-like | FxHash (non-cryptographic) |
| String storage | `sds` (embedded length) | `bytes::Bytes` (ref-counted) |
| List storage | Quicklist (ziplist + linked list) | `VecDeque` |
| RESP parsing | Hand-tuned pointer arithmetic | Zero-alloc `(offset, len)` ranges + `memchr` SIMD |
| Command dispatch | Hash table lookup | `match &[u8]` (compiled to jump table) |

---

## Tests

149 tests written in Python against `redis-py`. The tests validate RESP protocol behaviour over TCP — they don't care whether the server is written in Rust, C, or Python.

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

Dependencies: `bytes` · `itoa` · `memchr` · `mimalloc` · `rustc-hash` · `libc`

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

- [InCache](https://github.com/ImadRahhali/InCache) — the original Python prototype

---

## License

MIT
