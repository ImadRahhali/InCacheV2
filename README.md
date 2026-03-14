# InCacheV2

A Redis-compatible in-memory database written in Rust. Speaks RESP2 — connect with any Redis client.

Rewrite of [InCache](https://github.com/ImadRahhali/InCache), a Python prototype that topped out at ~36k ops/sec on Linux. InCacheV2 rewrites the same architecture in Rust to see how close we can get to Redis's C implementation — and where we can beat it.

```bash
cargo build --release
./target/release/incache_v2 --port 6399
```

```python
import redis
r = redis.Redis(host="localhost", port=6399, decode_responses=True)
r.set("hello", "world", ex=60)   # "world"
r.incr("counter")                  # 1
r.lpush("queue", "job1")
r.hset("user:1", mapping={"name": "Imad", "role": "engineer"})
```

---

## Benchmarks

Linux · Intel Xeon Platinum 8175M · `redis-benchmark` from Redis 7.2.7 · same machine, same conditions.

### Pipelined (`-P 16`)
Production clients batch 10–50 commands per round-trip. This is the realistic workload.

| Test | InCacheV2 | Redis 7.2.7 | |
|------|-----------|-------------|---|
| SET | **1,176,471** | 892,857 | **+32%** |
| GET | **1,176,471** | 1,000,000 | **+18%** |

Zero-alloc batch parser processes all 16 commands from one `read()` syscall, flushes all responses in one `write()`. 3 syscalls for 16 commands.

### Simple commands (10 clients, 100k ops)

| Command | InCacheV2 | Redis 7.2.7 | Ratio |
|---------|-----------|-------------|-------|
| SET | 83,264 | 85,034 | 98% |
| GET | 84,818 | 87,489 | 97% |
| INCR | 84,388 | 87,566 | 96% |
| LPUSH | 81,103 | 87,566 | 93% |
| HSET | 83,264 | 87,260 | 95% |

The ~3–5% gap is `Bytes::copy_from_slice` vs Redis's `sds` embedded-length strings. Closing it would require unsafe Rust.

### LRANGE (10 clients, 10k ops)

| Elements | InCacheV2 | Redis 7.2.7 | Ratio |
|----------|-----------|-------------|-------|
| 100 | 46,296 | 49,020 | 94% |
| 300 | 20,408 | 20,450 | **100%** |
| 500 | 14,085 | 14,065 | **100%** |
| 600 | 12,195 | 12,255 | **100%** |

At 300+ elements, throughput is identical. Rust's `VecDeque` iterator compiles to the same pointer-chasing loop as Redis's quicklist.

### Large values

| Payload | InCacheV2 SET | Redis SET | Ratio |
|---------|--------------|-----------|-------|
| 3B | 83,264 | 85,034 | 98% |
| 1KB | 78,247 | 85,324 | 92% |
| 4KB | 75,815 | 82,034 | 92% |

The ratio stays consistent — the gap is per-command overhead, not payload size.

```bash
cargo build --release && ./target/release/incache_v2 --port 6399 &
redis-benchmark -p 6399 -t set,get -n 100000 -c 10 -P 16   # pipelined
redis-benchmark -p 6399 -t set,get,incr,lpush,hset -n 100000 -c 10
redis-benchmark -p 6399 -t lrange -n 10000 -c 10
redis-benchmark -p 6399 -t set,get -n 100000 -c 10 -d 1024  # 1KB values
```

---

## Features

**Data structures** — strings, lists, hashes, sets with full command coverage:
- Strings: `SET/GET/MSET/MGET/INCR/INCRBY/DECR/DECRBY/APPEND/STRLEN/GETSET/SETNX/SETEX`
- Lists: `LPUSH/RPUSH/LPOP/RPOP/LRANGE/LLEN/LINDEX/LSET/LINSERT/LREM`
- Hashes: `HSET/HGET/HMSET/HMGET/HGETALL/HDEL/HEXISTS/HLEN/HKEYS/HVALS/HINCRBY`
- Sets: `SADD/SMEMBERS/SREM/SISMEMBER/SCARD/SUNION/SINTER/SDIFF/SMOVE/SPOP`

**TTL** — `EXPIRE/TTL/PERSIST/SETEX/SET EX|PX|NX|XX` · lazy expiry on access · active sweep every 100ms

**Keys** — `TYPE/RENAME/KEYS/EXISTS/DEL`

**Server** — `PING/ECHO/FLUSHALL/FLUSHDB/DBSIZE/INFO/SELECT/COMMAND COUNT/HELLO`

**Transactions** — `MULTI/EXEC/DISCARD`

**Auth** — `--requirepass <password>` · `AUTH` command

**Observability** — `SLOWLOG GET/LEN/RESET`

**Protocol** — RESP2 · pipelining · partial frame reads · inline commands

---

## Architecture

```
src/
├── main.rs       # CLI args, mimalloc global allocator
├── server.rs     # Raw epoll/kqueue loop — no async runtime, TCP_NODELAY
├── protocol.rs   # Zero-alloc RESP2 parser — (offset, len) ranges, memchr SIMD, itoa
├── store.rs      # FxHashMap keyspace — lazy + active TTL expiry, zero locking
└── commands/     # 45 commands — match &[u8] compiled to LLVM jump table
```

InCacheV2 mirrors Redis's architecture deliberately:

| | Redis (C) | InCacheV2 (Rust) |
|---|---|---|
| Event loop | `ae.c` — bare epoll/kqueue | Raw epoll/kqueue — no Tokio |
| Threading | Single-threaded | Single-threaded |
| Locking | None | None |
| Allocator | jemalloc | mimalloc |
| Hash | SipHash-like | FxHash (non-crypto, ~3× faster) |
| Strings | `sds` embedded-length | `bytes::Bytes` ref-counted |
| Lists | Quicklist | `VecDeque` |
| RESP parsing | Hand-tuned pointers | Zero-alloc `(offset, len)` + SIMD |
| Dispatch | Hash table | `match &[u8]` → jump table |

**6 dependencies total:** `bytes · itoa · memchr · mimalloc · rustc-hash · libc`

---

## Tests

175 tests — all written in Python against `redis-py` over a real TCP connection. The same tests pass against real Redis.

**149 functional tests** across `test_strings.py` (48), `test_lists.py` (30), `test_hashes.py` (27), `test_sets.py` (31), `test_server.py` (13).

**14 stress tests** — concurrent INCR from 10 threads (must total exactly 10,000), 1,000 rapid connections, 1MB value round-trip, garbage bytes injection, partial RESP frames, oversized inline commands.

**12 feature tests** — AUTH (required / wrong / correct), MULTI/EXEC/DISCARD (basic, nested INCR, error cases), SLOWLOG (GET/LEN/RESET).

```bash
pip install redis pytest
pytest tests/ -v
```

---

## Limitations

InCacheV2 is a learning project — not production-ready. Key gaps:

- **No persistence** — all data lost on restart (no RDB, no AOF)
- **No eviction** — no `maxmemory`, will OOM under sustained writes
- **No pub/sub** — no `SUBSCRIBE/PUBLISH/PSUBSCRIBE`
- **No sorted sets or streams** — no `ZADD/ZRANGE`, no `XADD/XREAD`
- **No replication or clustering** — single process, single machine
- **No TLS or ACLs** — password auth only
- **Single core** — no multi-threaded I/O offload like Redis 6+
- **No Lua scripting** — no `EVAL`

For production, use [Redis](https://redis.io) or [Valkey](https://valkey.io).

---

## How this was built

This project used the [vinext methodology](https://blog.cloudflare.com/vinext): write the test suite first as the spec, then make it pass.

**[InCache](https://github.com/ImadRahhali/InCache) (Python)** came first — ~800 lines of asyncio, hitting ~36k ops/sec on Linux. The real output was 149 pytest tests validating Redis command behaviour over a real `redis-py` client. Any server that passes them is Redis-compatible.

**InCacheV2 (Rust)** reused the exact same 149 tests. The first version passed them immediately. Then came the performance work: Tokio → raw epoll, `HashMap` → `FxHashMap`, `Vec<Bytes>` per command → zero-alloc stack parser, mimalloc, TCP_NODELAY. Each change benchmarked against Redis 7.2.7 on the same machine.

Result: **+32% over Redis on pipelined workloads, 95–98% on single commands, 100% parity on LRANGE.** AUTH, MULTI/EXEC, and SLOWLOG brought the total to 175 tests.

The entire project — both repos, all benchmarks — was built with AI-assisted development (Kiro CLI + claude-opus-4).

**The takeaway:** Redis is optimised for generality — persistence, replication, Lua, pub/sub, sorted sets. InCacheV2 does one thing: serve GET/SET as fast as possible. When you strip away everything Redis does that we don't, the remaining gap is ~3% — and that's just `sds` vs `Bytes::copy_from_slice`.

---

## See Also

[InCache](https://github.com/ImadRahhali/InCache) — the Python prototype this was built from.

---

## License

MIT
