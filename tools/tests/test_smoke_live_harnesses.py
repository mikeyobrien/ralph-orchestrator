#!/usr/bin/env python3
"""Integration coverage for the manual live-harness smoke without paid calls."""

from __future__ import annotations

import json
import os
import re
import shutil
import stat
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RUNNER = ROOT / "tools/smoke-live-harnesses.sh"
PARSER = ROOT / "tools/smoke_live_harness_results.py"
PRESET = ROOT / "presets/live-harness-smoke"
BACKENDS = (
    ("claude", "claude-sdk", "claude", "smoke.claude.done"),
    ("codex", "command", "codex", "smoke.codex.done"),
    ("opencode", "command", "opencode", "smoke.opencode.done"),
    ("pi", "pi", "pi", "smoke.pi.done"),
    ("hermes", "acp", "hermes", "smoke.hermes.done"),
    ("kiro", "acp", "kiro-cli", "smoke.complete"),
)


def executable(path: Path, content: str) -> Path:
    path.write_text(content, encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    return path


def journal_records(run: str = "fake-six-provider") -> list[dict[str, object]]:
    records: list[dict[str, object]] = [
        {"run": run, "topic": "loop.start", "fields": {"completion_event": "smoke.complete"}}
    ]
    for iteration, (name, kind, command, handoff) in enumerate(BACKENDS, 1):
        response = f"HARNESS_OK:{name}:HARNESS_SMOKE:{name}"
        records.extend(
            [
                {
                    "run": run,
                    "iteration": str(iteration),
                    "topic": "backend.start",
                    "fields": {"backend_kind": kind, "command": command},
                },
                {
                    "run": run,
                    "iteration": str(iteration),
                    "topic": handoff,
                    "payload": response,
                    "source": "agent",
                },
                {
                    "run": run,
                    "iteration": str(iteration),
                    "topic": "backend.finish",
                    "fields": {
                        "exit_code": "0",
                        "timed_out": False,
                        "output": response,
                    },
                },
            ]
        )
    records.append(
        {"run": run, "topic": "loop.stop", "fields": {"reason": "completion_event"}}
    )
    return records


def write_fixture(workspace: Path, records: list[dict[str, object]] | None = None) -> tuple[Path, Path, Path]:
    run_dir = workspace / ".autoloop/runs/fake-six-provider"
    run_dir.mkdir(parents=True)
    journal = workspace / ".autoloop/journal.jsonl"
    journal.write_text(
        "".join(json.dumps(record) + "\n" for record in (records or journal_records())),
        encoding="utf-8",
    )
    evidence = run_dir / "smoke-evidence.txt"
    evidence.write_text(
        "".join(f"HARNESS_SMOKE:{name}\n" for name, *_ in BACKENDS), encoding="utf-8"
    )
    output = workspace / "ralph-output.log"
    output.write_text("fake Ralph completed\n", encoding="utf-8")
    return journal, evidence, output


class FakeRunnerIntegration(unittest.TestCase):
    def test_real_runner_path_reports_six_passes_without_paid_executables(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            bin_dir = root / "bin"
            bin_dir.mkdir()
            paid_marker = root / "PAID_PROVIDER_WAS_CALLED"
            probe_trace = root / "probe.trace"

            # These names satisfy preflight but are hard traps: fake Ralph must model
            # logical provider turns through fake-probe, never launch provider CLIs.
            trap = executable(
                bin_dir / "provider-trap",
                "#!/bin/sh\nprintf '%s\\n' \"$0\" >>\"$PAID_MARKER\"\nexit 97\n",
            )
            for command in ("claude", "codex", "opencode", "pi", "hermes", "kiro-cli"):
                (bin_dir / command).symlink_to(trap)

            executable(bin_dir / "autoloop", "#!/bin/sh\nexit 98\n")
            executable(
                bin_dir / "fake-probe",
                "#!/bin/sh\nset -eu\nid=$1\nevidence=$2\ntrace=$3\nprintf 'call:%s\\n' \"$id\" >>\"$trace\"\nprintf 'HARNESS_SMOKE:%s\\n' \"$id\" >>\"$evidence\"\nprintf 'result:%s:HARNESS_SMOKE:%s\\n' \"$id\" \"$id\" >>\"$trace\"\n",
            )
            fake_ralph = executable(
                bin_dir / "ralph",
                textwrap.dedent(
                    f"""\
                    #!/usr/bin/env python3
                    import json, os, pathlib, subprocess
                    backends = {BACKENDS!r}
                    cwd = pathlib.Path.cwd()
                    run = "fake-six-provider"
                    run_dir = cwd / ".autoloop/runs" / run
                    run_dir.mkdir(parents=True)
                    evidence = run_dir / "smoke-evidence.txt"
                    trace = pathlib.Path(os.environ["PROBE_TRACE"])
                    records = [{{"run": run, "topic": "loop.start", "fields": {{"completion_event": "smoke.complete"}}}}]
                    for iteration, (name, kind, command, handoff) in enumerate(backends, 1):
                        subprocess.run(["fake-probe", name, str(evidence), str(trace)], check=True)
                        response = f"HARNESS_OK:{{name}}:HARNESS_SMOKE:{{name}}"
                        records += [
                            {{"run": run, "iteration": str(iteration), "topic": "backend.start", "fields": {{"backend_kind": kind, "command": command}}}},
                            {{"run": run, "iteration": str(iteration), "topic": handoff, "payload": response, "source": "agent"}},
                            {{"run": run, "iteration": str(iteration), "topic": "backend.finish", "fields": {{"exit_code": "0", "timed_out": False, "output": response}}}},
                        ]
                    records.append({{"run": run, "topic": "loop.stop", "fields": {{"reason": "completion_event", "elapsed_s": "1", "cost_usd": "0"}}}})
                    (cwd / ".autoloop/journal.jsonl").write_text("".join(json.dumps(r) + "\\n" for r in records))
                    print("fake six-provider launch complete; elapsed_s=1 cost_usd=0")
                    """
                ),
            )
            env = os.environ.copy()
            env.update(
                {
                    "PATH": f"{bin_dir}:{env['PATH']}",
                    "RALPH_BIN": str(fake_ralph),
                    "AUTOLOOP_BIN": str(bin_dir / "autoloop"),
                    "KEEP_SMOKE_DIR": "1",
                    "SMOKE_TIMEOUT_SECONDS": "30",
                    "PAID_MARKER": str(paid_marker),
                    "PROBE_TRACE": str(probe_trace),
                }
            )
            result = subprocess.run(
                ["bash", str(RUNNER)], cwd=ROOT, env=env, text=True, capture_output=True, timeout=45
            )
            combined = result.stdout + result.stderr
            self.assertEqual(result.returncode, 0, combined)
            rows = [line for line in result.stdout.splitlines() if line.rstrip().endswith("PASS")]
            self.assertEqual([line.split()[0] for line in rows], [item[0] for item in BACKENDS])
            print("\n" + "\n".join(rows))
            self.assertFalse(paid_marker.exists(), "a paid-provider trap executable was invoked")
            expected_trace = []
            for name, *_ in BACKENDS:
                expected_trace += [f"call:{name}", f"result:{name}:HARNESS_SMOKE:{name}"]
            self.assertEqual(probe_trace.read_text().splitlines(), expected_trace)

            match = re.search(r"Smoke workspace retained: (.+)", combined)
            self.assertIsNotNone(match, combined)
            retained = Path(match.group(1).strip())
            self.assertEqual(
                (retained / ".autoloop/runs/fake-six-provider/smoke-evidence.txt").read_text().splitlines(),
                [f"HARNESS_SMOKE:{name}" for name, *_ in BACKENDS],
            )
            shutil.rmtree(retained)

    def test_failure_matrix_is_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            base_records = journal_records()
            cases: dict[str, tuple[list[dict[str, object]], str | None, str]] = {}
            cases["missing response"] = (
                [r for r in base_records if not (r["topic"] == "backend.finish" and r.get("iteration") == "3")],
                None,
                "missing backend.finish/tool response",
            )
            cases["missing handoff"] = (
                [r for r in base_records if r["topic"] != "smoke.pi.done"], None, "handoffs must occur exactly once"
            )
            cases["generic completion only"] = (
                [r for r in base_records if r["topic"] != "smoke.complete"], None, "literal emitted topic smoke.complete"
            )
            broken = [dict(r) for r in base_records]
            broken[-2] = {**broken[-2], "fields": {"exit_code": "9", "timed_out": False, "output": "backend exploded"}}
            cases["backend failure"] = (broken, None, "backend.finish failed")
            cases["malformed journal"] = (base_records, "{not-json}\n", "malformed native journal")

            for label, (records, journal_override, diagnostic) in cases.items():
                with self.subTest(label=label):
                    case_dir = root / label.replace(" ", "-")
                    case_dir.mkdir()
                    journal, evidence, output = write_fixture(case_dir, records)
                    if journal_override is not None:
                        journal.write_text(journal_override, encoding="utf-8")
                    if label == "missing response":
                        pass
                    result = subprocess.run(
                        [
                            "python3", str(PARSER), "--journal", str(journal), "--evidence", str(evidence),
                            "--output", str(output), "--workspace", str(case_dir), "--rerun", "fake-rerun",
                            "--ralph-status", "0",
                        ],
                        text=True, capture_output=True,
                    )
                    self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                    self.assertIn(diagnostic, result.stderr)

            timeout_dir = root / "timeout"
            timeout_dir.mkdir()
            journal, evidence, output = write_fixture(timeout_dir)
            timed_out = subprocess.run(
                ["python3", str(PARSER), "--journal", str(journal), "--evidence", str(evidence),
                 "--output", str(output), "--workspace", str(timeout_dir), "--rerun", "fake-rerun",
                 "--ralph-status", "143", "--timed-out"],
                text=True, capture_output=True,
            )
            self.assertNotEqual(timed_out.returncode, 0)
            self.assertIn("watchdog timed out", timed_out.stderr)

            for label, content in {
                "missing evidence": "".join(f"HARNESS_SMOKE:{n}\n" for n, *_ in BACKENDS[:-1]),
                "duplicate evidence": "HARNESS_SMOKE:claude\n" + "".join(f"HARNESS_SMOKE:{n}\n" for n, *_ in BACKENDS),
                "out of order evidence": "HARNESS_SMOKE:codex\nHARNESS_SMOKE:claude\n" + "".join(f"HARNESS_SMOKE:{n}\n" for n, *_ in BACKENDS[2:]),
            }.items():
                with self.subTest(label=label):
                    case_dir = root / label.replace(" ", "-")
                    case_dir.mkdir()
                    journal, evidence, output = write_fixture(case_dir)
                    evidence.write_text(content, encoding="utf-8")
                    result = subprocess.run(
                        ["python3", str(PARSER), "--journal", str(journal), "--evidence", str(evidence),
                         "--output", str(output), "--workspace", str(case_dir), "--rerun", "fake-rerun",
                         "--ralph-status", "0"],
                        text=True, capture_output=True,
                    )
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("exactly six ordered unique", result.stderr)


class PromptProbeContract(unittest.TestCase):
    @staticmethod
    def command_for(name: str) -> str:
        prompt = (PRESET / "roles" / f"{name}.md").read_text(encoding="utf-8")
        match = re.search(r"```sh\n(.*?)\n```", prompt, re.DOTALL)
        if match is None:
            raise AssertionError(f"missing shell probe in {name} prompt")
        return match.group(1)

    def test_fixed_probes_use_the_sole_provider_visible_run_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            workspace = Path(temp)
            run_dir = workspace / ".autoloop/runs/live-smoke"
            run_dir.mkdir(parents=True)

            for name, *_ in BACKENDS:
                command = self.command_for(name)
                self.assertNotIn("AUTOLOOP_STATE_DIR", command)
                result = subprocess.run(
                    ["/bin/sh", "-c", command],
                    cwd=workspace,
                    text=True,
                    capture_output=True,
                )
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual(result.stdout, f"HARNESS_SMOKE:{name}\n")

            self.assertEqual(
                (run_dir / "smoke-evidence.txt").read_text(encoding="utf-8").splitlines(),
                [f"HARNESS_SMOKE:{name}" for name, *_ in BACKENDS],
            )

    def test_probe_fails_closed_without_exactly_one_run_directory(self) -> None:
        command = self.command_for("claude")
        for directory_count in (0, 2):
            with self.subTest(directory_count=directory_count), tempfile.TemporaryDirectory() as temp:
                workspace = Path(temp)
                runs = workspace / ".autoloop/runs"
                runs.mkdir(parents=True)
                for index in range(directory_count):
                    (runs / f"run-{index}").mkdir()
                result = subprocess.run(
                    ["/bin/sh", "-c", command],
                    cwd=workspace,
                    text=True,
                    capture_output=True,
                )
                self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual(list(runs.glob("*/smoke-evidence.txt")), [])


class NativePresetSelectionIntegration(unittest.TestCase):
    def test_all_role_backend_overrides_reach_native_launch_selection(self) -> None:
        autoloop = os.environ.get("AUTOLOOP_BIN") or shutil.which("autoloop")
        if not autoloop:
            self.skipTest("autoloop executable unavailable; CI engine-contract job must provide AUTOLOOP_BIN")
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            zero_work = root / "zero-load"
            zero_work.mkdir()
            subprocess.run(["git", "init", "-q", str(zero_work)], check=True)
            zero = subprocess.run(
                [autoloop, "run", str(PRESET), "--max-iterations", "0", "native schema load only"],
                cwd=zero_work, text=True, capture_output=True, timeout=30,
            )
            self.assertEqual(zero.returncode, 0, zero.stdout + zero.stderr)
            zero_records = [
                json.loads(line)
                for line in (zero_work / ".autoloop/journal.jsonl").read_text().splitlines()
            ]
            self.assertEqual(
                [record["fields"].get("reason") for record in zero_records if record["topic"] == "loop.stop"],
                ["max_iterations"],
            )
            self.assertIn("cost_usd: 0.000000", zero.stdout)

            guard = executable(root / "backend-guard", "#!/bin/sh\nexit 86\n")
            expected_args = {
                "claude": "",
                "codex": "exec,--yolo",
                "opencode": "run",
                "pi": "",
                "hermes": "acp",
                "kiro": "acp",
            }
            for name, kind, _command, _handoff in BACKENDS:
                case = root / name
                shutil.copytree(PRESET, case / "preset")
                topology = case / "preset/topology.toml"
                text = topology.read_text(encoding="utf-8")
                text = re.sub(r'backend_command = "[^"]+"', f'backend_command = "{guard}"', text)
                text = text.replace('"loop.start" = ["claude"]', f'"loop.start" = ["{name}"]')
                topology.write_text(text, encoding="utf-8")
                work = case / "work"
                work.mkdir()
                subprocess.run(["git", "init", "-q", str(work)], check=True)
                result = subprocess.run(
                    [autoloop, "run", str(case / "preset"), "--max-iterations", "1", "fake selection only"],
                    cwd=work, text=True, capture_output=True, timeout=30,
                )
                records = [json.loads(line) for line in (work / ".autoloop/journal.jsonl").read_text().splitlines()]
                starts = [record for record in records if record["topic"] == "backend.start"]
                self.assertEqual(len(starts), 1, result.stdout + result.stderr)
                stops = [record for record in records if record["topic"] == "loop.stop"]
                self.assertEqual(len(stops), 1, result.stdout + result.stderr)
                self.assertIn(stops[0]["fields"]["reason"], {"backend_failed", "backend_auth", "error"})
                fields = starts[0]["fields"]
                self.assertEqual(fields["backend_kind"], kind)
                self.assertEqual(fields["command"], str(guard))
                self.assertEqual(fields.get("args", ""), expected_args[name])
                self.assertEqual(fields["timeout_ms"], "300000")


if __name__ == "__main__":
    unittest.main(verbosity=2)
