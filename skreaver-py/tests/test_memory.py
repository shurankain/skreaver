"""Tests for Memory backends."""

import json
import os
import tempfile

import pytest


class TestFileMemory:
    """Tests for FileMemory backend."""

    def test_file_memory_creation(self):
        """Test FileMemory creation with path."""
        from skreaver.memory import FileMemory

        with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as f:
            path = f.name

        try:
            memory = FileMemory(path)
            assert memory.path == path
        finally:
            if os.path.exists(path):
                os.unlink(path)

    def test_file_memory_default(self):
        """Test FileMemory with default path."""
        from skreaver.memory import FileMemory

        memory = FileMemory.default()
        assert memory.path is not None
        assert "skreaver_temp_memory.json" in memory.path

    def test_file_memory_store_load(self):
        """Test storing and loading values."""
        from skreaver.memory import FileMemory

        with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as f:
            path = f.name

        try:
            memory = FileMemory(path)

            # Store a value
            memory.store("test_key", "test_value")

            # Load it back
            value = memory.load("test_key")
            assert value == "test_value"

            # Load non-existent key
            value = memory.load("nonexistent")
            assert value is None
        finally:
            if os.path.exists(path):
                os.unlink(path)

    def test_file_memory_store_many_load_many(self):
        """Test batch store and load operations."""
        from skreaver.memory import FileMemory

        with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as f:
            path = f.name

        try:
            memory = FileMemory(path)

            # Store multiple values
            memory.store_many({"key1": "value1", "key2": "value2", "key3": "value3"})

            # Load multiple values
            values = memory.load_many(["key1", "key2", "key3", "nonexistent"])
            assert values == ["value1", "value2", "value3", None]
        finally:
            if os.path.exists(path):
                os.unlink(path)

    def test_file_memory_snapshot_restore(self):
        """Test snapshot and restore functionality."""
        from skreaver.memory import FileMemory

        with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as f:
            path = f.name

        try:
            memory = FileMemory(path)

            # Store some values
            memory.store("key1", "value1")
            memory.store("key2", "value2")

            # Take snapshot
            snapshot = memory.snapshot()
            assert snapshot is not None

            # Parse snapshot to verify format
            data = json.loads(snapshot)
            assert "key1" in data
            assert data["key1"] == "value1"

            # Modify memory
            memory.store("key1", "modified")
            assert memory.load("key1") == "modified"

            # Restore from snapshot
            memory.restore(snapshot)
            assert memory.load("key1") == "value1"
        finally:
            if os.path.exists(path):
                os.unlink(path)

    def test_file_memory_keys(self):
        """Test getting all keys."""
        from skreaver.memory import FileMemory

        with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as f:
            path = f.name

        try:
            memory = FileMemory(path)

            # Initially empty
            assert memory.keys() == []

            # Add some keys
            memory.store("alpha", "1")
            memory.store("beta", "2")
            memory.store("gamma", "3")

            keys = memory.keys()
            assert len(keys) == 3
            assert set(keys) == {"alpha", "beta", "gamma"}
        finally:
            if os.path.exists(path):
                os.unlink(path)

    def test_file_memory_len(self):
        """Test __len__ method."""
        from skreaver.memory import FileMemory

        with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as f:
            path = f.name

        try:
            memory = FileMemory(path)

            assert len(memory) == 0

            memory.store("key1", "value1")
            assert len(memory) == 1

            memory.store("key2", "value2")
            assert len(memory) == 2
        finally:
            if os.path.exists(path):
                os.unlink(path)

    def test_file_memory_is_empty(self):
        """Test is_empty method."""
        from skreaver.memory import FileMemory

        with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as f:
            path = f.name

        try:
            memory = FileMemory(path)

            assert memory.is_empty() is True

            memory.store("key", "value")
            assert memory.is_empty() is False
        finally:
            if os.path.exists(path):
                os.unlink(path)

    def test_file_memory_contains(self):
        """Test __contains__ method."""
        from skreaver.memory import FileMemory

        with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as f:
            path = f.name

        try:
            memory = FileMemory(path)

            assert ("test_key" in memory) is False

            memory.store("test_key", "test_value")
            assert ("test_key" in memory) is True
        finally:
            if os.path.exists(path):
                os.unlink(path)

    def test_file_memory_getitem_setitem(self):
        """Test dict-like access."""
        from skreaver.memory import FileMemory

        with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as f:
            path = f.name

        try:
            memory = FileMemory(path)

            # Set item
            memory["mykey"] = "myvalue"

            # Get item
            assert memory["mykey"] == "myvalue"

            # KeyError for missing key
            with pytest.raises(KeyError):
                _ = memory["nonexistent"]
        finally:
            if os.path.exists(path):
                os.unlink(path)

    def test_file_memory_repr(self):
        """Test string representation."""
        from skreaver.memory import FileMemory

        memory = FileMemory("/tmp/test.json")
        repr_str = repr(memory)

        assert "FileMemory" in repr_str
        assert "/tmp/test.json" in repr_str

    def test_file_memory_persistence(self):
        """Test that data persists across instances."""
        from skreaver.memory import FileMemory

        with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as f:
            path = f.name

        try:
            # Create first instance and store data
            memory1 = FileMemory(path)
            memory1.store("persistent_key", "persistent_value")

            # Create second instance from same file
            memory2 = FileMemory(path)

            # Data should be loaded
            assert memory2.load("persistent_key") == "persistent_value"
        finally:
            if os.path.exists(path):
                os.unlink(path)

    def test_file_memory_cleanup_backups(self):
        """Test backup cleanup functionality."""
        from skreaver.memory import FileMemory

        with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as f:
            path = f.name

        try:
            memory = FileMemory(path)

            # Cleanup should work even with no backups
            succeeded, failed = memory.cleanup_backups(5)
            assert succeeded == 0
            assert failed == 0
        finally:
            if os.path.exists(path):
                os.unlink(path)

    def test_file_memory_invalid_key(self):
        """Test that invalid keys raise errors."""
        from skreaver.memory import FileMemory
        from skreaver.exceptions import MemoryError

        with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as f:
            path = f.name

        try:
            memory = FileMemory(path)

            # Empty key should be invalid
            with pytest.raises(MemoryError):
                memory.store("", "value")

            # Very long key should be invalid (> 128 chars)
            with pytest.raises(MemoryError):
                memory.store("x" * 200, "value")
        finally:
            if os.path.exists(path):
                os.unlink(path)


# Note: RedisMemory tests require a running Redis server
# They are marked as integration tests and skipped by default

@pytest.mark.integration
class TestRedisMemory:
    """Tests for RedisMemory backend (requires running Redis)."""

    @pytest.fixture
    def redis_url(self):
        """Get Redis URL from environment or use default."""
        return os.environ.get("REDIS_URL", "redis://localhost:6379")

    @pytest.mark.asyncio
    async def test_redis_memory_connect(self, redis_url):
        """Test RedisMemory connection."""
        pytest.importorskip("skreaver.memory", reason="redis feature not enabled")

        try:
            from skreaver.memory import RedisMemory
        except ImportError:
            pytest.skip("RedisMemory not available (redis feature not enabled)")

        try:
            memory = await RedisMemory.connect(redis_url)
            assert memory.url == redis_url
        except Exception as e:
            if "Connection refused" in str(e) or "failed to connect" in str(e).lower():
                pytest.skip(f"Redis not available: {e}")
            raise

    @pytest.mark.asyncio
    async def test_redis_memory_store_load(self, redis_url):
        """Test RedisMemory store and load."""
        try:
            from skreaver.memory import RedisMemory
        except ImportError:
            pytest.skip("RedisMemory not available (redis feature not enabled)")

        try:
            memory = await RedisMemory.connect(
                redis_url,
                key_prefix="skreaver_test"
            )

            # Store a value
            await memory.store("test_key", "test_value")

            # Load it back
            value = await memory.load("test_key")
            assert value == "test_value"

            # Cleanup
            await memory.store("test_key", "")
        except Exception as e:
            if "Connection refused" in str(e) or "failed to connect" in str(e).lower():
                pytest.skip(f"Redis not available: {e}")
            raise

    @pytest.mark.asyncio
    async def test_redis_memory_health_check(self, redis_url):
        """Test RedisMemory health check."""
        try:
            from skreaver.memory import RedisMemory
        except ImportError:
            pytest.skip("RedisMemory not available (redis feature not enabled)")

        try:
            memory = await RedisMemory.connect(redis_url)
            status = await memory.health_check()
            assert status in ["healthy", "unhealthy"]
        except Exception as e:
            if "Connection refused" in str(e) or "failed to connect" in str(e).lower():
                pytest.skip(f"Redis not available: {e}")
            raise
