# Transition test files

Shared by the Android and PC forks. This file is identical in both.

The first eight fixtures record the existing Android v1.9.1 planner's real output, before any planner change. The next fixtures record beat-aware behaviour. Run the Android recorder, review the result and only then commit changes to a fixture:

```sh
TRANSITIONS_RECORD=1 ./gradlew :shared:jvmTest
```

`shared/src/jvmTest/.../TransitionFixturesTest.kt` writes the files. `TRANSITIONS_FIXTURES_DIR` overrides their directory. Recording is never enabled in CI. The runner lives in `shared/src/jvmTest/kotlin/com/music/bitchord/playback/smart/` of the Android fork.

## Format

One JSON file per case, `NNN-short-name.json`:

```json
{
  "name": "Two confident tracks at the same tempo",
  "settings": {
    "automix": true,
    "beatAware": false,
    "phraseAligned": false,
    "tempoMatching": true,
    "albumGapless": false,
    "fallbackFadeSeconds": 8.0
  },
  "outgoing": {
    "id": "a",
    "durationMs": 200000,
    "title": "",
    "artist": "",
    "album": "",
    "albumId": "",
    "analysis": { "duration": 200.0, "bpm": 120.0 }
  },
  "incoming": { "id": "b", "durationMs": 200000 },
  "positionSeconds": 10.0,
  "expected": {
    "reason": "before-smart-duration",
    "transitionStyle": "DJ_BLEND",
    "transitionStartSeconds": 180.5,
    "fadeSeconds": 12.0,
    "incomingPlaybackRate": 1.0,
    "outgoingPlaybackRate": 1.0
  }
}
```

Defaults are omitted from the JSON: `beatConfidence` 0.9, `keyConfidence` 0.0, `firstBeat` 0.5, `status` `"ready"`, `vocal` 0.0, and `contentEndTime` = `duration - 2`. `transitionStyle` defaults to `EQUAL_POWER`, playback rates to 1.0, arrays to empty and `blocked` to false.

`analysis` is expanded to the same steady synthetic grid as `PhraseBarAutomixTest`: one downbeat every four beats, phrase boundaries every eighth downbeat, the fifth downbeat as the mix-in point, and unit energy samples every half second. BPM can be zero for a no-analysis case. Times may differ by up to 50 ms; the plan and tier reasons must match exactly.

## Rules

- Use made-up track titles and ids. Never put real account data, tokens or addon URLs in a fixture.
- The absolute source of the expected result is planner output, not a hand calculation.
- Legacy fixtures must keep passing with beat-aware mode off. They are the proof that existing behaviour has not changed.
- A fixture is only added once both apps can run it, or it is marked `"androidOnly": true` until the PC port catches up.
