# Beat-aware transitions: design spec

**Version:** 0.2, 15 Nov 2026

**Built against**

| App | Repo | Version | Commit |
|---|---|---|---|
| Android | `kushagrasinghx/BitChord` | v1.9.1 | `7f45c768a461fd7e68bf4613f71e8a767e643d4c` |
| PC | `jackychan4891/BitChord-PC-Continued` | v2.0.1 | `d7d71a128c53a2352be8ea081f9a0a423b96a9e0` |

This file is identical in both forks. Change it in both or in neither.

**Scope:** DJ-style transitions only. Karaoke and search are separate work.

**Status labels used below**

- **Reused**: existing code, unchanged.
- **Configured**: existing code, switched on by a new setting.
- **New**: code added by this project.

## 1. Principles

1. **Off means unchanged.** With "Beat-aware transitions" off, each app behaves exactly as the version it was built against.
2. **Fall back, don't fail.** Missing or weak evidence lowers the tier. It never causes an error, a gap or a stall.
3. **One rule set, two engines.** Android uses the existing planner. PC ports the same rules, thresholds and reason codes.
4. **The spec covers timing, not sound.** It fixes when to blend, where, for how long and when to fall back. Filters, EQ and effects are up to each platform.
5. **Equal-power gain curves** for every overlap on both platforms (Reused).
6. **Every feature sits behind a setting.**

## 2. Settings

| Setting | Default | Android | PC |
|---|---|---|---|
| Automix (master switch) | user's choice | `smartFadeEnabled` (Reused) | `automix` (Reused) |
| Crossfade length (fallback) | user's choice, 8 s if off | `crossfadeSeconds` (Reused) | `crossfade_secs`, 1-12 s (Reused) |
| Beat-aware transitions | off | New key | New field |
| - Phrase-aligned cues | on | `phraseBars` 8/16 (Configured) | New |
| - Tempo matching | on | `allowTempoStretch` (New parameter) | shown as unavailable in phase 1 |
| - Gapless within albums | on | `albumSequential` (Configured) | New (needs album IDs in the PC queue, to confirm) |

The 30-second mix window belongs to Beat-aware transitions and has no switch of its own.

## 3. Decision order

Rules run in this order. The first match wins and its reason code is logged.

| # | Rule | Result | Reason code | Android | PC |
|---|---|---|---|---|---|
| R0 | Automix off | Standard crossfade at the user's length, or gapless | `standard` | Reused | Reused |
| R1 | No duration; video row; Listen Together; bit-perfect output | No Automix | `no-duration` (the others are guards) | Reused | New (jam guard) |
| R2 | Outgoing track shorter than 45 s | No mix | `short-duration-guard` | Reused | New |
| R3 | Same album, played in order, setting on | Gapless (0.12 s) | `same-album-gapless` | Configured | New |
| R4 | Spoken word or live, judged from title, artist or album | Blocked | `blocked-speech-or-live` | Reused | New (port the text patterns) |
| R5 | Either track's analysis not ready | Plain crossfade at the user's length | `smart-analysis-fallback` | Reused | New |
| R6a | A tempo outside 40-220 BPM | Plain crossfade at the exit point | `outgoing-tempo`, `incoming-tempo` | Reused | New |
| R6b | Beat-aware on and EITHER track's beat confidence below 0.2 | Plain crossfade | `beat-confidence` | New (stricter than the existing both-below-0.2 test) | New |
| R6c | Rate needed is outside 0.9-1.25x, or a confidence is below 0.55 | Beat-aligned blend with no tempo change | `tempo-distance`, `beat-confidence` | Reused | New |
| R6d | Tempo matching switched off | Held at beat-aligned | `stretch-disabled` | New | New |
| R6e | Platform cannot change tempo | Held at beat-aligned | `no-stretcher` | n/a | New (PC phase 1) |
| R7 | Pair cannot be mixed | Equal-power fade at the end of content | `smart-unmixable-crossfade` | Reused | New |
| R8 | Phrase-aligned on and downbeats trusted | 16 bars if the outgoing track is 4 minutes or longer, otherwise 8 | `smart-phrase-8-bar`, `smart-phrase-16-bar` | Configured | New (needs downbeats, so PC phase 2) |
| R9 | Otherwise | Android: phrase switch, then the adaptive mix. PC: beat-aligned blend at the planned exit and entry points. | `smart-phrase-switch` and the adaptive mix's own codes | Reused | New (simpler) |

The existing Android check is: plain crossfade when BOTH tracks are below 0.2. R6b changes that to EITHER, and only while Beat-aware is on. With Beat-aware off the old test applies.

**Rules that apply after the tier is chosen**

- **Mix window:** the whole blend stays within the last 30 s of the outgoing track and the first 30 s of the incoming one. Android: Configured (`mixWindow`). PC: New (today PC allows up to 45 s).
- **Exit point:** the best-scoring candidate, never skipping more than 12 s of audible music. Android: Reused. PC: New.
- **Entry point:** after the incoming track's silent intro, never more than 25% into it. Android: Reused. PC: New (today PC starts at 0).
- **Blend lengths:** plain crossfade 1-12 s (user setting). Adaptive mix 4-16 beats, at least 4 s (6 s for fast tracks) and at most 16 s. Phrase-aligned 8 or 16 bars, inside the mix window.

## 4. Cancelling a blend, and the `isTransitioning()` rule

1. **Every blend runs through the engine's own state.** Android: `CrossfadeController.isTransitioning()`, true when `phase != IDLE`, so from arming until settled (Reused). PC: a new `AudioStatus` flag `transitioning`, true while a blend runs or a song is fading out (New).
2. **Nothing changes the queue mid-blend.** Code that replaces or reorders queue rows, swaps versions or upgrades quality must check `isTransitioning()` and wait while it is true. Android already does this for the music-only feature and for version swaps (Reused). PC needs the check for hi-res `Upgrade` and for jam queue edits (New; whether PC already guards these is to be confirmed).
3. **A queue change between blends cancels the planned blend.** The plan is worked out again on the next check. Android: Reused (the planner reruns about every 250 ms). PC: New.
4. **Skip or seek during a blend** ends it quickly and without a click: a fade of at least 40 ms, then the new song continues alone. PC: Reused (`QUICK_FADE_MS`). Android: Reused; the exact fade length is to be confirmed.
5. **Playback error or underrun during a blend** stops the blend and keeps the song that is furthest along. Android: Reused (`bail`). PC: to be confirmed.

## 5. Reason codes

The existing Android strings are the canonical list. PC must emit the same ones.

- Plan codes: `standard`, `no-duration`, `short-duration-guard`, `same-album-gapless`, `blocked-speech-or-live`, `smart-analysis-fallback`, `smart-plain-crossfade`, `smart-unmixable-crossfade`, `smart-phrase-8-bar`, `smart-phrase-16-bar`, `smart-phrase-switch`, plus the `before-*` forms used before a window opens.
- Tier codes: `outgoing-tempo`, `incoming-tempo`, `beat-confidence`, `tempo-distance`, `unmixable-pair`.
- New: `stretch-disabled`, `no-stretcher`.

## 6. Shared test files

See `docs/transitions-fixtures/README.md`. Both apps must pass the same files. Fixtures 001–008 record the unmodified Android v1.9.1 planner and prove that the default mapping is a no-op. Fixtures 009–011 cover one-sided weak confidence, no tempo matching and a 16-bar mix. Android `BeatAwareConfig` maps settings to planner arguments; PC will use the same mapping when its port is implemented.

### Android connection points (implemented)

- `shared/commonMain/.../playback/transitions/BeatAwareConfig.kt` is the isolated settings-to-planner mapping. It imports only `PHRASE_BARS_SHORT` and `PHRASE_BARS_LONG` from the existing planner and has no Android or Media3 dependency.
- `CrossfadeController.considerSmartTransition` is the sole planner call. It passes the config's phrase, album, mix-window and stretch options. `toTransitionInfo` also passes `MediaMetadata.albumTitle`, allowing the existing album comparison to work.
- `AppSettings` persists four switches: `beat_aware_transitions` (off by default), and phrase alignment, tempo matching and album gapless (all on by default underneath it). `SettingsSheet` exposes them beneath Automix.
- The existing renderer executes the resulting plan. There is no second playback engine, queue hook or analysis hook.

## 7. Allowed differences in phase 1

| Area | Android | PC phase 1 |
|---|---|---|
| Beat grid | Beat This! model, with downbeats | Own DSP: tempo, beat position and confidence, no downbeats, so R8 is skipped |
| Tempo stretch | Yes, incoming track only | No (held at beat-aligned, `no-stretcher`) |
| Vocal-clash and key checks | Yes | No |
| Sound effects | Filters, bass swap, echo | Existing hold-then-swap curve, beat repeat, flanger |

**PC phase 2:** run the Beat This! model through ONNX (the `ort` crate) so PC gets downbeats and can do R8. The weights and code are MIT-licensed.

## 8. Licensing

- `TransitionPlanner`, `TransitionPolicy`, `TrackAnalysis` and `BeatTracker` on Android are ported from Orchard and are AGPL-3.0-or-later. Their headers must stay.
- Any Rust port of those rules carries the same AGPL header and names the Kotlin file it follows.
- No API keys, add-on URLs, tokens or signing keys in either repo. They belong in `.gitignore` or GitHub secrets. CI signs nothing and uses no secrets.

## 9. Verification status

- The eight legacy fixtures passed before and after the planner change on the local JVM. All shared JVM tests passed after the change (131 tests in 20 suites).
- Confirmed against v1.9.1 source: `CrossfadeController` is the only app-module caller of `planTransition`; no other caller supplies `phraseBars` or `mixWindow`; the default fallback is 6 s.
- Android's no-stretch path is tested for both phrase and adaptive planning, so it cannot produce a rate other than 1.0. Octave-equivalent tempi still need no stretch.
- Still to confirm: the exact Android skip-mid-blend fade length; how PC's `Tempo::estimate` accepts a result; whether PC guards hi-res `Upgrade` during a blend, disables Automix during jams, and carries an album ID in queue items.

## 10. Changelog

- 0.2: implemented the isolated Android configuration mapping, its four persisted settings and the two planner options; added recorded legacy and beat-aware fixtures. 8 bars for tracks under four minutes, 16 bars otherwise.
- 0.1: first draft from the v1.9.1 and v2.0.1 audit.
