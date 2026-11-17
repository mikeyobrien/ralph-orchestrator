# Hat collections

Hat collections lets a person create a visual hat workflow, save it, start from a blueprint, or import YAML, then see that collection in the list.

## Sub-features

- `collections-empty` shows the builder with no saved collections.
- `collections-create` saves a named collection from New Collection.
- `collections-blueprint` creates the Code Assist blueprint.
- `collections-import` imports a small hat YAML.
- `collections-return` shows the saved name back on the list.

## How to get to it (user POV)

- Choose **Builder** in the sidebar.
- Choose **New Collection** or **Create Your First Collection**.
- Choose the **Code Assist** blueprint card.
- Choose **Import YAML**, paste hat YAML, and choose **Import**.

## Driving it with verify-ralph

Preconditions:

- `verify-ralph doctor` passed for this run.
- `$DATA_DIR/.ralph/api/collections-v1.json` has no collection named `Release flow`, `Code Assist`, or `Imported flow`.

- **Empty list.** Open Builder. Run `verify-ralph browser click --role link --name "Builder"` and `verify-ralph browser text --contains "No collections yet"`. The heading is `Hat Builder`. **Import YAML** and **New Collection** are visible. Blueprint cards include `Code Assist`, `Debug`, and `Research`.
- **New collection.** Choose **New Collection**. Run `verify-ralph browser click --role button --name "New Collection"`. The heading reads `New Collection` and the name field says `New Collection`.
- **Rename and save.** Replace the name and choose **Save**. Run `verify-ralph browser fill --placeholder "Collection name" --value "Release flow"` and `verify-ralph browser click --role button --name "Save"`. The page shows `Saved` and the heading reads `Release flow`.
- **Back to the list.** Choose **Back**. Run `verify-ralph browser click --role button --name "Back"`. The list contains `Release flow`.
- **Blueprint.** Choose the **Code Assist** card. Run `verify-ralph browser click --text "Code Assist"`. The heading reads `Code Assist` and the page shows `Saved`. The hat palette shows `Hat Palette`.
- **Import.** Return to the list, open **Import YAML**, and import a one-hat file. Run `verify-ralph browser click --role button --name "Back"`, `verify-ralph browser click --role button --name "Import YAML"`, and `verify-ralph browser text --contains "Import YAML"`. Write this file to `$EVIDENCE_DIR/import.yml`:

```yaml
hats:
  planner:
    name: Planner
    triggers: [work.start]
    publishes: [build.task]
```

  Then run `verify-ralph browser fill --label "Name" --value "Imported flow"` and `verify-ralph browser fill --placeholder "hats:" --value-file "$EVIDENCE_DIR/import.yml"` and `verify-ralph browser click --role button --name "Import"`. The editor heading reads `Imported flow`.
- **Close without importing.** Open **Import YAML** again and choose **Close**. Run `verify-ralph browser click --role button --name "Close"`. The dialog title `Import YAML` is gone.
- **Proof.** From the list, capture the named cards. Run `verify-ralph browser screenshot --path "$EVIDENCE_DIR/collections-list.png"` and `verify-ralph browser snapshot --path "$EVIDENCE_DIR/collections-list.aria.txt"`. Copy `$DATA_DIR/.ralph/api/collections-v1.json` to `$EVIDENCE_DIR/collections-v1.json`. It contains `Release flow`, `Code Assist`, and `Imported flow`.

## Gotchas

- **Run** starts a real `ralph run` with the collection. Do not choose it. A prompt dialog is not a successful verification of Run.
- **Save** is disabled until the name is non-empty. The blueprint path saves on click, so `Saved` can appear without a separate Save click.
- `Saved` fades after a few seconds. Read the list and the JSON file even if the label is gone.
- The blueprint cards and the collection cards are clickable text, not buttons. Target them with `--text`.
- The pencil and trash icons on a collection card have no accessible name. Do not click them by coordinates. Deleting is unverified until those buttons have names. A delete click also opens `window.confirm`. The harness dismisses that dialog unless you first run `verify-ralph browser dialog --action accept`.
- Dragging a hat from **Hat Palette** has no button handle. Use a blueprint or import to put a hat on the canvas.
- Import from the list, where **Collection name** belongs to the dialog. On the editor, the same placeholder is the collection name field. Use `--label "Name"` inside the dialog.
