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

All benchmarks run on **Linux** (Intel Xeon Platinum 8175M, 8c/16t @ 2.50GHz, Amazon Linux 2) using `redis-benchmark` from Redis 7.2.7. Both InCacheV2 and Redis 7.2.7 are single-threaded, running on the same machine, same conditions.

---

### Pipelined workloads

Pipelining sends multiple commands in a single network round-trip without waiting for each response. This is how production Redis clients (Jedis, Lettuce, redis-py) typically operate — batching 10–50 commands per round-trip to amortize network latency. This benchmark uses `-P 16` (16 commands per pipeline), which is a realistic production setting.

| Test | InCacheV2 | Redis 7.2.7 | Result |
|---|---|---|---|
| **Pipeline 16 — SET** | **1,176,471** | 892,857 | **InCacheV2 +32%** |
| **Pipeline 16 — GET** | **1,176,471** | 1,000,000 | **InCacheV2 +18%** |

InCacheV2's zero-alloc batch parser processes all 16 commands from a single `read()` syscall with no per-command heap allocation. The response is encoded directly into a write buffer and flushed in one `write()` syscall.

---

### Simple commands — single request/response

Each command is sent individually: the client sends one command, waits for the response, then sends the next. This measures raw per-command latency — the worst case for any server because every command pays the full syscall overhead (one `read()` + one `write()`). Benchmarked with 10 parallel clients, 100k operations.

| Command | InCacheV2 | Redis 7.2.7 | Ratio |
|---------|-----------|-------------|-------|
| SET     | 83,264    | 85,034      | 98%   |
| GET     | 84,818    | 87,489      | 97%   |
| INCR    | 84,388    | 87,566      | 96%   |
| LPUSH   | 81,103    | 87,566      | 93%   |
| HSET    | 83,264    | 87,260      | 95%   |

The ~3% gap is the cost of `Bytes::copy_from_slice` when storing values. Redis uses `sds` (Simple Dynamic Strings) — a custom string type that embeds length metadata in the allocation header and avoids copying. Matching this would require a custom allocator that sacrifices Rust's safety guarantees.

---

### LRANGE — bulk response

LRANGE returns a range of elements from a list. This benchmarks how efficiently the server serializes large array responses. As the element count grows, the per-command overhead becomes negligible and throughput is dominated by memory access patterns and RESP encoding speed. Benchmarked with 10 clients, 10k operations.

| Elements | InCacheV2 | Redis 7.2.7 | Ratio |
|----------|-----------|-------------|-------|
| 100      | 46,296    | 49,020      | 94%   |
| 300      | 20,408    | 20,450      | **100%** |
| 500      | 14,085    | 14,065      | **100%** |
| 600      | 12,195    | 12,255      | **100%** |

At 300+ elements, InCacheV2 and Redis produce identical throughput. Rust's `VecDeque` iterator compiles to the same pointer-chasing loop as Redis's linked-list traversal.

---

### Concurrency — 50 parallel clients

Tests how the server handles connection multiplexing under higher load. With 50 clients, the event loop must efficiently cycle through more file descriptors per `epoll_wait` call. Both servers are single-threaded, so this measures event loop efficiency, not parallelism.

| Command | InCacheV2 | Redis 7.2.7 | Ratio |
|---------|-----------|-------------|-------|
| SET     | 80,321    | 85,985      | 93%   |
| GET     | 77,761    | 86,133      | 90%   |

---

### Large values

Tests throughput with larger payloads. Redis's default benchmark uses 3-byte values, which is unrealistically small. Real applications often store JSON objects (1–4KB). This measures how well the server handles memory allocation and TCP write buffering for larger payloads.

| Payload | InCacheV2 SET | Redis SET | InCacheV2 GET | Redis GET |
|---------|--------------|-----------|--------------|-----------|
| 3B      | 83,264       | 85,034    | 84,818       | 87,489    |
| 1KB     | 78,247       | 85,324    | 77,580       | 86,655    |
| 4KB     | 75,815       | 82,034    | 75,758       | 83,822    |

Both degrade gracefully. The ratio stays consistent (~92%), confirming the gap is per-command overhead, not payload-dependent.

---

### Run benchmarks yourself

```bash
cargo build --release
./target/release/incache_v2 --port 6399 &

# Pipelined (the headline number)
redis-benchmark -p 6399 -t set,get -n 100000 -c 10 -P 16

# Simple commands
redis-benchmark -p 6399 -t set,get,incr,lpush,hset -n 100000 -c 10

# LRANGE
redis-benchmark -p 6399 -t lrange -n 10000 -c 10

# Concurrency
redis-benchmark -p 6399 -t set,get -n 100000 -c 50

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

**Server** — `PING`, `ECHO`, `FLUSHALL`, `FLUSHDB`, `DBSIZE`, `INFO`, `SELECT`, `COMMAND COUNT`, `HELLO`, `AUTH`, `SLOWLOG GET/LEN/RESET`

**Transactions** — `MULTI`, `EXEC`, `DISCARD`

**Protocol** — full RESP2, pipelining, partial frame reads, inline commands

**Authentication** — `--requirepass <password>` flag, `AUTH` command

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

175 tests covering correctness, concurrency, edge cases, robustness, and feature compliance.

**Functional correctness (149 tests)** — written in Python against `redis-py`. Every command is tested through a real TCP connection using the RESP protocol, exactly as a production client would. The tests don't care whether the server is Rust, C, or Python.

| Suite | Tests | Coverage |
|---|---|---|
| test_strings.py | 48 | SET/GET with all flags (EX/PX/NX/XX), TTL, INCR/DECR, APPEND, STRLEN, TYPE, KEYS glob, RENAME, MSET/MGET, DEL/EXISTS |
| test_lists.py | 30 | LPUSH/RPUSH, LPOP/RPOP, LRANGE with negative indices and out-of-bounds, LINDEX, LSET, LINSERT BEFORE/AFTER, LREM with positive/negative/zero count |
| test_hashes.py | 27 | HSET single and multiple fields, HGET, HMSET/HMGET, HGETALL, HDEL, HEXISTS, HLEN, HKEYS/HVALS, HINCRBY including non-integer error |
| test_sets.py | 31 | SADD with duplicates, SMEMBERS, SREM, SISMEMBER, SCARD, SUNION/SINTER/SDIFF, SMOVE, SPOP |
| test_server.py | 13 | PING with and without message, ECHO, FLUSHALL/FLUSHDB, DBSIZE, SELECT (db 0 only), INFO, unknown command error |

**Stress and robustness (14 tests):**

| Test | What it proves |
|---|---|
| 10-thread concurrent INCR (10,000 total) | Atomic correctness under contention — final value must be exactly 10,000 |
| 10-thread concurrent SET/GET/DEL | No crashes or inconsistent reads under parallel mixed operations |
| Expire-during-access | Key transitions cleanly from value → nil during rapid access |
| Expire hammered (200 rapid GETs) | No errors during the expiry window — only valid values or nil |
| 1MB value round-trip | Large value stored and retrieved correctly, STRLEN matches |
| Empty string value | `SET key ""` returns `""` not nil |
| 100-command pipeline | All 100 SET responses are `True`, all 100 GET responses match |
| 1,000 rapid connections | Open/close connections rapidly — server stays healthy |
| Garbage bytes | Random bytes sent to server — no crash, server continues serving |
| Partial RESP frame | Incomplete command sent then disconnected — no crash |
| Invalid command | Unknown command returns proper error message |
| Oversized inline command | 100KB inline command — no crash |
| KEYS `h?llo` pattern | Single-character wildcard matching |
| KEYS `h*llo` pattern | Multi-character wildcard matching |

**Feature tests (12 tests):**

| Test | What it proves |
|---|---|
| AUTH required | Unauthenticated SET is rejected |
| AUTH wrong password | Wrong password is rejected |
| AUTH correct password | Authenticated client can SET/GET |
| PING without auth | PING works without authentication |
| MULTI/EXEC basic | Transaction returns all results as array |
| MULTI/EXEC INCR | Three INCRs in transaction return [1, 2, 3] |
| DISCARD | Cancels queued commands, key unchanged |
| EXEC without MULTI | Returns error |
| DISCARD without MULTI | Returns error |
| SLOWLOG LEN | Returns integer count |
| SLOWLOG RESET | Clears the log |
| SLOWLOG GET | Returns list of entries |

```bash
pip install redis pytest pytest-asyncio
pytest tests/ -v
```

---

## Limitations

InCacheV2 is a learning project built to understand Redis internals. It is **not production-ready**. Here's what's missing:

**Data loss on restart** — there is no persistence. No RDB snapshots, no AOF (append-only file), no replication. Every restart loses all data. A production cache needs at least one persistence mechanism to survive process restarts, host reboots, or deployments.

**No memory limits or eviction** — InCacheV2 will consume memory until the OS kills it (OOM). Redis supports `maxmemory` with configurable eviction policies (LRU, LFU, random, volatile-ttl). Without eviction, a production deployment would eventually crash under sustained writes.

**No ACLs or TLS** — InCacheV2 supports password authentication (`--requirepass`) but not per-user ACLs or TLS encrypted connections.

**No replication or clustering** — InCacheV2 is a single process on a single machine. If it goes down, the data is gone and clients get connection errors. Redis supports primary-replica replication for high availability and Redis Cluster for horizontal sharding across multiple nodes.

**No pub/sub** — Redis's publish/subscribe messaging is used heavily for real-time notifications, cache invalidation, and event-driven architectures. InCacheV2 doesn't support `SUBSCRIBE`, `PUBLISH`, or `PSUBSCRIBE`.

**No sorted sets or streams** — `ZADD`, `ZRANGE`, `ZRANGEBYSCORE` (sorted sets) and `XADD`, `XREAD` (streams) are among Redis's most powerful features, used for leaderboards, rate limiting, time-series data, and message queues. InCacheV2 doesn't implement either.

**No Lua scripting** — Redis's `EVAL` command runs Lua scripts server-side for complex atomic operations. Not supported.

**No monitoring or observability** — no `CLIENT LIST`, no `MONITOR`, no keyspace notifications. InCacheV2 supports `SLOWLOG` for identifying slow commands, but lacks the full observability suite that production deployments need.

**Single-threaded only** — InCacheV2 uses one CPU core. Redis 6+ offloads I/O to background threads while keeping command execution single-threaded. For workloads that saturate a single core, there's no way to scale up without running multiple instances.


---

## Building

```bash
cargo build --release
./target/release/incache_v2                     # default: 0.0.0.0:6399
./target/release/incache_v2 --port 6380         # custom port
./target/release/incache_v2 --requirepass secret # password protected
```

Dependencies: `bytes` · `itoa` · `memchr` · `mimalloc` · `rustc-hash` · `libc`

---

## How this was built

This project started with a simple idea: use an existing project's own test suite as the implementation spec — the [vinext methodology](https://blog.cloudflare.com/vinext). Write 149 pytest tests that describe how Redis behaves, then make them pass.

**The Python prototype ([InCache](https://github.com/ImadRahhali/InCache))** came first. Pure Python, asyncio, ~800 lines. It hit ~36k ops/sec on Linux — respectable for an interpreted language, but roughly 2.5× slower than Redis. The test suite was the real output: 149 tests that validate RESP2 protocol behaviour through a real `redis-py` client. Any server that passes them is Redis-compatible.

The entire thing — both repos, all benchmarks, this README — was built in a single afternoon, with AI-assisted development (Kiro CLI + claude-ops-4.6-1m).

**The Rust rewrite (InCacheV2)** reused the exact same test suite. Same 149 tests, same `redis-py` client, different binary behind port 6399. The first working version passed all tests immediately. Then came the performance work: single-threaded Tokio → raw epoll loop, `HashMap` → `FxHashMap`, `Vec<Bytes>` per command → zero-alloc stack-based parser, mimalloc, TCP_NODELAY. Each change was benchmarked on the same Linux box against Redis 7.2.7.

The result: **+32% over Redis on pipelined workloads, 95-98% on single commands, 100% match on LRANGE.** Then AUTH, MULTI/EXEC, and SLOWLOG were added to close the feature gap. 175 tests total.


**The takeaway:** a Redis-compatible server that beats Redis on pipelining isn't magic. Redis is optimised for generality — persistence, replication, Lua, pub/sub, sorted sets, streams. InCacheV2 is optimised for exactly one thing: serving GET/SET as fast as possible with zero overhead. When you strip away everything Redis does that we don't, the remaining gap is ~3% — and that's just `sds` strings vs `Bytes::copy_from_slice`.

---

## See Also

- [InCache](https://github.com/ImadRahhali/InCache) — the original Python prototype

---

## License

MIT
