#!/usr/bin/env bash
export PYTHONPATH=$(pwd)/src
python3 -m ralph_orchestrator -c test_ralph.yml -i 50 --dry-run
