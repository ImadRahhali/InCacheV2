"""Stress tests: concurrency, large values, edge cases, malformed input, rapid connections."""
import threading
import socket
import time
import pytest
import redis


class TestConcurrentIncr:
    def test_concurrent_incr_10_threads(self, r):
        """10 threads × 1000 INCR on same key must equal 10,000."""
        r.set("counter", 0)
        errors = []

        def worker():
            c = redis.Redis(host="localhost", port=6399, decode_responses=True)
            try:
                for _ in range(1000):
                    c.incr("counter")
            except Exception as e:
                errors.append(e)
            finally:
                c.close()

        threads = [threading.Thread(target=worker) for _ in range(10)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()

        assert not errors, f"Errors during concurrent INCR: {errors}"
        assert int(r.get("counter")) == 10000


class TestConcurrentMixedOps:
    def test_concurrent_set_get_del(self, r):
        """Parallel SET/GET/DEL on overlapping keys — no crashes."""
        errors = []

        def writer(tid):
            c = redis.Redis(host="localhost", port=6399, decode_responses=True)
            try:
                for i in range(500):
                    c.set(f"key:{tid}:{i}", f"val:{i}")
                    c.get(f"key:{tid}:{i}")
                    c.delete(f"key:{tid}:{i}")
            except Exception as e:
                errors.append(e)
            finally:
                c.close()

        threads = [threading.Thread(target=writer, args=(t,)) for t in range(10)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()

        assert not errors, f"Errors during concurrent ops: {errors}"


class TestExpireDuringAccess:
    def test_expire_transition(self, r):
        """Key transitions from value → nil after expiry."""
        r.set("expkey", "alive", px=200)
        assert r.get("expkey") == "alive"
        time.sleep(0.3)
        assert r.get("expkey") is None

    def test_expire_hammered(self, r):
        """Rapid GET during expiry window — no errors."""
        r.set("hammer", "val", px=100)
        results = []
        for _ in range(200):
            results.append(r.get("hammer"))
            time.sleep(0.001)
        # Should see some "val" then some None, never an error
        assert all(v in ("val", None) for v in results)
        assert None in results  # must have expired


class TestLargeValues:
    def test_1mb_value(self, r):
        big = "x" * (1024 * 1024)
        r.set("bigkey", big)
        assert r.get("bigkey") == big
        assert r.strlen("bigkey") == 1024 * 1024

    def test_empty_string(self, r):
        r.set("empty", "")
        assert r.get("empty") == ""
        assert r.strlen("empty") == 0


class TestPipelineCorrectness:
    def test_pipeline_100_commands(self, r):
        pipe = r.pipeline(transaction=False)
        for i in range(100):
            pipe.set(f"pkey:{i}", f"pval:{i}")
        results = pipe.execute()
        assert all(r is True for r in results)

        pipe = r.pipeline(transaction=False)
        for i in range(100):
            pipe.get(f"pkey:{i}")
        results = pipe.execute()
        assert results == [f"pval:{i}" for i in range(100)]


class TestRapidConnections:
    def test_1000_connections(self, server_process):
        """Open and close 1000 connections rapidly — server stays healthy."""
        for _ in range(1000):
            c = redis.Redis(host="localhost", port=6399, decode_responses=True)
            c.ping()
            c.close()
        # Server still works after
        c = redis.Redis(host="localhost", port=6399, decode_responses=True)
        assert c.ping() is True
        c.close()


class TestMalformedInput:
    def test_garbage_bytes(self, server_process):
        """Send garbage — server must not crash."""
        s = socket.create_connection(("localhost", 6399))
        s.sendall(b"\xff\xfe\xfd\x00\x01\r\n")
        time.sleep(0.1)
        s.close()
        # Server still works
        c = redis.Redis(host="localhost", port=6399, decode_responses=True)
        assert c.ping() is True
        c.close()

    def test_partial_resp(self, server_process):
        """Send incomplete RESP frame, then close — no crash."""
        s = socket.create_connection(("localhost", 6399))
        s.sendall(b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n")  # missing last arg
        time.sleep(0.1)
        s.close()
        c = redis.Redis(host="localhost", port=6399, decode_responses=True)
        assert c.ping() is True
        c.close()

    def test_invalid_command(self, r):
        with pytest.raises(Exception):
            r.execute_command("TOTALLYNOTACOMMAND", "arg1", "arg2")

    def test_oversized_inline(self, server_process):
        """Send a very long inline command — no crash."""
        s = socket.create_connection(("localhost", 6399))
        s.sendall(b"PING " + b"A" * 100000 + b"\r\n")
        time.sleep(0.1)
        data = s.recv(4096)
        s.close()
        # Server still alive
        c = redis.Redis(host="localhost", port=6399, decode_responses=True)
        assert c.ping() is True
        c.close()


class TestKeyPatterns:
    def test_keys_question_mark(self, r):
        r.set("hello", "1")
        r.set("hallo", "2")
        r.set("hxllo", "3")
        keys = r.keys("h?llo")
        assert set(keys) == {"hello", "hallo", "hxllo"}

    def test_keys_star_middle(self, r):
        r.set("hello", "1")
        r.set("heeello", "2")
        r.set("hllo", "3")
        keys = r.keys("h*llo")
        assert set(keys) == {"hello", "heeello", "hllo"}
