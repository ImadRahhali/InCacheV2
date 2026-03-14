import subprocess
import sys
import time
import socket
import os
import pytest
import redis


def wait_for_server(host="localhost", port=6399, timeout=5.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            with socket.create_connection((host, port), timeout=0.1):
                return True
        except (ConnectionRefusedError, OSError):
            time.sleep(0.05)
    raise RuntimeError(f"Server did not start on {host}:{port} within {timeout}s")


@pytest.fixture(scope="session")
def server_process():
    # Find the Rust binary
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    binary = os.path.join(root, "target", "release", "incache_v2")
    if not os.path.exists(binary):
        binary = os.path.join(root, "target", "debug", "incache_v2")
    proc = subprocess.Popen(
        [binary, "--port", "6399"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    wait_for_server()
    yield proc
    proc.terminate()
    proc.wait()


@pytest.fixture
def r(server_process):
    client = redis.Redis(host="localhost", port=6399, decode_responses=True)
    client.flushall()
    yield client
    client.close()


@pytest.fixture
def rb(server_process):
    """Binary client — returns bytes instead of str."""
    client = redis.Redis(host="localhost", port=6399, decode_responses=False)
    client.flushall()
    yield client
    client.close()
