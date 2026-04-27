# Pepper Migration Playbook (browse_signals.domain_hash)

> Status: planned for v0.7 — this document captures the upgrade contract so v0.6
> users know what will change and v0.7 implementers have a written spec.

## Current state (v0.6)

`browse_signals.domain_hash` is computed as `HMAC-SHA256(DOMAIN_HASH_PEPPER, domain)`
where `DOMAIN_HASH_PEPPER = b"attune.browse_signals.v1.2026"` is a **compile-time
constant** baked into every Attune binary.

Tradeoffs accepted in v0.6:

- ✅ Same `attune` binary → same hash for the same domain → "delete all signals
  for github.com" stays correct across reinstalls
- ✅ HMAC-SHA256 with any pepper is far stronger than naked SHA-256 (defeats
  rainbow-table reversal of common domains like `google.com`, `mail.qq.com`)
- ❌ Two different vaults on two different machines hash the same domain to the
  same value — an attacker who exfiltrates both `vault.sqlite` files and knows
  the pepper can correlate browsing across them
- ❌ The pepper is not user-secret (it ships in the binary)

## Target state (v0.7)

`DOMAIN_HASH_PEPPER` becomes **vault-salt-derived**:

```rust
let vault_pepper = hkdf_expand(vault_salt, b"browse_signals.domain_hash.v2", 32);
let domain_hash = HMAC-SHA256(vault_pepper, domain);
```

Properties gained:
- Per-vault pepper → exfiltrated dual vaults cannot be correlated by domain
- Pepper rotates whenever vault is re-keyed (change-password flow)

## Migration challenge

Existing rows in `browse_signals` have `domain_hash` computed under the OLD pepper.
After upgrade, naive lookup `WHERE domain_hash = HMAC(new_pepper, domain)` finds
nothing — so the per-domain delete button + history filter would silently break.

## Migration algorithm (planned for v0.7)

1. **Schema version tracking**
   - Add `vault_meta` row: `key = 'pepper_version'`, `value = 'v1'` for legacy vaults
   - On Store::open, if `pepper_version` ≠ current code version → trigger migration
   - Single forward direction: `v1 → v2` (no downgrade path)

2. **Re-hash pass** (background job, throttled by H1 governor Conservative profile)
   - Iterate `browse_signals` in batches of 100
   - For each row:
     - Decrypt `url_enc` with DEK → extract domain via `host_of()`
     - Compute new hash: `HMAC(new_pepper, domain)`
     - `UPDATE browse_signals SET domain_hash = ? WHERE rowid = ?`
   - Commit in transactions of 100 rows (safe to interrupt)
   - On row decrypt failure (foreign vault leftover): leave row alone, log warning
     — `list_recent_browse_signals` already silent-skips these (per R15 P1)

3. **Mark complete**
   - `UPDATE vault_meta SET value = 'v2' WHERE key = 'pepper_version'`
   - Subsequent opens skip the re-hash pass

4. **User-visible window**
   - Migration runs on first v0.7 launch when vault unlocks
   - Per-domain operations may temporarily return mixed/empty results (~seconds-minutes
     for typical 10K-row vaults, longer for power users)
   - UI shows "Upgrading browse signals..." toast during the pass
   - "全清浏览信号" still works (domain_hash agnostic)

## Rollback

If v0.7 is reverted to v0.6:
- New rows written under v2 pepper become unreachable to v0.6's v1 pepper lookup
- "Delete by domain" silently skips them
- "全清" still works
- **Recommendation**: do not downgrade across pepper version. v0.7 release notes will mark this as Breaking.

## Testing

Migration tests live in `rust/crates/attune-core/tests/migration_roundtrip_test.rs`
following the W3 batch A `migrate_breadcrumbs_encrypt` pattern (per R07 P0):

- `migration_drops_old_plaintext_breadcrumb_column` — old column gone
- `migration_is_idempotent_on_second_open` — re-running is no-op
- `encrypted_breadcrumb_survives_close_and_reopen` — encrypted data round-trips

A `migration_pepper_v1_to_v2_rehashes_all_rows` test will be added when v0.7 ships.

## Why deferred to v0.7

W3 batch B prioritized shipping G1 capture + G5 privacy panel + sidecar encryption
(R04 P0-1, the immediate at-rest exposure). Pepper-versioning is a defense-in-depth
hardening with a well-understood migration path — appropriate for v0.7 alongside
the planned key-rotation work (K5 Items Keys per Standard Notes 004 spec).

## References

- W3 batch B design spec: `docs/superpowers/specs/2026-04-27-w3-batch-b-design.md`
- R04 P0-1 review (sidecar encryption gap that motivated the broader audit): `tmp/w3-final-review-tracker.md`
- v0.7 K5 design (Standard Notes 004 items keys): planned, see strategy plan
- HKDF (RFC 5869) — pepper derivation primitive used above
