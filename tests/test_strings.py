import time
import pytest


class TestSetGet:
    def test_set_and_get(self, r):
        assert r.set("key", "value") is True
        assert r.get("key") == "value"

    def test_get_missing_key(self, r):
        assert r.get("nokey") is None

    def test_set_overwrites(self, r):
        r.set("key", "first")
        r.set("key", "second")
        assert r.get("key") == "second"

    def test_set_with_ex(self, r):
        r.set("key", "value", ex=1)
        assert r.get("key") == "value"
        time.sleep(1.1)
        assert r.get("key") is None

    def test_set_with_px(self, r):
        r.set("key", "value", px=100)
        assert r.get("key") == "value"
        time.sleep(0.2)
        assert r.get("key") is None

    def test_set_nx_not_exists(self, r):
        result = r.set("key", "value", nx=True)
        assert result is True
        assert r.get("key") == "value"

    def test_set_nx_already_exists(self, r):
        r.set("key", "original")
        result = r.set("key", "new", nx=True)
        assert result is None
        assert r.get("key") == "original"

    def test_set_xx_exists(self, r):
        r.set("key", "original")
        result = r.set("key", "updated", xx=True)
        assert result is True
        assert r.get("key") == "updated"

    def test_set_xx_not_exists(self, r):
        result = r.set("key", "value", xx=True)
        assert result is None
        assert r.get("key") is None


class TestMSetMGet:
    def test_mset_and_mget(self, r):
        r.mset({"a": "1", "b": "2", "c": "3"})
        result = r.mget("a", "b", "c")
        assert result == ["1", "2", "3"]

    def test_mget_with_missing(self, r):
        r.set("a", "1")
        result = r.mget("a", "missing", "also_missing")
        assert result == ["1", None, None]


class TestDelExists:
    def test_del_single(self, r):
        r.set("key", "val")
        assert r.delete("key") == 1
        assert r.get("key") is None

    def test_del_multiple(self, r):
        r.set("a", "1")
        r.set("b", "2")
        assert r.delete("a", "b", "c") == 2

    def test_del_missing(self, r):
        assert r.delete("nokey") == 0

    def test_exists_true(self, r):
        r.set("key", "val")
        assert r.exists("key") == 1

    def test_exists_false(self, r):
        assert r.exists("nokey") == 0

    def test_exists_multiple(self, r):
        r.set("a", "1")
        r.set("b", "2")
        assert r.exists("a", "b", "c") == 2


class TestIncrDecr:
    def test_incr_new_key(self, r):
        assert r.incr("counter") == 1

    def test_incr_existing(self, r):
        r.set("counter", "10")
        assert r.incr("counter") == 11

    def test_incrby(self, r):
        r.set("counter", "10")
        assert r.incrby("counter", 5) == 15

    def test_decr(self, r):
        r.set("counter", "10")
        assert r.decr("counter") == 9

    def test_decrby(self, r):
        r.set("counter", "10")
        assert r.decrby("counter", 3) == 7

    def test_incr_non_integer_raises(self, r):
        r.set("key", "notanumber")
        with pytest.raises(Exception):
            r.incr("key")

    def test_incr_negative(self, r):
        r.set("counter", "5")
        assert r.incrby("counter", -3) == 2


class TestAppendStrlen:
    def test_append_new_key(self, r):
        assert r.append("key", "hello") == 5
        assert r.get("key") == "hello"

    def test_append_existing(self, r):
        r.set("key", "hello")
        assert r.append("key", " world") == 11
        assert r.get("key") == "hello world"

    def test_strlen(self, r):
        r.set("key", "hello")
        assert r.strlen("key") == 5

    def test_strlen_missing(self, r):
        assert r.strlen("nokey") == 0


class TestGetset:
    def test_getset_existing(self, r):
        r.set("key", "old")
        assert r.getset("key", "new") == "old"
        assert r.get("key") == "new"

    def test_getset_new_key(self, r):
        assert r.getset("key", "value") is None
        assert r.get("key") == "value"


class TestSetnx:
    def test_setnx_new(self, r):
        assert r.setnx("key", "value") is True
        assert r.get("key") == "value"

    def test_setnx_existing(self, r):
        r.set("key", "original")
        assert r.setnx("key", "new") is False
        assert r.get("key") == "original"


class TestSetex:
    def test_setex(self, r):
        r.setex("key", 1, "value")
        assert r.get("key") == "value"
        time.sleep(1.1)
        assert r.get("key") is None


class TestExpireTtl:
    def test_expire_and_ttl(self, r):
        r.set("key", "value")
        r.expire("key", 10)
        ttl = r.ttl("key")
        assert 8 <= ttl <= 10

    def test_ttl_no_expiry(self, r):
        r.set("key", "value")
        assert r.ttl("key") == -1

    def test_ttl_missing_key(self, r):
        assert r.ttl("nokey") == -2

    def test_persist(self, r):
        r.set("key", "value", ex=10)
        assert r.persist("key") == 1
        assert r.ttl("key") == -1

    def test_expire_missing_key(self, r):
        assert r.expire("nokey", 10) == 0


class TestType:
    def test_type_string(self, r):
        r.set("key", "value")
        assert r.type("key") == "string"

    def test_type_list(self, r):
        r.lpush("key", "val")
        assert r.type("key") == "list"

    def test_type_hash(self, r):
        r.hset("key", "f", "v")
        assert r.type("key") == "hash"

    def test_type_set(self, r):
        r.sadd("key", "member")
        assert r.type("key") == "set"

    def test_type_missing(self, r):
        assert r.type("nokey") == "none"


class TestKeys:
    def test_keys_all(self, r):
        r.set("foo", "1")
        r.set("bar", "2")
        keys = r.keys("*")
        assert set(keys) == {"foo", "bar"}

    def test_keys_pattern(self, r):
        r.set("foo1", "1")
        r.set("foo2", "2")
        r.set("bar", "3")
        keys = r.keys("foo*")
        assert set(keys) == {"foo1", "foo2"}

    def test_keys_empty(self, r):
        assert r.keys("*") == []


class TestRename:
    def test_rename(self, r):
        r.set("old", "value")
        r.rename("old", "new")
        assert r.get("new") == "value"
        assert r.get("old") is None

    def test_rename_missing_raises(self, r):
        with pytest.raises(Exception):
            r.rename("nokey", "other")
