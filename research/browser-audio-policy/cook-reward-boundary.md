# Cook reward level audio is a committed-delta gate

The source reward remains **300 Cooking XP /3000 tenths**, claimed atomically
with completion and other rewards. Ordinary source cooking success gives
30 XP for shrimp and40 XP for bread; the compiled source threshold table is
read without edits. Extra legitimate cooking can therefore change the actual
resulting level:

| Source-derived boundary | Before XP / base level | After reward XP / base level |
| --- | --- | --- |
| One shrimp + one bread |70 /1 |370 /4 |
| The above + one extra shrimp |100 /2 |400 /5 |
| The above +171 extra shrimp |5200 /21 |5500 /21 |

These are **arithmetic/committed-projection fixtures**, not a newly played
journey, test-only XP grant, fabricated quest event or authorization to change
gameplay. Production audio consumes the already-committed skill/XP snapshots;
it does not calculate or award any XP.

`cook-reward-boundary.json` records exact source input hashes and four calls to
the **unchanged native opcode3202**. The native transport receives only an
index11 group and unused auxiliary integer: it has no quest or base-level
argument and no universal33/level4 condition. Both33 and34 are accepted as
supplied native groups. The probe does **not** establish a new per-quest
selector.

The original dated observation remains unchanged: normal Cook completion152,
reward level4 dialog/jingle33 after quest-scroll dismissal; later ordinary
Cooking level5/jingle34 is **not a simultaneous quest reward**. Those examples
do not constrain a player's pre-reward XP or every possible legitimate reward
level. The audio adapter preserves the actual source-selected group, rather
than choosing a new selector from these examples.

The implementation captures the completion transaction's before/after
`SkillView.baseLevel` and `xpTenths`, not temporary `currentLevel` boosts.
Cook-caused `level_up` requires `skillId`; optional declared levels/completion
ID are consistency checks. A positive committed delta is retained through the
source-scroll153 gate. Zero delta has no level cue. Stable event/cue and
completion/skill correlation prevent repeated callbacks from sounding it twice.
The same supplied completion can be preceded by its XP notification in one
immutable batch; staging that event does not create or replay a quest event.

Reproduce with:

```sh
python3 tools/browser-audio-tests/native/cook_reward_boundary.py
pnpm --dir web exec tsc --noEmit
node --test web/audio/reward-levels.test.ts
node tools/browser-audio-tests/run.mjs
```

All source files, frozen pack/approval bytes, UI, engine and XP rules remain
unchanged. Live-game/Mac/Edge/speaker/M1 acceptance is not claimed.
