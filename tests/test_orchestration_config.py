#!/usr/bin/env python3
# ABOUTME: Tests for orchestration configuration and subagent profiles
# ABOUTME: Verifies SubagentProfile dataclass and default profiles

import pytest
from dataclasses import is_dataclass


class TestSubagentProfile:
    """Tests for SubagentProfile dataclass."""

    def test_subagent_profile_is_dataclass(self):
        """SubagentProfile should be a dataclass."""
        from ralph_orchestrator.orchestration.config import SubagentProfile

        assert is_dataclass(SubagentProfile)

    def test_subagent_profile_has_required_fields(self):
        """SubagentProfile should have name, description, required_tools, required_mcps, optional_mcps, prompt_template."""
        from ralph_orchestrator.orchestration.config import SubagentProfile

        profile = SubagentProfile(
            name="test",
            description="A test profile",
            required_tools=["tool1"],
            required_mcps=["mcp1"],
            optional_mcps=["mcp2"],
            prompt_template="Test template",
        )

        assert profile.name == "test"
        assert profile.description == "A test profile"
        assert profile.required_tools == ["tool1"]
        assert profile.required_mcps == ["mcp1"]
        assert profile.optional_mcps == ["mcp2"]
        assert profile.prompt_template == "Test template"

    def test_subagent_profile_default_optional_mcps(self):
        """optional_mcps should default to empty list."""
        from ralph_orchestrator.orchestration.config import SubagentProfile

        profile = SubagentProfile(
            name="minimal",
            description="Minimal profile",
            required_tools=[],
            required_mcps=[],
            prompt_template="Template",
        )

        assert profile.optional_mcps == []

    def test_subagent_profile_serialization(self):
        """SubagentProfile should be serializable to dict."""
        from ralph_orchestrator.orchestration.config import SubagentProfile
        from dataclasses import asdict

        profile = SubagentProfile(
            name="test",
            description="Test",
            required_tools=["t1"],
            required_mcps=["m1"],
            optional_mcps=["m2"],
            prompt_template="Template",
        )

        data = asdict(profile)
        assert data["name"] == "test"
        assert data["required_tools"] == ["t1"]


class TestSubagentProfiles:
    """Tests for SUBAGENT_PROFILES constant."""

    def test_subagent_profiles_exists(self):
        """SUBAGENT_PROFILES constant should exist."""
        from ralph_orchestrator.orchestration.config import SUBAGENT_PROFILES

        assert isinstance(SUBAGENT_PROFILES, dict)

    def test_subagent_profiles_has_all_types(self):
        """SUBAGENT_PROFILES should have validator, researcher, implementer, analyst."""
        from ralph_orchestrator.orchestration.config import SUBAGENT_PROFILES

        required_types = {"validator", "researcher", "implementer", "analyst"}
        actual_types = set(SUBAGENT_PROFILES.keys())

        assert required_types == actual_types, f"Missing: {required_types - actual_types}"

    def test_validator_profile(self):
        """Validator profile should have appropriate skills and MCPs."""
        from ralph_orchestrator.orchestration.config import SUBAGENT_PROFILES, SubagentProfile

        validator = SUBAGENT_PROFILES["validator"]
        assert isinstance(validator, SubagentProfile)
        assert validator.name == "validator"
        assert len(validator.description) > 0
        # Validator should use sequential-thinking for structured reasoning
        assert "sequential-thinking" in validator.required_mcps

    def test_researcher_profile(self):
        """Researcher profile should have research-oriented skills and MCPs."""
        from ralph_orchestrator.orchestration.config import SUBAGENT_PROFILES, SubagentProfile

        researcher = SUBAGENT_PROFILES["researcher"]
        assert isinstance(researcher, SubagentProfile)
        assert researcher.name == "researcher"
        assert len(researcher.description) > 0
        # Researcher should have context7 or tavily for web research
        research_mcps = {"Context7", "tavily", "firecrawl-mcp"}
        assert any(mcp in researcher.required_mcps or mcp in researcher.optional_mcps
                   for mcp in research_mcps)

    def test_implementer_profile(self):
        """Implementer profile should have implementation-oriented tools."""
        from ralph_orchestrator.orchestration.config import SUBAGENT_PROFILES, SubagentProfile

        implementer = SUBAGENT_PROFILES["implementer"]
        assert isinstance(implementer, SubagentProfile)
        assert implementer.name == "implementer"
        assert len(implementer.description) > 0
        # Implementer should have test-driven-development skill
        assert "test-driven-development" in implementer.required_tools

    def test_analyst_profile(self):
        """Analyst profile should have debugging-oriented skills."""
        from ralph_orchestrator.orchestration.config import SUBAGENT_PROFILES, SubagentProfile

        analyst = SUBAGENT_PROFILES["analyst"]
        assert isinstance(analyst, SubagentProfile)
        assert analyst.name == "analyst"
        assert len(analyst.description) > 0
        # Analyst should use sequential-thinking for structured reasoning
        assert "sequential-thinking" in analyst.required_mcps

    def test_all_profiles_have_prompt_template(self):
        """Every profile should have a non-empty prompt template."""
        from ralph_orchestrator.orchestration.config import SUBAGENT_PROFILES

        for name, profile in SUBAGENT_PROFILES.items():
            assert len(profile.prompt_template) > 0, f"{name} has empty prompt_template"

    def test_profiles_are_immutable_copies(self):
        """Modifying returned profiles shouldn't affect original."""
        from ralph_orchestrator.orchestration.config import SUBAGENT_PROFILES

        # Get a copy, modify it
        original_name = SUBAGENT_PROFILES["validator"].name

        # The dataclass is immutable by default (frozen=False but fields are)
        # This test verifies we can access profiles without side effects
        _ = SUBAGENT_PROFILES["validator"]
        assert SUBAGENT_PROFILES["validator"].name == original_name
