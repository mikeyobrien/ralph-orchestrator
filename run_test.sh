#!/bin/bash
export PYTHONPATH=$(pwd)/src
python3 -m hats_orchestrator -c test_hats.yml -i 50 --dry-run
