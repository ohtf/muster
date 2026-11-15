# Practice Admin guide

A Practice Admin sets Muster up, adds and removes computers, runs test alarms and keeps an eye on whether every computer can raise and receive alarms. You do this from the Admin tab in the Muster window. To open the window, click the Muster icon by the clock and choose Open Muster.

Have at least two admins, on two different computers, so one lost or broken computer does not lock you out. The Admin tab reminds you if there is only one.

## Setting up the practice

Do this once, on the computer of the person who will be the first admin.

1. In the Muster window, choose Set up a new practice.
2. Enter the practice name, a name for this computer (for example "Practice Manager's PC"), and a recovery passphrase of at least 12 characters.
3. Choose where to save the recovery kit, then click Create practice. This takes a few seconds.
4. Choose this computer's room, and try the practice hold. It does not alert anyone.

### The recovery kit

The recovery kit is a file. With its passphrase, it can make any computer in the practice an admin. You need it if every admin computer is lost or broken, and there is no other way back in.

Copy the kit to a USB stick and keep it somewhere locked, such as the practice safe. Keep the passphrase separately, for example in the practice's password manager or a sealed envelope held by a partner. Delete the copy on the computer once it is safe. Anyone with both the kit and the passphrase can take control of your practice's Muster, so treat them like the keys to the building.

## Adding computers

1. On the Admin tab, under Add a computer, give the code a label (for example "Reception PCs") and choose how many days it stays valid.
2. Click Create code, then Copy.
3. On the new computer, install Muster, choose Join an existing practice and paste the code.
4. Choose the room the computer is in.

One code works for as many computers as you like until it expires. Anyone who has the code can add a computer to your practice, so send it in a way only staff can see, and choose a short expiry. The Enrolment codes list shows which computers joined with each code. If a code may have got out, click Revoke next to it. Computers that already joined stay; remove any you do not recognise.

## Rooms and names

The room is what colleagues see in the alert, so make sure it matches the sign on the door. On the Admin tab, click Edit next to a computer to change its name or room. The change reaches the computer the next time it is on.

Staff can also change the room on their own computer, on the Settings tab. If your IT provider has set the room for a computer, nobody can change it in Muster.

## Removing a computer

On the Admin tab, click Remove next to the computer, then Remove again to confirm. It stops sending and receiving alarms straight away. Do this when a computer is lost, stolen, sold or scrapped. To bring a removed computer back, your IT provider has to reset Muster on it, and then you add it again with a new code.

## Making another admin

Click Make admin next to a computer, then Make admin again. That computer can then do everything on the Admin tab.

## Test alarms

A test alarm goes to every computer, which shows "Test alarm received - no action needed" without the alarm sound. Click Run a test alarm now on the Admin tab. After a minute, the report under Test alarms shows how many computers answered. Click Copy as CSV for the full list, for example to paste into a spreadsheet.

To run one every week, open Practice settings, choose a day and time under Weekly test alarm, choose a computer to run it from, and click Save practice settings. Choose a computer that is always on at that time. If it is off, the report says the test did not run.

## Checking every computer is working

The Devices list on the Admin tab shows each computer's status and when it was last heard. Click Needs attention to see only the ones with a problem.

| Message | What it means | What to do |
|---|---|---|
| No other devices reachable | This computer cannot reach any other. Its alarms will not get through. | Check it is on the practice network. If it is on Wi-Fi, try a cable. If it persists, ask your IT provider whether the network blocks Muster (port 47474). |
| Only reachable via relay | It can only reach others through the relay server. Alarms still get through. | Usually fine for laptops on Wi-Fi. For a desktop, ask your IT provider. |
| Relay unreachable | The relay server is set up but cannot be reached. | Ask whoever runs the relay. |
| Sound is muted on this computer | Alarms will be silent here. | Turn the sound on. |
| No speakers or headphones found | Alarms will be silent here. | Plug in speakers or headphones. |
| Waiting for the new practice key | The computer was off when someone was removed, and has not caught up yet. | Leave it on and connected. It usually clears within a minute. If it does not, tell your IT provider. |
| This computer has been removed from the practice | Someone removed it. | If that was a mistake, ask your IT provider to reset Muster on it, then add it again with a new code. |

## Practice settings

Practice settings apply to every computer, unless your IT provider has set them on a computer. They include the hold time, the sound while holding the button, the alarm sound, the weekly test alarm, opening hours and relay addresses.

Before choosing an alarm sound, click Play next to it on a computer near your fire alarm panel. If it sounds anything like your fire alarm, choose another pattern.

## The audit log

Each computer keeps a record of alarms, test alarms, computers added and removed, and settings changes. On the Admin tab, under Audit log, click Copy as CSV to paste it into a spreadsheet.

## If every admin computer is lost

On any computer still in the practice, open the Settings tab and click Recover admin access. Enter where the recovery kit file is (for example on the USB stick) and its passphrase, then click Make this computer an admin. Then make a second admin, and remove the lost computers.
