# ABOUTME: Tests for --codex CLI shortcut behavior
# ABOUTME: Ensures codex convenience flag maps to ACP settings

import argparse

import pytest

from ralph_orchestrator.__main__ import _apply_codex_shortcut


def _ns(**kwargs) -> argparse.Namespace:
    # Provide defaults used by _apply_codex_shortcut
    defaults = dict(
        codex=False,
        agent="auto",
        acp_agent=None,
        acp_permission_mode=None,
        codex_permission_mode=None,
        codex_model=None,
        codex_reasoning_effort=None,
        agent_args=[],
    )
    defaults.update(kwargs)
    return argparse.Namespace(**defaults)


def test_codex_sets_acp_defaults():
    parser = argparse.ArgumentParser()
    args = _ns(codex=True)

    _apply_codex_shortcut(args, parser)

    assert args.agent == "acp"
    assert args.acp_agent == "codex-acp"
    assert args.acp_permission_mode == "interactive"
    assert args.agent_args == []


def test_codex_respects_explicit_acp_overrides():
    parser = argparse.ArgumentParser()
    args = _ns(
        codex=True,
        acp_agent="custom-acp-agent",
        acp_permission_mode="deny_all",
        codex_permission_mode="interactive",
    )

    _apply_codex_shortcut(args, parser)

    assert args.agent == "acp"
    assert args.acp_agent == "custom-acp-agent"
    assert args.acp_permission_mode == "deny_all"


def test_codex_conflicts_with_non_acp_agent():
    parser = argparse.ArgumentParser()
    args = _ns(codex=True, agent="claude")

    with pytest.raises(SystemExit):
        _apply_codex_shortcut(args, parser)


def test_codex_model_and_reasoning_append_agent_args():
    parser = argparse.ArgumentParser()
    args = _ns(
        codex=True,
        codex_model="gpt-5.2",
        codex_reasoning_effort="xhigh",
    )

    _apply_codex_shortcut(args, parser)

    assert args.agent == "acp"
    assert args.acp_agent == "codex-acp"
    assert args.acp_permission_mode == "interactive"
    assert args.agent_args == [
        "-c",
        "model=\"gpt-5.2\"",
        "-c",
        "model_reasoning_effort=\"xhigh\"",
    ]
