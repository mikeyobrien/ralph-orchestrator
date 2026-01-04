# ABOUTME: Tests for agent_priority-driven adapter ordering and auto selection
# ABOUTME: Ensures auto mode can prefer ACP/Codex and control fallback ordering

import pytest

from ralph_orchestrator.main import RalphConfig, AgentType, AdapterConfig
from ralph_orchestrator.orchestrator import RalphOrchestrator


class _DummyClaudeAdapter:
    def __init__(self, verbose: bool = False) -> None:
        self.name = "claude"
        self.available = True


class _DummyGeminiAdapter:
    def __init__(self) -> None:
        self.name = "gemini"
        self.available = True


class _DummyQChatAdapter:
    def __init__(self) -> None:
        self.name = "qchat"
        self.available = True


class _DummyACPAdapter:
    def __init__(
        self,
        agent_command: str = "gemini",
        agent_args: list[str] | None = None,
        timeout: int = 300,
        permission_mode: str = "auto_approve",
        permission_allowlist: list[str] | None = None,
        verbose: bool = False,
    ) -> None:
        self.name = "acp"
        self.available = True
        self.agent_command = agent_command
        self.agent_args = agent_args or []
        self.timeout = timeout
        self.permission_mode = permission_mode
        self.permission_allowlist = permission_allowlist or []
        self.verbose = verbose


def test_agent_priority_orders_adapters_and_auto_selects_first(tmp_path, monkeypatch):
    # Patch adapters so this test is deterministic and does not depend on installed CLIs.
    import ralph_orchestrator.orchestrator as orch_mod

    monkeypatch.setattr(orch_mod, "ClaudeAdapter", _DummyClaudeAdapter)
    monkeypatch.setattr(orch_mod, "GeminiAdapter", _DummyGeminiAdapter)
    monkeypatch.setattr(orch_mod, "QChatAdapter", _DummyQChatAdapter)
    monkeypatch.setattr(orch_mod, "ACPAdapter", _DummyACPAdapter)

    prompt_file = tmp_path / "PROMPT.md"
    prompt_file.write_text("# Task: Test\n\nDo nothing.\n", encoding="utf-8")

    config = RalphConfig(
        agent=AgentType.AUTO,
        prompt_file=str(prompt_file),
        agent_priority=["acp", "claude", "gemini"],
        adapters={
            "q": AdapterConfig(enabled=False),
            "acp": AdapterConfig(
                enabled=True,
                tool_permissions={
                    "agent_command": "codex-acp",
                    "agent_args": [
                        "-c",
                        'model="gpt-5.2"',
                        "-c",
                        'model_reasoning_effort="high"',
                    ],
                    "permission_mode": "auto_approve",
                    "permission_allowlist": [],
                },
            ),
        },
    )

    orchestrator = RalphOrchestrator(prompt_file_or_config=config)

    # Auto mode should pick the first available adapter in the priority list.
    assert orchestrator.current_adapter is orchestrator.adapters["acp"]

    # Adapter dict insertion order drives fallback order.
    assert list(orchestrator.adapters.keys()) == ["acp", "claude", "gemini"]

    # ACP adapter should be configured from config.tool_permissions.
    acp = orchestrator.adapters["acp"]
    assert acp.agent_command == "codex-acp"
    assert acp.permission_mode == "auto_approve"
    assert acp.agent_args == [
        "-c",
        'model="gpt-5.2"',
        "-c",
        'model_reasoning_effort="high"',
    ]
