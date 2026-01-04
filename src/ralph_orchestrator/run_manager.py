#!/usr/bin/env python3
# ABOUTME: Run isolation and state management for Ralph orchestrator
# ABOUTME: Each prompt execution gets unique run ID with traceable manifest

import json
import os
import uuid
from dataclasses import dataclass, asdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional, Dict, Any


@dataclass
class RunInfo:
    """Information about a single run.

    Attributes:
        run_id: Unique identifier for this run
        manifest: Run manifest containing prompt_path, started_at, criteria, status
        evidence_dir: Path to validation-evidence directory for this run
        run_dir: Path to root directory for this run
    """

    run_id: str
    manifest: Dict[str, Any]
    evidence_dir: str
    run_dir: str


class RunManager:
    """Manages run isolation for Ralph orchestrator.

    Creates and retrieves runs with unique IDs, each run having:
    - Unique directory structure in .agent/runs/{id}/
    - manifest.json with prompt_path, started_at, criteria, status
    - validation-evidence/ subdirectory for evidence files
    - Pointer file in .agent/prompts/{name}/latest-run-id
    """

    def __init__(self, base_dir: str = ".agent"):
        """Initialize RunManager.

        Args:
            base_dir: Base directory for run storage (default: .agent)
        """
        self.base_dir = Path(base_dir)
        self.runs_dir = self.base_dir / "runs"
        self.prompts_dir = self.base_dir / "prompts"

    def create_run(self, prompt_path: str) -> str:
        """Create a new run for a prompt.

        Args:
            prompt_path: Path to the prompt file

        Returns:
            Unique run_id string
        """
        # Generate unique run ID with timestamp prefix for sorting
        timestamp = datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S")
        unique_suffix = uuid.uuid4().hex[:8]
        run_id = f"{timestamp}-{unique_suffix}"

        # Create run directory structure
        run_dir = self.runs_dir / run_id
        run_dir.mkdir(parents=True, exist_ok=True)

        evidence_dir = run_dir / "validation-evidence"
        evidence_dir.mkdir(parents=True, exist_ok=True)

        coordination_dir = run_dir / "coordination"
        coordination_dir.mkdir(parents=True, exist_ok=True)

        metrics_dir = run_dir / "metrics"
        metrics_dir.mkdir(parents=True, exist_ok=True)

        # Create manifest
        manifest = {
            "prompt_path": prompt_path,
            "started_at": datetime.now(timezone.utc).isoformat(),
            "status": "running",
            "criteria": [],
        }

        manifest_path = run_dir / "manifest.json"
        with open(manifest_path, "w") as f:
            json.dump(manifest, f, indent=2)

        # Update latest-run-id pointer for this prompt
        prompt_name = self._extract_prompt_name(prompt_path)
        pointer_dir = self.prompts_dir / prompt_name
        pointer_dir.mkdir(parents=True, exist_ok=True)

        pointer_path = pointer_dir / "latest-run-id"
        pointer_path.write_text(run_id)

        return run_id

    def get_run(self, run_id: str) -> Optional[RunInfo]:
        """Get information about a specific run.

        Args:
            run_id: Unique run identifier

        Returns:
            RunInfo if run exists, None otherwise
        """
        run_dir = self.runs_dir / run_id
        manifest_path = run_dir / "manifest.json"

        if not manifest_path.exists():
            return None

        with open(manifest_path) as f:
            manifest = json.load(f)

        evidence_dir = run_dir / "validation-evidence"

        return RunInfo(
            run_id=run_id,
            manifest=manifest,
            evidence_dir=str(evidence_dir),
            run_dir=str(run_dir),
        )

    def get_latest_run(self, prompt_name: str) -> Optional[RunInfo]:
        """Get the most recent run for a prompt.

        Args:
            prompt_name: Name of the prompt (without path/extension)

        Returns:
            RunInfo for most recent run, or None if no runs exist
        """
        pointer_path = self.prompts_dir / prompt_name / "latest-run-id"

        if not pointer_path.exists():
            return None

        run_id = pointer_path.read_text().strip()
        return self.get_run(run_id)

    def update_run_status(self, run_id: str, status: str) -> bool:
        """Update the status of a run.

        Args:
            run_id: Unique run identifier
            status: New status value

        Returns:
            True if updated successfully, False if run not found
        """
        run_dir = self.runs_dir / run_id
        manifest_path = run_dir / "manifest.json"

        if not manifest_path.exists():
            return False

        with open(manifest_path) as f:
            manifest = json.load(f)

        manifest["status"] = status
        manifest["updated_at"] = datetime.now(timezone.utc).isoformat()

        with open(manifest_path, "w") as f:
            json.dump(manifest, f, indent=2)

        return True

    def list_runs(self, prompt_name: Optional[str] = None) -> list[RunInfo]:
        """List all runs, optionally filtered by prompt name.

        Args:
            prompt_name: Optional prompt name to filter by

        Returns:
            List of RunInfo objects, sorted by creation time (newest first)
        """
        if not self.runs_dir.exists():
            return []

        runs = []
        for run_dir in self.runs_dir.iterdir():
            if not run_dir.is_dir():
                continue

            run_info = self.get_run(run_dir.name)
            if run_info is None:
                continue

            if prompt_name is not None:
                run_prompt_name = self._extract_prompt_name(
                    run_info.manifest.get("prompt_path", "")
                )
                if run_prompt_name != prompt_name:
                    continue

            runs.append(run_info)

        # Sort by run_id (which starts with timestamp) descending
        runs.sort(key=lambda r: r.run_id, reverse=True)
        return runs

    def _extract_prompt_name(self, prompt_path: str) -> str:
        """Extract prompt name from path.

        Examples:
            prompts/orchestration/PROMPT.md -> PROMPT
            SELF_IMPROVEMENT_PROMPT.md -> SELF_IMPROVEMENT_PROMPT
        """
        path = Path(prompt_path)
        return path.stem
