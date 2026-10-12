# Settings

Settings lets a person read the workspace `ralph.yml` and save an edit back to that file.

## Sub-features

- `settings-load` shows the seeded config in the editor.
- `settings-edit` marks the editor dirty when the text changes.
- `settings-save` writes the editor back to `ralph.yml`.
- `settings-reset` restores the last loaded text before a save.

## How to get to it (user POV)

- Choose **Settings** in the sidebar.
- Edit the configuration text and choose **Save**.
- Choose **Reset** to discard unsaved edits.

## Driving it with verify-ralph

Preconditions:

- `verify-ralph doctor` passed for this run.
- `$DATA_DIR/ralph.yml` exists and contains `backend: "claude"`.
- The editor does not already contain the comment `# verify-ralph settings`.

- **Open.** Choose **Settings**. Run `verify-ralph browser click --role link --name "Settings"`. The heading is `Settings` and a badge reads `ralph.yml`.
- **Loaded text.** Read the editor. Run `verify-ralph browser value --placeholder "# Ralph configuration"`. The printed YAML contains `backend: "claude"`.
- **Dirty edit.** Append a comment and put the whole document back. Run `verify-ralph browser fill --placeholder "# Ralph configuration" --value-file "$EVIDENCE_DIR/settings.yml"`, where that file is the previous editor text plus a final line `# verify-ralph settings`. The page shows `Unsaved changes`. **Save** is enabled.
- **Save.** Choose **Save**. Run `verify-ralph browser click --role button --name "Save"`. The page shows `Saved`.
- **File.** Read `$DATA_DIR/ralph.yml`. It contains `# verify-ralph settings` and still contains `backend: "claude"`.
- **Reset.** Add the line `# discard me` without saving, then choose **Reset**. Run `verify-ralph browser fill --placeholder "# Ralph configuration" --value-file "$EVIDENCE_DIR/settings-dirty.yml"` and `verify-ralph browser click --role button --name "Reset"`. The editor no longer contains `# discard me`, and `$DATA_DIR/ralph.yml` does not contain it either.
- **Proof.** Capture the dirty editor and the saved editor. Run `verify-ralph browser screenshot --path "$EVIDENCE_DIR/settings-dirty.png"` and `verify-ralph browser screenshot --path "$EVIDENCE_DIR/settings-saved.png"` and `verify-ralph browser snapshot --path "$EVIDENCE_DIR/settings-saved.aria.txt"`. Copy the saved file to `$EVIDENCE_DIR/ralph.yml`.

## Gotchas

- `fill` replaces the whole editor. Write the existing YAML back with the new line. Filling only the comment erases the config.
- The green **Saved** label clears after a few seconds. The file is the proof.
- **Active Collection** is a select whose only useful options come from hat presets in the workspace. The seeded `ralph.yml` has no `hats:` key, and the disposable workspace has no `presets/` directory, so the select can be empty. That is not a failed page load. The label under it says the dropdown is read-only.
- Invalid YAML stays unsaved and shows `Error saving`. A save that shows that error did not update `ralph.yml`.
