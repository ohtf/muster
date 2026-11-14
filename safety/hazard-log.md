# Hazard log (initial draft)

Format follows DCB0129. Ratings are **proposed** and must be reviewed by the manufacturer's Clinical Safety Officer. Whether DCB0129 formally applies to a staff panic alarm is debatable. It is used here because it is the recognised framework and makes local DCB0160 assessment by practices easier.

## Risk matrix

Likelihood: Very low (1), Low (2), Medium (3), High (4), Very high (5).
Consequence: Minor (1), Significant (2), Considerable (3), Major (4), Catastrophic (5).
Risk rating: 1 Very low, 2 Low, 3 Medium, 4 High, 5 Very high, using the DCB0129 risk matrix (to be inserted by the CSO).

## Hazards

| ID | Hazard | Causes | Effect | Init L / C / Risk | Controls (spec refs) | Residual L / C / Risk |
|---|---|---|---|---|---|---|
| H01 | Alarm not delivered to anyone | Network blocks all paths; agent not running; device not enrolled | Person in danger gets no help | 3 / 5 / High | Multi-path delivery (ADR 0004); re-flood; relay; honest status light (FR-STS-003, P8); sender panel shows non-delivery (05, "Could not reach other devices"); watchdog (FR-PLT-002); drills (FR-DRL); coverage view | 1 / 5 / Medium |
| H02 | Alarm delivered but not noticed | Muted or missing speakers; user away; window hidden; locked screen | Delayed or no response | 3 / 4 / High | Audio status in receipt and heartbeat; amber status; override mute (FR-RCV-009); topmost window, taskbar flash; looping escalating sound; responder list visible to sender | 2 / 4 / Medium |
| H03 | Sender falsely reassured | UI shows delivery or responders when there are none | Sender stops seeking other help | 2 / 5 / High | Delivery count only from real receipts (P10); "Not yet delivered" at 5 s; explicit failure message at 15 s | 1 / 5 / Medium |
| H04 | Accidental alarm | Mis-click; object on keyboard; knee on button | Alarm fatigue; disruption; staff disable software | 4 / 2 / Medium | 1.5 s hold; soft cue during hold; cancel as sent in error; near-miss logging; lab test L10 | 2 / 2 / Low |
| H05 | Malicious false alarm by outsider | Spoofed or replayed packets | Disruption; fatigue | 3 / 2 / Medium | Signatures, certs, encryption, persisted dedupe, max life (04; P2, P5) | 1 / 2 / Very low |
| H06 | Malicious false alarm by insider | Enrolled user abuses system | Disruption | 2 / 2 / Low | Attribution and audit; admin revocation | 2 / 2 / Low |
| H07 | Aggressor alerted by activation | Audible sound on sender device | Escalation of violence | 3 / 4 / High | Silent to room (ADR 0008); soft non-alarm cue only during hold; neutral sender panel | 1 / 4 / Medium |
| H08 | Stale alarm triggers response | Device wakes after incident; replay | Wasted response; fatigue | 3 / 1 / Low | Anti-entropy sync, max life, expiry (FR-ALM-008; P5; S8) | 1 / 1 / Very low |
| H09 | Wrong location shown | Room misconfigured; hot-desking | Responders go to the wrong room | 3 / 4 / High | Room set at enrolment; room in tray menu; admin coverage shows rooms; optional confirm on login (post-v1); user display name shown | 2 / 4 / Medium |
| H10 | Remote session users unprotected | Citrix/RDS/thin clients | User believes they are covered | 3 / 4 / High | Remote session detection with visible disabled state (FR-PLT-003); documented limitation; IT guidance | 2 / 4 / Medium |
| H11 | Confusion with fire alarm | Similar sound pattern | Evacuation instead of help, or the reverse | 2 / 4 / Medium | Distinct rhythmic patterns (FR-RCV-011); choice of patterns; lab test L14; practice guidance | 1 / 4 / Low |
| H12 | Loss of admin control | Sole admin leaves; keys lost | Cannot add or remove devices; revoked laptops stay enrolled | 3 / 3 / Medium | 2-admin warning (FR-ADM-004); recovery kit (04, flow D); non-cascade revocation | 1 / 3 / Low |
| H13 | Lost or stolen device keeps access | Laptop theft | Read or inject alarms | 2 / 2 / Low | Revocation and rotation; relay enforcement; DPAPI | 1 / 2 / Very low |
| H14 | Update breaks interoperability | Mixed versions in a practice | Partial delivery | 2 / 5 / High | Versioning rules (03); N-1 compatibility (NFR-COMP-001); mixed-version warning in admin view; managed updates | 1 / 5 / Medium |
| H15 | Reliance on Muster instead of emergency services | Users or practices treat it as a monitored alarm or a 999 route | Delayed emergency response | 3 / 5 / High | Clear documentation and first-run text that Muster calls colleagues nearby only; practice emergency procedures remain primary | 2 / 5 / High (accepted with documentation; review) |
| H16 | Lone person (home profile) with no responder | Nobody else present or enrolled | No help arrives | 3 / 5 / High | Home profile out of v1 scope; when introduced, require at least one other device and display "This only alerts people on your devices"; integration outputs to remote contacts (1.1) | n/a in v1 |
| H17 | Drill fatigue or alarm mistaken for drill | Too many or audible drills; drill flag misread | Real alarm ignored | 2 / 4 / Medium | Silent drills by default; drills visually distinct; real alarms never use drill styling (P12) | 1 / 4 / Low |
| H18 | Agent prevents clinical work | Alert window steals focus or keystrokes into the clinical system | Data entry error in patient record | 2 / 3 / Medium | No default button; non-activating window (FR-RCV-002, FR-RCV-008); future HID uses vendor-defined interface, not keystrokes | 1 / 3 / Low |

## Open actions

- CSO to confirm the matrix and the ratings.
- Add evidence links (test reports, lab results) to each control before release.
- Review H15 wording with practice managers during the pilot.
