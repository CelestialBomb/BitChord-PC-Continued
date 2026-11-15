# Transition test files

Shared by the Android and PC forks. This file is identical in both.

There are no fixtures yet. The first ones will be recorded from the existing Android planner's real output, before any planner change.

## Format

One JSON file per case, `NNN-short-name.json`:

```json
{
  "name": "two confident tracks, close tempo",
  "settings": {
    "automix": true,
    "beatAware": true,
    "phraseAligned": true,
    "tempoMatching": true,
    "albumGapless": true,
    "fallbackFadeSeconds": 8
  },
  "outgoing": { "id": "a", "durationSeconds": 215.0, "title": "", "artist": "", "album": "", "analysis": {} },
  "incoming": { "id": "b", "durationSeconds": 198.0, "title": "", "artist": "", "album": "", "analysis": {} },
  "positionSeconds": 0.0,
  "expected": {
    "reason": "smart-phrase-8-bar",
    "transitionStartSeconds": 0.0,
    "fadeSeconds": 0.0,
    "incomingCueSeconds": 0.0,
    "incomingPlaybackRate": 1.0
  }
}
```

`analysis` holds the subset of fields the planner reads: `bpm`, `beatConfidence`, `downbeats`, `phraseBoundaries`, `contentEndTime`, `energyCurve`, `mixInCandidates` and `mixOutCandidates`.

Times may differ by up to 50 ms. `reason` must match exactly.

## Rules

- Use made-up track titles and ids. Never put real account data, tokens or addon URLs in a fixture.
- A fixture is only added once both apps can run it, or it is marked `"androidOnly": true` until the PC port catches up.
