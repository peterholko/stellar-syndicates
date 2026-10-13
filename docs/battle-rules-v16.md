# Battle rules v16: deterministic math

New battles open on rules v16 (`crates/sim/src/tactical/v16.rs`). Every sin,
cos, atan2 and powf the battle engine evaluates is now software
(`crates/sim/src/dmath.rs`, built on the `libm` crate, a port of musl's
libm). So a battle fights the same in a debug build and a release build, on
every platform, and an archived replay rebuilds after a server rebuild or an
OS update. Every rule is v15's. Only the last bits of a few calculations
differ, and that is enough to change how any given fight plays out.

## The problem

Before v16 the engine called the platform's libm through Rust's `f64`
methods, and the optimizer rewrites some of those calls in a release build
only:

- **Paired sin and cos.** On Apple platforms LLVM fuses a `sin` and a `cos`
  of the same angle into one `__sincos_stret` call. Measured on this Mac,
  the fused call disagrees with separate `sin` and `cos` in the last bit for
  278 of 200,000 angles. The engine pairs them for weaving, formations,
  close passes, tackle orbits, ring points and slow turns.
- **`powf(x, 0.5)`.** With a constant exponent of 0.5, LLVM computes `sqrt(x)`
  instead of calling `pow`. Apple's `pow(x, 0.5)` differs from `sqrt(x)` for
  2,862 of 2,000,000 inputs. Battery armour uses 0.5, and so does the
  torpedo hit chance wherever the compiler inlines it.

A debug build calls the library as written. Ten fights measured across
rules v1–v15 differed between debug and release in 87 of 150 runs, every
version from v3 up. A fight that differs in the last bit soon differs in
which hull dies. The visible symptoms:
- The battle fixtures generated in debug failed `cargo test --release`.
- Whether release code fuses a pair depends on the optimizer's inlining
  choices. So even a release-built server could rebuild an archived replay
  differently after an unrelated code change, a compiler update or an OS
  update to libm.

## How it works

`tactical::Math` says how a rules version computes these functions:
`TacticalState::math()`, from the battle's rules. Every call site in the
engine takes it:
- the hit chance and battery armour;
- body radius and slow turns;
- tie-breaks, weaving and formations;
- close passes, ring points and tackle orbits;
- the world's platforms.

- **`Math::Soft` (v16 and later)** calls `dmath`. Software math is ordinary
  arithmetic, which IEEE 754 rounds exactly and the optimizer may not
  reorder, so every build computes the same bits. `sqrt` and arithmetic need
  no wrapper.
- **`Math::Platform` (v1–v15)** keeps every original expression exactly as
  written, with a paired sin and cos still one expression, so the optimizer
  treats it as before. Their archives were recorded by a release server, and
  they still rebuild there bit for bit. Measured: all 150 runs of v1–v15
  give the same release-build fingerprint as before the change. Their debug
  builds still differ from release, as they always did.

## Tests

- `dmath::tests::the_same_bits_in_every_build`: 20,000 inputs through every
  function, hashed and pinned.
- `tactical::fingerprint::current_rules_fight_the_same_in_every_build`: ten
  fights pinned on the current rules (open space, a world with relief, a
  station, a withdrawal, a convoy of non-combat hulls). It passes in both
  `cargo test` and `cargo test --release`. If it fails in only one, a
  platform-math call crept back in. `print_rules_fingerprints` (ignored)
  prints every version's fingerprint for comparing builds.
- The replay tests and `battle_fixtures` now pass in release too.

When adding trig or powers to the battle engine, use the battle's `Math`,
never `f64::sin`, `cos`, `sin_cos`, `atan2` or `powf` directly.

## Test fights

The ten fixtures in `client/public/dev/battles` are regenerated on v16, in
either build:

```bash
BATTLE_FIXTURES_WRITE=1 cargo test -p server battle_fixtures
```
