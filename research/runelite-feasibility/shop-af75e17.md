# Required shop identity ABI adaptation

The explicit 2026-09-14T23:48:34.871-04:00 instruction was applied in order:

* 3310032cc6621f41d395103a35e81b2cfd87e1ae →
  `1216abac311bf76998525211d07645a64515e40d`
* af75e17b98eeb94adbb9ef6d1066b3ec805a866a →
  `f1ede5d748894ab71388af2ecc45d06840791076`

Canonical faa8002/c841f6a contact, private played-time and ground-clock fixes
remain present. The current raw source artifact remains
`a200ca08c80f6a3fc812fe02ca95f52325e183084f83b12e8d470a1f37cc6d62`.
No source assets, reference hashes, gameplay defaults or additional protocol
fields were modified by the owned adaptation.

## Actual client contract changes

The authoritative `game.proto` was regenerated with the existing checksum-pinned
protoc3.21.12. The official RuneLite1.12.38 runtime, API, plugin JAR and decoder
are unchanged.

`ShopSelection.fromDisplayedRow(displayedShop, displayedIndex, quantity)` creates
one immutable message from the **actual displayed `ShopLine.item` canonical ID**.
It looks up the row's declared index, not its array offset. It copies neither a
numeric native/source item ID nor a displayed price. The same selection supplies
both `buyDisplayedRow` and `quoteDisplayedRow` transport entry points.

New buy and quote requests always include `expected_item`, including fixed
catalog rows. Empty/missing, numeric-source and wrong-kind identities are
rejected locally, never converted into legacy `None`. Pricing, stock, capacity,
permissions, tombstone cleanup and restock clocks remain entirely authoritative.
The client does not implement or reinterpret those rules.

Wire changes are exactly additive: `ShopBuy.expected_item` is optional string
field4; `WorldInput.shop_buy` remains25 and `QuoteRequest.shop_buy` remains3.
No account-v1 tag or protocol-version change is made.

## Stale rows and uncertain retries

The transport retains an immutable identified `ClientMessage` before sending an
input. `retryPendingInput()` resends that exact message, preserving operation
UUID, sequence, quantity, item identity and observation fields. New input is
blocked while an operation is unresolved. Retries do **not** rebuild an intent
from a newer shop row. The retained-message path also preserves historical
identity-less bytes; new-request validation is not applied as a migration to
old messages.

For a shop conflict, the transport requests a fresh read-only world view and
notifies the caller to choose a currently displayed item. It clears a rejected
pending selection only when the returned next sequence confirms it was not
consumed. An advanced sequence, network failure or failed refresh does not claim
rollback or permit a silently substituted purchase. Quote conflicts similarly
refresh without automatic identity replacement. Failed refresh is explicitly
reported, not labeled successful.

Pending operations are retained **in memory**; durable cross-process
recovery/reconnect remains outside this prototype's verified coverage. Native
shop-widget/menu interaction and a live shop transaction have not been
demonstrated. These transport entry points are not a new desktop UI.

## Exact executable checks

`validation-shop-af75e17.json` records commands, exits and output:

* Java compilation: pass;16 source files, no upstream class modifications.
*35 synthetic Java shop checks: displayed canonical identity for buys/quotes,
  sparse/reused indices, immutable old/new selections, missing/empty identity
  rejection, exact additive field4 bytes, unchanged action25/quote3 envelopes,
  historical `None` roundtrips and unchanged pending retry bytes.
*9 existing generated-codec/projection checks and5 Python harness checks: pass.
* Shared `clubscape-protocol --test shop_identity`:3 pass.
* Shared `clubscape-server --test shop_intent_compatibility`:2 pass, including
  exact old `None` JSON/wire behavior and domain-separated canonical-v1 hashes
  for historical fixed/extra-row intents.
* Owned public-service probe Clippy with warnings denied and rustfmt: pass.

These are codec/client-contract/shared-component checks, **not live shop,
RuneLite plugin-update, full M1, or presentation acceptance**.

Reproduce without altering historical reports:

```sh
JDK17=/home/lramos15/.local/share/jdks/temurin-17.0.20.1+1
python3 tools/runelite-compatibility/prepare.py --java-home "$JDK17" \
  --report research/runelite-feasibility/build-inputs-shop-af75e17.json
python3 tools/runelite-compatibility/build.py --java-home "$JDK17" \
  --report research/runelite-feasibility/build-shop-af75e17.json \
  --history research/runelite-feasibility/build-history-shop-af75e17.json
python3 tools/runelite-compatibility/validate.py --java-home "$JDK17" \
  --report research/runelite-feasibility/validation-shop-af75e17.json
```

## Remaining status

The backend shop identity/capacity interlock is closed; the owned ABI adaptation
is implemented and checked. The previously recorded **different** startup
surfaces (game-root environment nesting and insufficient descriptor capacity)
are unchanged by these commits. The three-invocation ledger and previous
research remain frozen; no new live attempt, database, account, XP callback or
plugin gain was created by this update.

Compatibility remains **UNVERIFIED**, with no support tier or approved desktop
deferral. Director startup fixes and renewed explicit live bounds remain needed
for the original-runtime server/scene/player/event/plugin tuple.
