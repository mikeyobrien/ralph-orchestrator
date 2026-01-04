#!/usr/bin/env python3
# ABOUTME: Unit tests for RunManager - run isolation and state management
# ABOUTME: Tests create_run, get_run, get_latest_run, and manifest structure

import json
import os
import pytest
import shutil
import tempfile
from pathlib import Path
from datetime import datetime


class TestRunManager:
    """Tests for RunManager class - run isolation infrastructure."""

    @pytest.fixture(autouse=True)
    def setup(self, tmp_path):
        """Set up test environment with temporary directory."""
        self.test_dir = tmp_path
        self.original_cwd = os.getcwd()
        os.chdir(self.test_dir)

        # Create test prompt file
        prompts_dir = self.test_dir / "prompts" / "orchestration"
        prompts_dir.mkdir(parents=True)
        (prompts_dir / "PROMPT.md").write_text("# Test Prompt\nThis is a test.")

        yield

        os.chdir(self.original_cwd)

    def test_import_run_manager(self):
        """RunManager should be importable from ralph_orchestrator.run_manager."""
        from ralph_orchestrator.run_manager import RunManager
        assert RunManager is not None

    def test_import_run_info(self):
        """RunInfo should be importable from ralph_orchestrator.run_manager."""
        from ralph_orchestrator.run_manager import RunInfo
        assert RunInfo is not None

    def test_create_run_returns_run_id(self):
        """create_run should return a unique run_id string."""
        from ralph_orchestrator.run_manager import RunManager

        rm = RunManager()
        run_id = rm.create_run("prompts/orchestration/PROMPT.md")

        assert run_id is not None
        assert isinstance(run_id, str)
        assert len(run_id) > 0

    def test_create_run_creates_directory_structure(self):
        """create_run should create .agent/runs/{id}/ directory structure."""
        from ralph_orchestrator.run_manager import RunManager

        rm = RunManager()
        run_id = rm.create_run("prompts/orchestration/PROMPT.md")

        run_dir = Path(".agent/runs") / run_id
        assert run_dir.exists()
        assert run_dir.is_dir()

    def test_create_run_creates_manifest(self):
        """create_run should create manifest.json with required fields."""
        from ralph_orchestrator.run_manager import RunManager

        rm = RunManager()
        run_id = rm.create_run("prompts/orchestration/PROMPT.md")

        manifest_path = Path(".agent/runs") / run_id / "manifest.json"
        assert manifest_path.exists()

        with open(manifest_path) as f:
            manifest = json.load(f)

        # Verify required fields
        assert "prompt_path" in manifest
        assert manifest["prompt_path"] == "prompts/orchestration/PROMPT.md"
        assert "started_at" in manifest
        assert "status" in manifest

    def test_create_run_creates_validation_evidence_dir(self):
        """create_run should create validation-evidence/ subdirectory."""
        from ralph_orchestrator.run_manager import RunManager

        rm = RunManager()
        run_id = rm.create_run("prompts/orchestration/PROMPT.md")

        evidence_dir = Path(".agent/runs") / run_id / "validation-evidence"
        assert evidence_dir.exists()
        assert evidence_dir.is_dir()

    def test_get_run_returns_run_info(self):
        """get_run should return RunInfo with manifest and paths."""
        from ralph_orchestrator.run_manager import RunManager, RunInfo

        rm = RunManager()
        run_id = rm.create_run("prompts/orchestration/PROMPT.md")

        run_info = rm.get_run(run_id)

        assert run_info is not None
        assert isinstance(run_info, RunInfo)

    def test_run_info_has_manifest(self):
        """RunInfo should have manifest dict."""
        from ralph_orchestrator.run_manager import RunManager

        rm = RunManager()
        run_id = rm.create_run("prompts/orchestration/PROMPT.md")
        run_info = rm.get_run(run_id)

        assert hasattr(run_info, "manifest")
        assert isinstance(run_info.manifest, dict)
        assert run_info.manifest["prompt_path"] == "prompts/orchestration/PROMPT.md"

    def test_run_info_has_evidence_dir(self):
        """RunInfo should have evidence_dir path."""
        from ralph_orchestrator.run_manager import RunManager

        rm = RunManager()
        run_id = rm.create_run("prompts/orchestration/PROMPT.md")
        run_info = rm.get_run(run_id)

        assert hasattr(run_info, "evidence_dir")
        assert Path(run_info.evidence_dir).exists()

    def test_run_info_has_run_id(self):
        """RunInfo should have run_id."""
        from ralph_orchestrator.run_manager import RunManager

        rm = RunManager()
        run_id = rm.create_run("prompts/orchestration/PROMPT.md")
        run_info = rm.get_run(run_id)

        assert hasattr(run_info, "run_id")
        assert run_info.run_id == run_id

    def test_get_run_nonexistent_returns_none(self):
        """get_run with nonexistent run_id should return None."""
        from ralph_orchestrator.run_manager import RunManager

        rm = RunManager()
        run_info = rm.get_run("nonexistent-run-id-12345")

        assert run_info is None

    def test_get_latest_run_returns_most_recent(self):
        """get_latest_run should return the most recent run for a prompt."""
        from ralph_orchestrator.run_manager import RunManager
        import time

        rm = RunManager()

        # Create two runs for same prompt
        run_id_1 = rm.create_run("prompts/orchestration/PROMPT.md")
        time.sleep(0.01)  # Small delay to ensure different timestamps
        run_id_2 = rm.create_run("prompts/orchestration/PROMPT.md")

        latest = rm.get_latest_run("PROMPT")

        assert latest is not None
        assert latest.run_id == run_id_2

    def test_get_latest_run_nonexistent_prompt_returns_none(self):
        """get_latest_run with no runs should return None."""
        from ralph_orchestrator.run_manager import RunManager

        rm = RunManager()
        latest = rm.get_latest_run("NONEXISTENT_PROMPT")

        assert latest is None

    def test_manifest_includes_started_at_iso_format(self):
        """Manifest started_at should be ISO 8601 format."""
        from ralph_orchestrator.run_manager import RunManager

        rm = RunManager()
        run_id = rm.create_run("prompts/orchestration/PROMPT.md")
        run_info = rm.get_run(run_id)

        started_at = run_info.manifest["started_at"]
        # Should be parseable as ISO format
        datetime.fromisoformat(started_at.replace("Z", "+00:00"))

    def test_manifest_includes_status(self):
        """Manifest should include status field."""
        from ralph_orchestrator.run_manager import RunManager

        rm = RunManager()
        run_id = rm.create_run("prompts/orchestration/PROMPT.md")
        run_info = rm.get_run(run_id)

        assert "status" in run_info.manifest
        assert run_info.manifest["status"] == "running"

    def test_unique_run_ids(self):
        """Multiple create_run calls should generate unique run_ids."""
        from ralph_orchestrator.run_manager import RunManager

        rm = RunManager()
        run_ids = set()

        for _ in range(10):
            run_id = rm.create_run("prompts/orchestration/PROMPT.md")
            assert run_id not in run_ids
            run_ids.add(run_id)

    def test_creates_prompt_latest_pointer(self):
        """create_run should update .agent/prompts/{name}/latest-run-id."""
        from ralph_orchestrator.run_manager import RunManager

        rm = RunManager()
        run_id = rm.create_run("prompts/orchestration/PROMPT.md")

        pointer_path = Path(".agent/prompts/PROMPT/latest-run-id")
        assert pointer_path.exists()

        stored_id = pointer_path.read_text().strip()
        assert stored_id == run_id

    def test_run_manager_custom_base_dir(self):
        """RunManager should accept custom base directory."""
        from ralph_orchestrator.run_manager import RunManager

        custom_dir = Path(".custom-agent")
        rm = RunManager(base_dir=str(custom_dir))
        run_id = rm.create_run("prompts/orchestration/PROMPT.md")

        run_dir = custom_dir / "runs" / run_id
        assert run_dir.exists()
