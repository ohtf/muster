# Clinical safety

Muster is safety-relevant software: if it fails to deliver an alarm, or misleads someone about whether an alarm was delivered, a person in danger may not get help. This file is the entry point to its clinical safety documentation.

## What it does

Muster lets staff in GP practices and similar small workplaces call colleagues nearby for help by pressing and holding an on-screen button or keyboard shortcut. Every other enrolled device in the practice shows an alert and sounds an alarm. The sender sees, discreetly, whether the alarm was delivered and who is coming.

## What it does not do

- It does not contact emergency services, monitored alarm centres or anyone outside the practice.
- It does not handle, store or transmit patient data.
- It does not make or support clinical decisions.
- It is not supported in remote desktop sessions or on thin clients (it detects them and says so).

## Intended users and environment

Practice staff (clinical and non-clinical) on Windows, macOS and Linux desktop computers on a practice network, administered by a Practice Admin and deployed by practice or ICB/CSU IT. See [spec/00-overview.md](spec/00-overview.md) for roles.

## Safety owner and status

- Responsible role: Clinical Safety Officer (CSO) for the manufacturer (OHTF). **Not yet appointed for this project.**
- Status: pre-alpha. The hazard log is an initial draft with proposed ratings, not yet reviewed by a CSO. Do not deploy Muster as a practice's panic alarm until the v1 safety gates in [spec/09-roadmap.md](spec/09-roadmap.md) (R44 to R48) are complete.

## Documents

- [safety/hazard-log.md](safety/hazard-log.md) - DCB0129-style hazard log (draft).
- [spec/04-security.md](spec/04-security.md) - threat model and controls.
- [spec/07-testing.md](spec/07-testing.md) - how the safety claims are tested (simulator properties, network tests, hardware lab protocol).

Tests that protect a hazard control cite the hazard ID (for example `H03`) or requirement ID (for example `FR-STS-003`) in their name or doc comment.

## Medical-device applicability

This is a governance record, not a regulatory determination. **Draft, to be completed by the decision-maker** (see `spec/queries.md` Q5).

- Intended purpose: a staff personal-safety alarm that lets workplace staff summon colleagues on the same site. No diagnosis, prevention, monitoring, treatment or alleviation of disease or injury; no patient data; no clinical claims.
- Decision-maker and role: _to be recorded_.
- Decision date: _to be recorded_.
- Next review date: _to be recorded_.
- Evidence considered: MHRA [borderline products](https://www.gov.uk/guidance/borderline-products-how-to-tell-if-your-product-is-a-medical-device) and [regulating medical devices in the UK](https://www.gov.uk/guidance/regulating-medical-devices-in-the-uk) guidance; NHS England [DCB0129 and DCB0160 applicability](https://digital.nhs.uk/services/clinical-safety/applicability-of-dcb-0129-and-dcb-0160/step-by-step-guidance) guidance (to be accessed and dated by the decision-maker); this repository's specification.
- Conclusion: _to be recorded_. The specification's working view is that DCB0129 may not formally apply, but it is followed because it is the recognised framework and eases local DCB0160 assessment by practices.
- Actions and owner: _to be recorded_.
- Reassessment triggers: any change to intended purpose or claims; alerting anyone outside the practice; integration with clinical systems or medical devices; handling patient data; the home profile; mobile apps; hardware buttons.
