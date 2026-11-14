# ADR 0012: Issuers are judged at issue time; revocation applies to the device

- Status: Accepted (2026-11-14), made under the "choose the safer option" rule. Flagged for review by Marcus and the CSO.

## Context

`spec/04-security.md` rule 3 says a cert is valid only if its issuer "has a valid cert", and rule 4 says certs with `expires_at` must not be expired. Read literally, every device enrolled with a 30-day token would stop being a valid member 30 days later, when the token's enroller cert expires, and would silently stop receiving and sending alarms. The same reading makes every device enrolled by a departing practice manager invalid when their admin cert is revoked, which the spec explicitly rules out for non-cascading revocation.

A Revocation carries both `target_serial` and `target_device`, but the spec does not say which one is revoked.

## Decision

1. **Leaf certs** are judged at the current wall time: expired (with 24 h tolerance) or revoked in any way means invalid.
2. **Issuer certs** in the chain are judged as of the child cert's `issued_at`:
   - the issuer must not have expired (with 24 h tolerance) before `issued_at`;
   - a non-cascading revocation of the issuer invalidates only children with `issued_at` later than the revocation's `created_at`;
   - a cascading revocation of the issuer always invalidates the whole subtree.
3. **A revocation applies to the target serial and to every cert whose subject is the target device.** A revoked device must generate new keys to re-enrol. This matches the intent ("remove this device") and stops a revoked device being quietly re-certified under the same key.

## Consequences

- Token-enrolled devices stay enrolled after the token expires, as the spec's flow B and FR-ENR-006 intend.
- `issued_at` is chosen by the issuer, so a holder of a leaked token or a revoked admin key could backdate new certs. This is the known limitation already documented for tokens (`spec/04-security.md`, "Known limitation: backdated self-issued certs"), now explicitly extended to non-cascade-revoked admins. The mitigations are the same: short token lifetimes, admin review of devices by issuer and token, and cascade revocation for suspected compromise.
- Validity is a pure function of the set of certs and revocations, so it is order-independent (P4).
