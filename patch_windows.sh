#!/bin/bash

# 1. Update Claude Integration tests: Remove outdated skips so tests actually run
echo "Updating Claude integration tests..."
rg -l "Claude adapter uses SDK, not subprocess" /mnt/c/Users/tuant/ralph-orchestrator/tests/test_integration.py | xargs -r sed -i 's/@unittest.skip("Claude adapter uses SDK, not subprocess - test outdated")//g'

# 2. Fix the fcntl patches to use our cross-platform variable MOCK_FCNTL_PATH
# This replaces: with patch('fcntl.fcntl') with: with patch(MOCK_FCNTL_PATH)
echo "Fixing fcntl patches..."
rg -l "patch('fcntl.fcntl')" /mnt/c/Users/tuant/ralph-orchestrator/tests/ | xargs -r sed -i "s/patch('fcntl.fcntl')/patch(MOCK_FCNTL_PATH)/g"

# 3. Fix encoding issues in context.py (Path.read_text)
echo "Enforcing UTF-8 encoding in context.py..."
sed -i 's/\.read_text()/\.read_text(encoding="utf-8")/g' /mnt/c/Users/tuant/ralph-orchestrator/src/ralph_orchestrator/context.py

# 4. Add @pytest.mark.asyncio to the checkpoint test if not present
echo "Adding asyncio support to checkpoint tests..."
sed -i '/def test_orchestrator_checkpoint_creation/i \    @pytest.mark.asyncio' /mnt/c/Users/tuant/ralph-orchestrator/tests/test_integration.py

echo "Patching complete. Checking for remaining fcntl strings..."
rg "fcntl\.fcntl" --glob "!*.pyc"
