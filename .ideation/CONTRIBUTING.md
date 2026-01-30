# Contributing to Content Ideation

## Adding New Avatars

1. Create avatar file: `.ideation/templates/your-avatar.yaml`
2. Follow schema in `templates/avatar/avatar-schema.md`
3. Test with: `./.ideation/ideate setup your-avatar late-night-techno`
4. Commit: `git add .ideation/templates/your-avatar.yaml`

## Adding New Templates

1. Create template: `.ideation/templates/your-template.md`
2. Include theme, focus areas, constraints
3. Test with: `./.ideation/ideate setup myla your-template`
4. Update `templates/README.md` with template description
5. Commit both files

## Modifying the Preset

Edit: `.ideation/preset.yml`

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
yq eval . .ideation/preset.yml

# Test with example inputs
./.ideation/ideate setup myla late-night-techno
./.ideation/ideate run

# Check diagnostics
RALPH_DIAGNOSTICS=1 ./.ideation/ideate run
```

## Sharing Patterns

Found a pattern that improves ideas?

```bash
ralph tools memory add "pattern: [your pattern]" -t pattern --tags ideation
```

Share via PR to update preset instructions.
