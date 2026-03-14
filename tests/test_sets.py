import pytest


class TestSadd:
    def test_sadd_single(self, r):
        assert r.sadd("set", "a") == 1

    def test_sadd_multiple(self, r):
        assert r.sadd("set", "a", "b", "c") == 3

    def test_sadd_duplicate(self, r):
        r.sadd("set", "a")
        assert r.sadd("set", "a") == 0

    def test_sadd_partial_duplicates(self, r):
        r.sadd("set", "a", "b")
        assert r.sadd("set", "b", "c") == 1

    def test_sadd_wrong_type_raises(self, r):
        r.set("key", "string")
        with pytest.raises(Exception):
            r.sadd("key", "member")


class TestSmembers:
    def test_smembers(self, r):
        r.sadd("set", "a", "b", "c")
        assert r.smembers("set") == {"a", "b", "c"}

    def test_smembers_missing(self, r):
        assert r.smembers("noset") == set()


class TestSrem:
    def test_srem_single(self, r):
        r.sadd("set", "a", "b")
        assert r.srem("set", "a") == 1
        assert r.smembers("set") == {"b"}

    def test_srem_multiple(self, r):
        r.sadd("set", "a", "b", "c")
        assert r.srem("set", "a", "b") == 2

    def test_srem_missing_member(self, r):
        r.sadd("set", "a")
        assert r.srem("set", "z") == 0

    def test_srem_all_removes_key(self, r):
        r.sadd("set", "a")
        r.srem("set", "a")
        assert r.exists("set") == 0


class TestSismember:
    def test_sismember_true(self, r):
        r.sadd("set", "a")
        assert r.sismember("set", "a") == 1

    def test_sismember_false(self, r):
        r.sadd("set", "a")
        assert r.sismember("set", "z") == 0

    def test_sismember_missing_key(self, r):
        assert r.sismember("noset", "a") == 0


class TestScard:
    def test_scard(self, r):
        r.sadd("set", "a", "b", "c")
        assert r.scard("set") == 3

    def test_scard_missing(self, r):
        assert r.scard("noset") == 0


class TestSunion:
    def test_sunion(self, r):
        r.sadd("s1", "a", "b")
        r.sadd("s2", "b", "c")
        assert r.sunion("s1", "s2") == {"a", "b", "c"}

    def test_sunion_single(self, r):
        r.sadd("s1", "a", "b")
        assert r.sunion("s1") == {"a", "b"}

    def test_sunion_with_missing(self, r):
        r.sadd("s1", "a")
        assert r.sunion("s1", "noset") == {"a"}


class TestSinter:
    def test_sinter(self, r):
        r.sadd("s1", "a", "b", "c")
        r.sadd("s2", "b", "c", "d")
        assert r.sinter("s1", "s2") == {"b", "c"}

    def test_sinter_no_overlap(self, r):
        r.sadd("s1", "a")
        r.sadd("s2", "b")
        assert r.sinter("s1", "s2") == set()

    def test_sinter_with_missing(self, r):
        r.sadd("s1", "a", "b")
        assert r.sinter("s1", "noset") == set()


class TestSdiff:
    def test_sdiff(self, r):
        r.sadd("s1", "a", "b", "c")
        r.sadd("s2", "b", "c", "d")
        assert r.sdiff("s1", "s2") == {"a"}

    def test_sdiff_no_diff(self, r):
        r.sadd("s1", "a", "b")
        r.sadd("s2", "a", "b", "c")
        assert r.sdiff("s1", "s2") == set()

    def test_sdiff_with_missing(self, r):
        r.sadd("s1", "a", "b")
        assert r.sdiff("s1", "noset") == {"a", "b"}


class TestSmove:
    def test_smove(self, r):
        r.sadd("src", "a", "b")
        r.sadd("dst", "c")
        assert r.smove("src", "dst", "a") == 1
        assert r.smembers("src") == {"b"}
        assert r.smembers("dst") == {"a", "c"}

    def test_smove_member_not_in_src(self, r):
        r.sadd("src", "a")
        assert r.smove("src", "dst", "z") == 0

    def test_smove_to_new_dst(self, r):
        r.sadd("src", "a")
        r.smove("src", "newdst", "a")
        assert r.smembers("newdst") == {"a"}


class TestSpop:
    def test_spop(self, r):
        r.sadd("set", "a")
        result = r.spop("set")
        assert result == "a"
        assert r.scard("set") == 0

    def test_spop_missing(self, r):
        assert r.spop("noset") is None

    def test_spop_reduces_size(self, r):
        r.sadd("set", "a", "b", "c")
        result = r.spop("set")
        assert result in {"a", "b", "c"}
        assert r.scard("set") == 2
