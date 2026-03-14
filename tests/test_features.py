"""Tests for AUTH, MULTI/EXEC, and SLOWLOG."""
import subprocess
import sys
import time
import socket
import pytest
import redis
import os


def wait_for_server(port, timeout=5.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            with socket.create_connection(("localhost", port), timeout=0.1):
                return True
        except (ConnectionRefusedError, OSError):
            time.sleep(0.05)
    raise RuntimeError(f"Server did not start on port {port}")


# ── AUTH tests use a separate server on port 6398 with --requirepass ──

@pytest.fixture(scope="module")
def auth_server():
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    binary = os.path.join(root, "target", "release", "incache_v2")
    if not os.path.exists(binary):
        binary = os.path.join(root, "target", "debug", "incache_v2")
    proc = subprocess.Popen(
        [binary, "--port", "6398", "--requirepass", "secret123"],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    wait_for_server(6398)
    yield proc
    proc.terminate()
    proc.wait()


class TestAuth:
    def test_auth_required(self, auth_server):
        c = redis.Redis(host="localhost", port=6398, decode_responses=True)
        with pytest.raises(redis.exceptions.AuthenticationError):
            c.set("key", "val")
        c.close()

    def test_auth_wrong_password(self, auth_server):
        c = redis.Redis(host="localhost", port=6398, decode_responses=True, password="wrong")
        with pytest.raises(redis.exceptions.AuthenticationError):
            c.set("key", "val")
        c.close()

    def test_auth_correct_password(self, auth_server):
        c = redis.Redis(host="localhost", port=6398, decode_responses=True, password="secret123")
        assert c.set("key", "val") is True
        assert c.get("key") == "val"
        c.close()

    def test_ping_without_auth(self, auth_server):
        """PING should work without auth."""
        c = redis.Redis(host="localhost", port=6398, decode_responses=True)
        assert c.ping() is True
        c.close()


# ── MULTI/EXEC tests use the normal server on port 6399 ──

class TestMultiExec:
    def test_multi_exec_basic(self, r):
        pipe = r.pipeline(transaction=True)
        pipe.set("tx1", "a")
        pipe.set("tx2", "b")
        pipe.get("tx1")
        results = pipe.execute()
        assert results == [True, True, "a"]

    def test_multi_exec_incr(self, r):
        r.set("counter", "0")
        pipe = r.pipeline(transaction=True)
        pipe.incr("counter")
        pipe.incr("counter")
        pipe.incr("counter")
        results = pipe.execute()
        assert results == [1, 2, 3]

    def test_discard(self, r):
        """DISCARD cancels the transaction."""
        r.set("key", "original")
        # Use raw commands since redis-py doesn't expose DISCARD easily
        r.execute_command("MULTI")
        r.execute_command("SET", "key", "changed")
        r.execute_command("DISCARD")
        assert r.get("key") == "original"

    def test_exec_without_multi(self, r):
        with pytest.raises(Exception):
            r.execute_command("EXEC")

    def test_discard_without_multi(self, r):
        with pytest.raises(Exception):
            r.execute_command("DISCARD")


class TestSlowlog:
    def test_slowlog_len(self, r):
        result = r.execute_command("SLOWLOG", "LEN")
        assert isinstance(result, int)

    def test_slowlog_reset(self, r):
        r.execute_command("SLOWLOG", "RESET")
        assert r.execute_command("SLOWLOG", "LEN") == 0

    def test_slowlog_get(self, r):
        r.execute_command("SLOWLOG", "RESET")
        result = r.execute_command("SLOWLOG", "GET")
        assert isinstance(result, list)
