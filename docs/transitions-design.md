# Beat-aware transitions: design spec

**Version:** 0.6, 17 Nov 2026

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
| Mix style: Smooth, Fade, Rise, Quick | Smooth | `automix_style`, `applyMixStyle` (New) | New, to do |
| Blend length: Short, Medium, Long | Medium | `automix_blend_length`, `applyMixStyle` (New) | New, to do |
| Mix on skip | off | `mix_on_skip`, `decideSkip` (New) | New, to do |

The 30-second mix window belongs to Beat-aware transitions and has no switch of its own.

Mix style, blend length and Mix on skip apply whenever Automix is on, with or without Beat-aware. Smooth and Medium change nothing, so the defaults keep principle 1.

**Mix style** only chooses among gestures the renderer already has, and only for a mix the planner chose (`DJ_BLEND`, `DJ_FILTER`). Blocked plans, gapless album handoffs and every plain-crossfade fallback (R0-R7) are never restyled.

| Style | Result |
|---|---|
| Smooth | The planner's own choice |
| Fade | Equal-power fade on the same timing: no filters, bass swap or echo |
| Rise | Filter sweep (`DJ_FILTER`, `SWEEP`) |
| Quick | At most 8 bars; bass swap; echo-out only on a beat-locked blend with room for five repeats after it |

**Blend length** scales the planned overlap by 0.5, 1 or 1.6. The end stays put and the start moves. On a trusted grid the result is whole bars, on four-bar lines from four bars up. It stays within 4-30 s and within the last 30 s of the outgoing track and the first 30 s of the incoming one. Long is never shorter than Medium, and Short never longer.

## 3. Decision order

Rules run in this order. The first match wins and its reason code is logged.

| # | Rule | Result | Reason code | Android | PC |
|---|---|---|---|---|---|
| R0 | Automix off | Standard crossfade at the user's length, or gapless | `standard` | Reused | Reused |
| R1 | No duration; video row; Listen Together; bit-perfect output | No Automix | `no-duration` (the others are guards) | Reused | New (jam guard) |
| R2 | Outgoing track shorter than 45 s | No mix | `short-duration-guard` | Reused | New |
| R3 | Same album, played in order (shuffle off), setting on | Gapless (0.12 s) | `same-album-gapless` | Configured | New |
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

**Same album (R3).** When both tracks carry an album ID, the IDs decide. Otherwise the album titles must match (trimmed, ignoring case) and the tracks must share at least one credited artist. Credits are split on `,` `&` `;` `/` `feat.` `ft.` `featuring` `with` `x`, so "Juelz" and "Juelz, Z3lla" match. R3 applies even when the analysis finds a mix-out point inside the track: an album played through is gapless as recorded. Before Beat-aware nothing asked for R3, so legacy plans are unaffected.

**Rules that apply after the tier is chosen**

- **Mix window:** the whole blend stays within the last 30 s of the outgoing track and the first 30 s of the incoming one. Android: Configured (`mixWindow`). PC: New (today PC allows up to 45 s).
- **Exit point:** the best-scoring candidate, never skipping more than 12 s of audible music. Android: Reused. PC: New.
- **Entry point:** after the incoming track's silent intro, never more than 25% into it. Android: Reused. PC: New (today PC starts at 0).
- **Blend lengths:** plain crossfade 1-12 s (user setting). Adaptive mix 4-16 beats, at least 4 s (6 s for fast tracks) and at most 16 s. Phrase-aligned 8 or 16 bars, inside the mix window.
- **Style and length:** applied last, to the plan the rules above produced (section 2).

## 4. Cancelling a blend, and the `isTransitioning()` rule

1. **Every blend runs through the engine's own state.** Android: `CrossfadeController.isTransitioning()`, true when `phase != IDLE`, so from arming until settled (Reused). PC: a new `AudioStatus` flag `transitioning`, true while a blend runs or a song is fading out (New).
2. **Nothing changes the queue mid-blend.** Code that replaces or reorders queue rows, swaps versions or upgrades quality must check `isTransitioning()` and wait while it is true. Android already does this for the music-only feature and for version swaps (Reused). PC needs the check for hi-res `Upgrade` and for jam queue edits (New; whether PC already guards these is to be confirmed).
3. **A queue change between blends cancels the planned blend.** The plan is worked out again on the next check. Android: Reused (the planner reruns about every 250 ms), plus New: `onQueueChanged()` on every playlist change drops the old pair's plans, cancels a blend still arming for it and asks for the new next song's analysis at once. PC: New.
4. **Skip or seek during any blend** ends it quickly and without a click: a fade of at least 40 ms, then the new song continues alone. This includes a Mix on skip blend, so skips never stack (4.7). PC: Reused (`QUICK_FADE_MS`). Android: Reused, 120 ms (`BAIL_MS`). Previous also ends a blend, and gives the song a fresh attempt (4.6).
5. **Playback error or underrun during a blend** stops the blend and keeps the song that is furthest along. Android: Reused (`bail`). PC: to be confirmed.
6. **A failed blend is not retried as it was.** A failure is any stop that is not the listener's own skip, seek, pause or queue edit. Each failure is recorded against its outgoing -> incoming pair for the session. After one failure the pair gets a plain crossfade only (no loop mix, no DJ blend); after two, nothing is armed and the song plays to its end. A failed loop mix never seeks backwards, and only seeks within the song its position belongs to. Previous clears the record for the songs involved. Android: New (`TransitionFailureLog`). PC: New, to do.
7. **Mix on skip.** With the setting on, a forward skip (next, swipe, headset, notification, car) may blend instead of cutting. `decideSkip` decides, in this order:
   - Cut (`skip-cut-off`): the setting or Automix is off.
   - Cut (`skip-cut-guard`): no next song, paused, Listen Together, bit-perfect output, a version swap, a video row, repeat-one, the sleep timer, casting, a pair that failed before, or under 5 s of the song left.
   - Cut (`skip-cut-busy`): any blend is armed or running. It is ended as in 4.4. If it was a skip blend still waiting, its own skip is done first, so two presses skip two songs.
   - Cut (`skip-cut-rapid`): another forward skip within 1.5 s.
   - Blend (`skip-blend`): 2, 3 or 4 s for Short, Medium or Long. It starts on the outgoing track's next beat when its grid is trusted (confidence at least 0.55, 40-220 BPM) and that beat is at most 0.5 s away; otherwise it starts at once. The incoming track starts after its silent intro, never more than 25 % in, or at 0:00 without analysis. Gesture: echo-out (`ECHO_RIDE`) for Smooth and Quick on a trusted beat with at least two beats of blend, a filter sweep for Rise or without a beat, an equal-power fade for Fade. No tempo stretch. The session moves to the new song at the start of the blend.
   - Fallback (`skip-fade`): when the incoming player is not ready within 600 ms, a 1 s equal-power fade with no beat wait, once it is ready. When it is not ready within 2.5 s, or playback is paused while waiting, the skip is done the ordinary way.
   - A skip blend is booked as a skip, not as a song that ended: it is not scrobbled as listened to and does not fire "sleep after this song".
   - Queue taps and previous never blend.

   Android: New (`decideSkip`, `CrossfadeController.onNextRequested`). PC: New, to do.

8. **Custom transitions (0.5).** A playlist with "Mix" on lets the listener hand-tune the transition between two songs. It runs after R0-R2 and the failure gate (4.6) and before R3-R9, and it never overrides a guard.
   - **When it applies:** both songs are context rows (not Play next) with the same playback source, that source is a playlist with Mix on, Automix is on, and a pair is saved for them (keyed `<playlist>|<outgoing id>-><incoming id>`; the `VL` prefix of a YouTube Music browse id is ignored). Any other pairing, including the same two songs reached from another playlist or by Play next, gets the automatic plan. Listen Together, bit-perfect output, casting and video rows stay as before, so custom never applies there.
   - **What is saved:** mode (mix, gapless or no mix); for mix: style, length in bars (or seconds when there is no trusted grid), `beatSeconds` (the beat length the editor measured, so the played length equals the drawn one), the start as seconds before the outgoing song's end, bass swap, an exit filter (none, low-pass, high-pass) with an amount, and the two song durations it was made for. No EQ in 0.5.
   - **Gapless** is a plain handoff (`custom-gapless`, 120 ms). **No mix** (`custom-no-mix`) arms nothing and the queue advances plainly. Neither is blocked by the failure record. A mix pair is, like any blend.
   - **Fallbacks:** `custom-fallback-duration` (either song's length differs from the saved one by more than 2 s), `custom-fallback-window` (the blend would leave the last 45 s of the outgoing song or the first 45 s of the incoming one, or pass the end of the content), `custom-fallback-late` (the playhead is already past the start). Each returns the automatic plan, so there is never a gap or a loop.
   - **Planner decisions kept:** the tempo stretch rate and the incoming cue. Mix on skip ignores custom pairs. A custom Mix pair uses the loop mix only when it asks for it (4.8.1).
   - **Saving or editing a pair clears that pair's failure record**, so the new transition applies at once, Preview included.
   - **Window loader (the editor's waveforms and beat grid).** The editor needs only the last 45 s of one song and the first 45 s of the other, never whole songs. Order: saved windows (`MixWindowStore`, per song) then the playback cache, then a fetch of only the needed byte ranges into the playback cache (the container header plus the window, at most 3 MB per window), then a wait for Wi-Fi when "Wi-Fi only downloads" is on and the connection is metered. Only when the fetch fails does the editor offer to download the two songs. Android's Matroska reader cannot seek over bytes that were not read, so a fetched tail is decoded as a small standalone WebM built from the header and the tail's whole clusters (`MatroskaWindow`). That is WebM (Opus) only; other containers fall back to the download offer.
   - **Preview** plays a two-song queue of exactly the pair, stamped with the playlist as its source, from 8 s before the blend, using the unsaved draft as a one-shot override.
   - **Backup:** the in-app backup file carries an optional `customTransitions` section (Mix flags and pairs; schema stays 1). Import replaces the store when the section is present and leaves it alone when absent.
   - **4.8.1 Custom loop mix (0.6).** A Mix pair may ask for the loop mix (the automatic DJ transition that loops an instrumental stretch of each song and moves the tempo from one to the other). It saves `loop`, `loopBars` (1, 2 or 4: the loop on both songs) and `loopRamp`; style, length, bass swap and filter then do not apply. The frame marks about where the outgoing loop starts (`startBeforeEndSeconds`).
     - **Planned at play time, by the controller,** because choosing a loop that is truly instrumental needs the vocal mask and the lyrics: the instrumental loop of the chosen length nearest the saved start (within 2 bars either side) on the outgoing song, and the earliest one in the incoming song's opening. The same safety rules as the automatic loop mix apply (video rows, playback speed, sleep timer, skip silence, outputs that cannot take it, analysed off another recording, waiting on analysis or lyrics).
     - **Tempo:** ramp on is the automatic behaviour (at most 2.5 BPM per bar). Ramp off holds the outgoing tempo until the incoming loop lets go, then steps to its own; it is only allowed when the two songs are within 8 BPM (after folding to double or half time), otherwise the ramp stays on and the editor says so.
     - **No end-of-file rule for the handover.** The controller holds the outgoing player's end for the whole mix and the sink keeps generating the mix past the last sample, so a loop whose handover falls after the outgoing file's end is fine: on the phone Flamingo -> i don't belong here (loop at 173.3 s of 197.5 s, 89 -> 120 BPM) handed over 2.6 s after the file ended and finished cleanly. A rule that refused such loops was tried in 0.6 on a wrong reading of an earlier mid-mix flush and removed; those flushes came about 20.5 s after takeover with the end held and 24 s of file left, and are not explained by the file's end.
     - **Fallback `custom-fallback-loop`:** no loop of that length near the start, none that hands over inside the song, an output that cannot take it, or no analysis. The pair then takes the whole automatic plan, loop mix included. While analysis or lyrics are still on their way and the start is more than 12 s off, the pair waits instead of arming a blend. A failed loop mix goes in the failure record like any blend.
     - **Needs the whole-track analysis** of both songs (vocal mask and levels), which comes from playing a song through or downloading it. Analyse playlist only gets tempo and key from a song's opening, so it does not enable it; the editor then says to play both songs through once.
     - **Preview** of a loop mix starts 25 s before the start (the decks take about 1.5 s to build and the lyrics a few seconds to arrive).
   - **Limits:** pairs are keyed by song id, so a song swapped to another version (for example Prefer music-only) gets the automatic plan. With the app's shuffle on, saved pairs apply only when the two songs really are adjacent. Local playlists themselves are not in the in-app backup (existing gap). English strings only. Android: New. PC: not done.

## 5. Reason codes

The existing Android strings are the canonical list. PC must emit the same ones.

- Plan codes: `standard`, `no-duration`, `short-duration-guard`, `same-album-gapless`, `blocked-speech-or-live`, `smart-analysis-fallback`, `smart-plain-crossfade`, `smart-unmixable-crossfade`, `smart-phrase-8-bar`, `smart-phrase-16-bar`, `smart-phrase-switch`, plus the `before-*` forms used before a window opens.
- Tier codes: `outgoing-tempo`, `incoming-tempo`, `beat-confidence`, `tempo-distance`, `unmixable-pair`.
- New: `stretch-disabled`, `no-stretcher`.
- New in 0.4, policy reasons added by style and length: `style-fade`, `style-rise`, `style-quick`, `length-short`, `length-long`.
- New in 0.4, skip codes: `skip-blend`, `skip-fade`, `skip-cut-off`, `skip-cut-guard`, `skip-cut-busy`, `skip-cut-rapid`.
- New in 0.5, custom transitions (4.8): `custom` (shown as `before-custom` before the window opens), `custom-gapless`, `custom-no-mix`, `custom-fallback-duration`, `custom-fallback-window`, `custom-fallback-late`.
- New in 0.6: `custom-loop` (the pair asks for the loop mix; the controller plans it) and `custom-fallback-loop`.

## 6. Shared test files

See `docs/transitions-fixtures/README.md`. Both apps must pass the same files. Fixtures 001–008 record the unmodified Android v1.9.1 planner and prove that the default mapping is a no-op. Fixtures 009–011 cover one-sided weak confidence, no tempo matching and a 16-bar mix. Fixtures 012–013 cover the album match and 014–018 the style and length (`androidOnly` until the PC port catches up). Two optional fixture settings were added in 0.4, `mixStyle` and `blendLength`. Android `BeatAwareConfig` maps settings to planner arguments; PC will use the same mapping when its port is implemented. Mix on skip is covered by unit tests (`SkipBlendTest`), not fixtures. Fixtures 019-023 (0.5, `androidOnly`) cover custom transitions through an optional `custom` block (mode, style, bars or seconds, `beatSeconds`, start before the end, filter, the durations it was made for).

### Android connection points (implemented)

- `shared/commonMain/.../playback/transitions/BeatAwareConfig.kt` is the isolated settings-to-planner mapping. It imports only `PHRASE_BARS_SHORT` and `PHRASE_BARS_LONG` from the existing planner and has no Android or Media3 dependency.
- `CrossfadeController.considerSmartTransition` is the sole planner call. It passes the config's phrase, album, mix-window and stretch options. `toTransitionInfo` also passes `MediaMetadata.albumTitle`, allowing the existing album comparison to work.
- `AppSettings` persists four switches: `beat_aware_transitions` (off by default), and phrase alignment, tempo matching and album gapless (all on by default underneath it). `SettingsSheet` exposes them beneath Automix.
- The existing renderer executes the resulting plan. There is no second playback engine or analysis hook.
- 0.4: `TransitionFailureLog`, `MixStyle`/`applyMixStyle` and `SkipBlend`/`decideSkip` are pure shared code. The app adds the queue hook (`onQueueChanged`), the previous hook (`onPreviousRequested`), the skip entry point (`onNextRequested`) and a `skipped` flag on the handoff. With Beat-aware on, a plan still waiting on analysis shows a dimmed, provisional window on the scrubber.

## 7. Allowed differences in phase 1

| Area | Android | PC phase 1 |
|---|---|---|
| Beat grid | Beat This! model, with downbeats | Own DSP: tempo, beat position and confidence, no downbeats, so R8 is skipped |
| Tempo stretch | Yes, incoming track only | No (held at beat-aligned, `no-stretcher`) |
| Vocal-clash and key checks | Yes | No |
| Sound effects | Filters, bass swap, echo | Existing hold-then-swap curve, beat repeat, flanger |

**PC phase 2:** run the Beat This! model through ONNX (the `ort` crate) so PC gets downbeats and can do R8. The weights and code are MIT-licensed.

### 7.1 What PC does today (v0.3)

Honest status, so the two apps are not assumed to behave the same:

| Rule | Android | PC now |
|---|---|---|
| Planner rules R0-R9 and their reason codes | Used for playback | **Ported and tested against all 11 shared fixtures** (`src/transitions/`), but not yet used to choose where a blend starts |
| R6a tempo outside 40-220 BPM | Plain crossfade | **Live:** the normal crossfade length is used |
| R6b weak beat confidence (below 0.2) | Plain crossfade | **Live:** the normal crossfade length is used |
| 30-second mix window | Applied to the plan | **Live:** a blend never exceeds 30 s |
| `isTransitioning()` | `CrossfadeController.isTransitioning()` | **Live:** `AudioStatus::transitioning()`; a hi-res upgrade is skipped mid-blend |
| Phrase-aligned starts (R8), tempo matching, album gapless (R3), speech/live (R4), short tracks (R2) | Used for playback | **Not live.** Implemented in the planner and covered by fixtures, but playback still starts the blend by timing alone |

Setting: **Beat-aware transitions** (Settings, under Automix, off by default). Off is exactly the previous behaviour, and there is a test for it.

Confidence: PC's live tempo meter reports 0..1 with the same formula as `transitions::analysis::grid_confidence`. It is a loudness-autocorrelation heuristic, not the Beat This! model, so its values are not directly comparable with Android's.

## 8. Licensing

- `TransitionPlanner`, `TransitionPolicy`, `TrackAnalysis` and `BeatTracker` on Android are ported from Orchard and are AGPL-3.0-or-later. Their headers must stay.
- Any Rust port of those rules carries the same AGPL header and names the Kotlin file it follows.
- No API keys, add-on URLs, tokens or signing keys in either repo. They belong in `.gitignore` or GitHub secrets. CI signs nothing and uses no secrets.

## 9. Verification status

- The eight legacy fixtures passed before and after the planner change on the local JVM. All shared JVM tests passed after the change (131 tests in 20 suites).
- Confirmed against v1.9.1 source: `CrossfadeController` is the only app-module caller of `planTransition`; no other caller supplies `phraseBars` or `mixWindow`; the default fallback is 6 s.
- Android's no-stretch path is tested for both phrase and adaptive planning, so it cannot produce a rate other than 1.0. Octave-equivalent tempi still need no stretch.
- Android skip-mid-blend fade: 120 ms (`BAIL_MS`), confirmed in 0.4.
- 0.4 on device (Android): the failure record stops the end-of-song loop; Play next is mixed; album gapless and Mix on skip await a device test.
- 0.5 on device (Android, over adb): the custom editor lays out and saves; Rise + High-pass, Gapless and No mix play as `custom`, `custom-gapless` (no gap, next song at 0 ms) and `custom-no-mix`; a saved pair plays as `custom` in normal playback; fetched head and tail windows decode (tail via the WebM stitch). Not checkable from logs: the volume and filter ramps, which need listening.
- Still to confirm: how PC's `Tempo::estimate` accepts a result; whether PC guards hi-res `Upgrade` during a blend, disables Automix during jams, and carries an album ID in queue items.

## 10. Changelog

- 0.6: Android only so far (PC to do). 4.8.1 custom loop mix (loop on/off, 1/2/4-bar loops, tempo ramp on/off). Codes `custom-loop`, `custom-fallback-loop`.

- 0.5: Android only so far (PC to do). New 4.8 custom transitions per playlist (editor, Preview, window loader, backup), their reason codes and fixtures 019-023 (`androidOnly`).

- 0.4: Android only so far (PC to do).
  - New settings: Mix style, Blend length and Mix on skip (section 2, 4.7).
  - R3: album match by ID, or by title plus a shared artist; no longer skipped for an interior mix-out; shuffle off.
  - 4.3: queue hook. 4.4: skip during any blend cuts, 120 ms on Android. 4.6: failed blends are not retried as they were.
  - New fixtures 012-018 (`androidOnly`) and fixture settings `mixStyle`, `blendLength`.

- 0.3: PC Rust port of the policy and planner passes the shared fixtures; PC engine uses the confidence floor, tempo range, 30 s window and `transitioning()` guard behind its Beat-aware setting. Section 7.1 lists what PC does and does not do yet.
- 0.2: implemented the isolated Android configuration mapping, its four persisted settings and the two planner options; added recorded legacy and beat-aware fixtures. 8 bars for tracks under four minutes, 16 bars otherwise.
- 0.1: first draft from the v1.9.1 and v2.0.1 audit.
