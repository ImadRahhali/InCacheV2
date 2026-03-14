import pytest


class TestLPushRPush:
    def test_lpush_single(self, r):
        assert r.lpush("list", "a") == 1
        assert r.lrange("list", 0, -1) == ["a"]

    def test_lpush_multiple(self, r):
        r.lpush("list", "a", "b", "c")
        assert r.lrange("list", 0, -1) == ["c", "b", "a"]

    def test_rpush_single(self, r):
        assert r.rpush("list", "a") == 1
        assert r.lrange("list", 0, -1) == ["a"]

    def test_rpush_multiple(self, r):
        r.rpush("list", "a", "b", "c")
        assert r.lrange("list", 0, -1) == ["a", "b", "c"]

    def test_lpush_rpush_mixed(self, r):
        r.rpush("list", "b")
        r.lpush("list", "a")
        r.rpush("list", "c")
        assert r.lrange("list", 0, -1) == ["a", "b", "c"]

    def test_push_wrong_type_raises(self, r):
        r.set("key", "string")
        with pytest.raises(Exception):
            r.lpush("key", "val")


class TestLPopRPop:
    def test_lpop(self, r):
        r.rpush("list", "a", "b", "c")
        assert r.lpop("list") == "a"
        assert r.lrange("list", 0, -1) == ["b", "c"]

    def test_rpop(self, r):
        r.rpush("list", "a", "b", "c")
        assert r.rpop("list") == "c"
        assert r.lrange("list", 0, -1) == ["a", "b"]

    def test_lpop_empty(self, r):
        assert r.lpop("nolist") is None

    def test_rpop_empty(self, r):
        assert r.rpop("nolist") is None

    def test_pop_until_empty(self, r):
        r.rpush("list", "a")
        r.lpop("list")
        assert r.exists("list") == 0


class TestLrange:
    def test_lrange_full(self, r):
        r.rpush("list", "a", "b", "c")
        assert r.lrange("list", 0, -1) == ["a", "b", "c"]

    def test_lrange_partial(self, r):
        r.rpush("list", "a", "b", "c", "d")
        assert r.lrange("list", 1, 2) == ["b", "c"]

    def test_lrange_negative_indices(self, r):
        r.rpush("list", "a", "b", "c")
        assert r.lrange("list", -2, -1) == ["b", "c"]

    def test_lrange_out_of_bounds(self, r):
        r.rpush("list", "a", "b")
        assert r.lrange("list", 0, 100) == ["a", "b"]

    def test_lrange_missing_key(self, r):
        assert r.lrange("nolist", 0, -1) == []


class TestLlen:
    def test_llen(self, r):
        r.rpush("list", "a", "b", "c")
        assert r.llen("list") == 3

    def test_llen_empty(self, r):
        assert r.llen("nolist") == 0


class TestLindex:
    def test_lindex(self, r):
        r.rpush("list", "a", "b", "c")
        assert r.lindex("list", 0) == "a"
        assert r.lindex("list", 1) == "b"
        assert r.lindex("list", -1) == "c"

    def test_lindex_out_of_bounds(self, r):
        r.rpush("list", "a")
        assert r.lindex("list", 5) is None


class TestLset:
    def test_lset(self, r):
        r.rpush("list", "a", "b", "c")
        r.lset("list", 1, "B")
        assert r.lrange("list", 0, -1) == ["a", "B", "c"]

    def test_lset_negative(self, r):
        r.rpush("list", "a", "b", "c")
        r.lset("list", -1, "C")
        assert r.lrange("list", 0, -1) == ["a", "b", "C"]

    def test_lset_out_of_range(self, r):
        r.rpush("list", "a")
        with pytest.raises(Exception):
            r.lset("list", 5, "val")


class TestLinsert:
    def test_linsert_before(self, r):
        r.rpush("list", "a", "c")
        r.linsert("list", "BEFORE", "c", "b")
        assert r.lrange("list", 0, -1) == ["a", "b", "c"]

    def test_linsert_after(self, r):
        r.rpush("list", "a", "b")
        r.linsert("list", "AFTER", "a", "aa")
        assert r.lrange("list", 0, -1) == ["a", "aa", "b"]

    def test_linsert_pivot_not_found(self, r):
        r.rpush("list", "a", "b")
        assert r.linsert("list", "BEFORE", "z", "val") == -1


class TestLrem:
    def test_lrem_positive_count(self, r):
        r.rpush("list", "a", "b", "a", "c", "a")
        assert r.lrem("list", 2, "a") == 2
        assert r.lrange("list", 0, -1) == ["b", "c", "a"]

    def test_lrem_negative_count(self, r):
        r.rpush("list", "a", "b", "a", "c", "a")
        assert r.lrem("list", -2, "a") == 2
        assert r.lrange("list", 0, -1) == ["a", "b", "c"]

    def test_lrem_zero_count(self, r):
        r.rpush("list", "a", "b", "a", "c", "a")
        assert r.lrem("list", 0, "a") == 3
        assert r.lrange("list", 0, -1) == ["b", "c"]

    def test_lrem_no_match(self, r):
        r.rpush("list", "a", "b")
        assert r.lrem("list", 1, "z") == 0
