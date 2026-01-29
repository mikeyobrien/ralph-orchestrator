# Contributing to Content Ideation

## Adding New Avatars

1. Create avatar file: `.ideation/avatars/your-avatar.yaml`
2. Follow schema in `avatars/README.md`
3. Test with: `./bin/ideate setup your-avatar late-night-techno`
4. Commit: `git add .ideation/avatars/your-avatar.yaml`

## Adding New Templates

1. Create template: `.ideation/templates/your-template.md`
2. Include theme, focus areas, constraints
3. Test with: `./bin/ideate setup myla your-template`
4. Update `templates/README.md` with template description
5. Commit both files

## Modifying the Preset

Edit: `presets/content-ideation.yml`

**Adding a new creator hat:**
```yaml
hats:
  your_creator:
    name: "Your Creator Name"
    triggers: ["ideate.create"]
    publishes: ["ideas.generated"]
    instructions: |
      [Your instructions]
```

**Adding a new reviewer hat:**
- Insert between existing reviewers and completion_checker
- Trigger on previous reviewer's event
- Publish next event in chain

**Modifying scoring:**
- Edit reviewer hat instructions
- Adjust scoring criteria (1-10 scale)
- Update completion threshold in completion_checker

## Testing Changes

```bash
# Validate YAML
yq eval . presets/content-ideation.yml

# Test with example inputs
./bin/ideate setup myla late-night-techno
./bin/ideate run

# Check diagnostics
RALPH_DIAGNOSTICS=1 ./bin/ideate run
```

## Sharing Patterns

Found a pattern that improves ideas?

```bash
ralph tools memory add "pattern: [your pattern]" -t pattern --tags ideation
```

Share via PR to update preset instructions.
