# Avatar Library

Avatar profiles define the personality, expertise, and voice of content creators.

## Schema

```yaml
name: string                     # Avatar name
personality: string              # Voice, tone, character description
expertise: [string]              # Areas of knowledge
audience_relationship: string    # How avatar relates to audience
constraints: string              # Things to avoid (optional)
examples: [string]               # Reference content URLs (optional)
```

## Usage

```bash
cp .ideation/avatars/myla.yaml .ideation/input/avatar.yaml
```

## Available Avatars

- **myla.yaml** - Electronic music curator, melodic techno expert
