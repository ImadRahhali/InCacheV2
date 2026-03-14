import pytest


class TestHsetHget:
    def test_hset_and_hget(self, r):
        assert r.hset("hash", "field", "value") == 1
        assert r.hget("hash", "field") == "value"

    def test_hget_missing_field(self, r):
        r.hset("hash", "field", "value")
        assert r.hget("hash", "missing") is None

    def test_hget_missing_key(self, r):
        assert r.hget("nohash", "field") is None

    def test_hset_multiple(self, r):
        count = r.hset("hash", mapping={"f1": "v1", "f2": "v2", "f3": "v3"})
        assert count == 3
        assert r.hget("hash", "f1") == "v1"
        assert r.hget("hash", "f2") == "v2"

    def test_hset_update_existing(self, r):
        r.hset("hash", "field", "old")
        count = r.hset("hash", "field", "new")
        assert count == 0  # 0 new fields added
        assert r.hget("hash", "field") == "new"

    def test_hset_wrong_type_raises(self, r):
        r.set("key", "string")
        with pytest.raises(Exception):
            r.hset("key", "field", "value")


class TestHmsetHmget:
    def test_hmset(self, r):
        r.hmset("hash", {"f1": "v1", "f2": "v2"})
        assert r.hget("hash", "f1") == "v1"
        assert r.hget("hash", "f2") == "v2"

    def test_hmget(self, r):
        r.hset("hash", mapping={"f1": "v1", "f2": "v2"})
        result = r.hmget("hash", "f1", "f2", "missing")
        assert result == ["v1", "v2", None]


class TestHgetall:
    def test_hgetall(self, r):
        r.hset("hash", mapping={"f1": "v1", "f2": "v2"})
        result = r.hgetall("hash")
        assert result == {"f1": "v1", "f2": "v2"}

    def test_hgetall_missing_key(self, r):
        assert r.hgetall("nohash") == {}


class TestHdel:
    def test_hdel_single(self, r):
        r.hset("hash", mapping={"f1": "v1", "f2": "v2"})
        assert r.hdel("hash", "f1") == 1
        assert r.hget("hash", "f1") is None

    def test_hdel_multiple(self, r):
        r.hset("hash", mapping={"f1": "v1", "f2": "v2", "f3": "v3"})
        assert r.hdel("hash", "f1", "f2") == 2

    def test_hdel_missing_field(self, r):
        r.hset("hash", "f1", "v1")
        assert r.hdel("hash", "missing") == 0

    def test_hdel_all_fields_removes_key(self, r):
        r.hset("hash", "f1", "v1")
        r.hdel("hash", "f1")
        assert r.exists("hash") == 0


class TestHexists:
    def test_hexists_true(self, r):
        r.hset("hash", "field", "value")
        assert r.hexists("hash", "field") == 1

    def test_hexists_false(self, r):
        r.hset("hash", "field", "value")
        assert r.hexists("hash", "other") == 0

    def test_hexists_missing_key(self, r):
        assert r.hexists("nohash", "field") == 0


class TestHlen:
    def test_hlen(self, r):
        r.hset("hash", mapping={"f1": "v1", "f2": "v2", "f3": "v3"})
        assert r.hlen("hash") == 3

    def test_hlen_missing(self, r):
        assert r.hlen("nohash") == 0


class TestHkeysHvals:
    def test_hkeys(self, r):
        r.hset("hash", mapping={"f1": "v1", "f2": "v2"})
        assert set(r.hkeys("hash")) == {"f1", "f2"}

    def test_hvals(self, r):
        r.hset("hash", mapping={"f1": "v1", "f2": "v2"})
        assert set(r.hvals("hash")) == {"v1", "v2"}

    def test_hkeys_missing(self, r):
        assert r.hkeys("nohash") == []

    def test_hvals_missing(self, r):
        assert r.hvals("nohash") == []


class TestHincrby:
    def test_hincrby_new_field(self, r):
        assert r.hincrby("hash", "counter", 1) == 1

    def test_hincrby_existing(self, r):
        r.hset("hash", "counter", "10")
        assert r.hincrby("hash", "counter", 5) == 15

    def test_hincrby_negative(self, r):
        r.hset("hash", "counter", "10")
        assert r.hincrby("hash", "counter", -3) == 7

    def test_hincrby_non_integer_raises(self, r):
        r.hset("hash", "field", "notanumber")
        with pytest.raises(Exception):
            r.hincrby("hash", "field", 1)
