"""Process-local cache of `larql.load` (Vindex) and `larql.session` per workspace path.

PyO3 exposes Vindex/Session as **unsendable** — they must not move across OS threads.
We keep handles in **thread-local** storage and use global epoch counters so
``invalidate()`` / path switches still drop stale views on every thread.

Sliding TTL from last access; TTL is checked only when epochs still match.
"""

from __future__ import annotations

import os
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import larql


def _norm_workspace_path(path: str) -> str:
    return str(Path(path).expanduser().resolve())


def normalize_workspace_path(path: str) -> str:
    """Resolved absolute path for comparing current vs run workspace (409 rerun guard)."""
    return _norm_workspace_path(path)


def workspace_paths_differ(previous: str | None, new_path: str) -> bool:
    """True when switching to a different resolved workspace directory."""
    if previous is None or not str(previous).strip():
        return False
    try:
        return _norm_workspace_path(previous) != _norm_workspace_path(new_path)
    except (OSError, ValueError):
        return True


def _ttl_seconds() -> float:
    raw = os.environ.get("LARQL_UI_RUNTIME_TTL", "").strip()
    if not raw:
        return 300.0
    try:
        return max(30.0, float(raw))
    except ValueError:
        return 300.0


@dataclass
class _RuntimeCacheEntry:
    mono: float
    vindex: Any | None
    session: Any | None
    full_epoch: int
    path_epoch: int


@dataclass
class _ThreadCache:
    lock: threading.Lock
    by_path: dict[str, _RuntimeCacheEntry]


class LarqlRuntimeCache:
    def __init__(self, ttl_seconds: float | None = None) -> None:
        self._ttl = ttl_seconds if ttl_seconds is not None else _ttl_seconds()
        self._local = threading.local()
        self._epoch_lock = threading.Lock()
        self._full_epoch = 0
        self._path_epochs: dict[str, int] = {}

    def _thread_cache(self) -> _ThreadCache:
        tc = getattr(self._local, "tc", None)
        if tc is None:
            tc = _ThreadCache(threading.Lock(), {})
            self._local.tc = tc
        return tc

    def _epochs_for_key(self, key: str) -> tuple[int, int]:
        with self._epoch_lock:
            return self._full_epoch, self._path_epochs.get(key, 0)

    def invalidate(self, path: str | None = None) -> None:
        """Drop cached handles for ``path``, or all workspaces when ``path is None``.

        Bumps global/path epochs so other OS threads stop reusing stale handles.
        """
        with self._epoch_lock:
            if path is None:
                self._full_epoch += 1
            else:
                k = _norm_workspace_path(path)
                self._path_epochs[k] = self._path_epochs.get(k, 0) + 1
        tc = self._thread_cache()
        with tc.lock:
            if path is None:
                tc.by_path.clear()
            else:
                tc.by_path.pop(_norm_workspace_path(path), None)

    def _expired(self, mono: float) -> bool:
        return (time.monotonic() - mono) > self._ttl

    def vindex(self, workspace_path: str) -> Any:
        key = _norm_workspace_path(workspace_path)
        tc = self._thread_cache()
        with tc.lock:
            entry = tc.by_path.get(key)
            fe, pe = self._epochs_for_key(key)
            if (
                entry
                and entry.vindex is not None
                and entry.full_epoch == fe
                and entry.path_epoch == pe
                and not self._expired(entry.mono)
            ):
                entry.mono = time.monotonic()
                return entry.vindex

        loaded = larql.load(workspace_path)
        fe, pe = self._epochs_for_key(key)
        now = time.monotonic()
        with tc.lock:
            entry = tc.by_path.get(key)
            if entry is None:
                tc.by_path[key] = _RuntimeCacheEntry(now, loaded, None, fe, pe)
            else:
                entry.vindex = loaded
                entry.full_epoch = fe
                entry.path_epoch = pe
                entry.mono = now
            return loaded

    def session(self, workspace_path: str) -> Any:
        key = _norm_workspace_path(workspace_path)
        tc = self._thread_cache()
        with tc.lock:
            entry = tc.by_path.get(key)
            fe, pe = self._epochs_for_key(key)
            if (
                entry
                and entry.session is not None
                and entry.full_epoch == fe
                and entry.path_epoch == pe
                and not self._expired(entry.mono)
            ):
                entry.mono = time.monotonic()
                return entry.session

        sess = larql.session(workspace_path)
        fe, pe = self._epochs_for_key(key)
        now = time.monotonic()
        with tc.lock:
            entry = tc.by_path.get(key)
            if entry is None:
                tc.by_path[key] = _RuntimeCacheEntry(now, None, sess, fe, pe)
            else:
                entry.session = sess
                entry.full_epoch = fe
                entry.path_epoch = pe
                entry.mono = now
            return sess
