# M1 source content builder

See [`content/m1/README.md`](../../content/m1/README.md) for the product data,
counts, validation commands and explicit readiness limits. This directory owns
the deterministic importer/binder/generator, not shared types or game mechanics.

| Tool | Purpose |
| --- | --- |
| `build.py` | Generate actual `GameContent`, 61 full geometry shards and hash-bound source bindings |
| `check.py` | ID, reference, note, initial-state, exact geometry and graph-preservation checks |
| `test_content.py` | Independent source arithmetic, masks, variants, grants/quests and negative assertions |
| `verify_routes.py` | Qualified 30-segment source-layout connectivity; never opens runtime doors |
| `compile.py` | Run the real `clubscape-content` Runtime compiler and persist its exact outcome |
| `schema-check/` | Deserialize through the actual shared Rust `GameContent`, without pretending to compile it |
| `import_references.py` | Reproduce bounded pinned wiki identity/coordinate revisions |
| `bind_wiki.py` | Extract factual source location rows, separate map anchors and 15 real shop stock lines |
| `import_code_references.py` | Pin source interface IDs and inspectable clipping/decoder references |
| `import_definitions.py`, `DefinitionSupplement.java` | Decode only missing item identities/links from existing original bytes |

The ordinary build needs Python 3.12 and **no downloads**. Shared-type and real
compiler checks need the existing pinned Rust toolchain. All scratch/output
files stay under an owned project directory; no temporary-system directory,
global installation or graphics change is required.

## Optional source reproduction

These are input-reproduction commands, not another freshness search or full
cache extraction:

```sh
python3 tools/m1-content/import_references.py
python3 tools/m1-content/bind_wiki.py
python3 tools/m1-content/import_code_references.py
```

`--discover` is only for a deliberate first acquisition of the small named
missing-reference list. Ordinary replay uses exact revisions and checks the
committed hashes. Raw wiki prose stays in the owned ignored `.local/wiki`;
committed facts retain their revision/row hashes and attribution.

The source worker's original request omitted required items such as hammer,
water, dough, burnt food, shield and ordinary shop stock. The supplement closes
those **definitions**, not their missing render-asset conversions:

```sh
python3 tools/m1-content/import_definitions.py \
  --cache-dir /path/to/existing/current-source/cache-2695 \
  --tooling-dir /path/to/existing/current-source/tooling \
  --npc-catalogue /path/to/existing/current-source/dependency-scan/npc-catalogue.json \
  --java-home /path/to/verified/jdk-17
```

On the recorded development worktrees, the read-only inputs are under
`.worktrees/m1-runtime-inputs/.local/current-source/`. Read and verify the machine
setup before choosing any host-specific JDK path. The importer checks every
decoder-library hash against `tools/cache-import/dependencies.json`, reads JS5
disk sectors using `rb` only, verifies the actual selected group SHA/CRC/revision,
and never instantiates a writable cache `Store`. It does not download a cache,
regenerate models, modify assets or overwrite source-worker outputs.

`definitions.json.gz` contains 146 unmodified selected item/mapping definitions
and six additional NPC definitions from the already decoded catalogue. The main
product selection is narrower; named duplicates, minigame variants, note
templates, placeholders and conditional stackability are not blindly promoted
to gameplay identities. Model/animation/interface gaps are exact ID lists in
the generated bindings and `content/m1/asset-references.json`.

## Reproducibility and safety

`research/m1-bindings/input-lock.json` records all consumed source, shared-schema
and generator hashes. `content/m1/manifest.json` records compressed/uncompressed
output hashes. Gzip mtime is zero, filename is absent and OS byte is normalized;
dictionary order, cell order and placement identities are stable. The optional
plain JSON output must stay inside an owned M1 directory.

Compiler validation is deliberately not replaced by Python assertions.
`compile.py` uses the real compiler's source and Runtime CLI, records its source
hashes and preserves nonzero failure. It does not use `--test-fixture`, remove
unrepresentable fields, move source actors onto invented walkable cells or claim
that structural reachability is gameplay acceptance.

The old clipping algorithm reference is explicit inference against the current
source tile/object flags. Source loop guards, probability/timing precision,
stateful mechanics, live NPC origins and source presentation approvals remain
separate gates even when all generator tests pass.
