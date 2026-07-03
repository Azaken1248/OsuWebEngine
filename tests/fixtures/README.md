# Test Fixtures

This directory contains `.osu` beatmap files and `.osr` replay files
used for unit tests, integration tests, and differential testing.

## Directory Structure

```
fixtures/
├── beatmaps/       .osu beatmap files
├── replays/        .osr replay files matching the beatmaps
└── README.md       this file
```

## Adding Fixtures

1. Place `.osu` file in `beatmaps/`
2. Place matching `.osr` file in `replays/`
3. Add a corresponding test case in the relevant module
4. Commit the files (they are small enough for git)

## Fixture Selection Criteria

Fixtures should cover:
- Simple circles-only maps (baseline)
- Slider-heavy maps (curve math validation)
- Spinner test maps
- Stacking edge cases (overlapping objects, combo resets)
- Marathon-length maps (performance testing)
- Edge cases: extreme AR/CS/OD, 2B-style overlaps, snake sliders

## Note

All fixtures must be from publicly available maps or be
purpose-created test maps. Do not commit copyrighted content.
