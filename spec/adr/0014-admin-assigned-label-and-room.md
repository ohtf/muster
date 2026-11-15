# ADR 0014: Admins set a device's label and room with a DeviceSettings message

- Status: Accepted (2026-11-15).

## Context

FR-ADM-002 says admins can rename devices and set rooms. The protocol had no message for that: a device's label and room came only from its own configuration, and a device announced them in its heartbeat. Rooms are what a colleague reads in the alert ("Consulting Room 4"), so a wrong room sends help to the wrong place, and the admin is usually the person who knows the practice's room names.

## Decision

1. **A new sealed, flooded message, 0x09 DeviceSettings**: `{0: target DeviceId, 1: label tstr (optional), 2: room tstr (optional), 3: assigned_at uint}`. A missing field leaves that value unchanged. Strings follow the usual 64-byte send limit.
2. **Only a valid admin's message counts**, and only the target applies it. Other devices re-flood it and learn the new values from the target's heartbeat as usual.
3. **One slot for the device's own label and room**, kept by the core and persisted (`PersistOp::SetOwnSettings`). Both the person's own Settings and an admin's DeviceSettings write it, and **the newest `assigned_at` wins**. So a person can still correct their room after an admin set it, and an admin message that arrives late does not undo a newer local change.
4. **Machine configuration and policy win over both.** If `device.room` or `device.label` is set in the machine config or policy, the core ignores changes to it, and Settings shows the room as set by IT.
5. **Delivery to devices that are off.** The issuing admin keeps the change in memory and, whenever it hears a heartbeat from the target that does not show it, sends it again directly (at most every 30 s, for up to 7 days). The change is done once the heartbeat shows it.
6. **Audit.** The target logs `DeviceSettingsAssigned { by }`.

The desktop app's own room setting, which briefly lived in a `user.toml` in the data folder, now goes through the same slot.

## Consequences

- One new message type, with a golden vector (`crates/panic-button-proto/tests/vectors/device_settings.hex`). Older devices drop it as an unknown type, which is harmless: their admin's change simply does not apply until they update.
- The pending re-send is lost if the admin's app restarts before the target comes back. The admin can set it again; the admin view shows the room each device reports.
- An admin can relabel a device to something misleading. This is no more than an admin can already do (they can remove devices), and the change is audited on the target.
- Scenario `admin_assigns_label_and_room` covers the flood, a sleeping device, the person's later change, a configuration lock, and a member being refused.
