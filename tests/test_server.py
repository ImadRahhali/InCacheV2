import pytest


class TestPing:
    def test_ping(self, r):
        assert r.ping() is True

    def test_ping_with_message(self, rb):
        # redis-py's PING callback always returns bool, so use a raw socket
        import socket
        s = socket.create_connection(("localhost", 6399))
        s.sendall(b"*2\r\n$4\r\nPING\r\n$5\r\nhello\r\n")
        data = s.recv(1024)
        s.close()
        assert data == b"$5\r\nhello\r\n"


class TestEcho:
    def test_echo(self, r):
        assert r.execute_command("ECHO", "hello world") == "hello world"


class TestFlush:
    def test_flushall(self, r):
        r.set("a", "1")
        r.set("b", "2")
        r.flushall()
        assert r.dbsize() == 0

    def test_flushdb(self, r):
        r.set("a", "1")
        r.flushdb()
        assert r.dbsize() == 0


class TestDbsize:
    def test_dbsize_empty(self, r):
        assert r.dbsize() == 0

    def test_dbsize_with_keys(self, r):
        r.set("a", "1")
        r.set("b", "2")
        r.set("c", "3")
        assert r.dbsize() == 3

    def test_dbsize_after_del(self, r):
        r.set("a", "1")
        r.delete("a")
        assert r.dbsize() == 0


class TestSelect:
    def test_select_zero(self, r):
        assert r.execute_command("SELECT", "0") is True

    def test_select_nonzero_raises(self, r):
        with pytest.raises(Exception):
            r.execute_command("SELECT", "1")


class TestInfo:
    def test_info_returns_string(self, r):
        info = r.info()
        assert isinstance(info, dict)

    def test_info_has_server_section(self, r):
        info = r.execute_command("INFO")
        # redis-py parses INFO into a dict; check for our server identifier
        info_str = str(info).lower()
        assert "pyredis" in info_str or "redis_version" in info_str or "incache" in info_str


class TestUnknownCommand:
    def test_unknown_command(self, r):
        with pytest.raises(Exception, match="unknown command"):
            r.execute_command("NOTACOMMAND")
