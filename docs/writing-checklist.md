# Writing checklist

Check every user guide against this list before it is committed. Most guides are read by practice staff, often in a hurry, so write for someone who has never seen Muster and has no interest in how it works inside.

The patterns come from Wikipedia's [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) and the [humanizer skill](https://github.com/blader/humanizer) built on it. Machine-drafted text tends to have them, and so does a lot of corporate writing.

## Plain English for staff

- Use the words on the screen. If the button says "I'm coming", write "click I'm coming", not "acknowledge the alarm".
- One instruction per step. Number steps only when the order matters.
- Say what the reader will see, so they know it worked.
- Leave out how it works unless the reader has to act on it.
- British spelling. Times as 07:45, dates as 15 November 2026.

## Patterns to remove

- "Not X but Y", "not just X", or "X, not Y" where nobody claimed X. State Y.
- A short sentence at the end of a paragraph that repeats the paragraph ("That's it.", "Simple as that.").
- Run-ups before the point: "Here's what you need to know", "Let's look at", "Note that".
- Lists of three where the third item is padding. Count the real items.
- Em dashes and en dashes. Use a full stop, comma, colon or brackets. House style uses hyphens only inside words.
- Inflated words: crucial, key, robust, seamless, ensure, leverage, delve, powerful, simply, easily, just. "Simply click" tells a stuck reader they are slow.
- "Serves as", "features", "boasts", "offers". Use "is" and "has".
- Bold labels at the start of every list item ("**Security:** ..."). Use bold only for a word the reader must not miss, and rarely.
- Headings in Title Case, or headings that are slogans. Use sentence case and say what the section covers.
- The page describing itself ("This guide will walk you through"). Start with the first thing the reader needs.
- Sign-offs and offers ("We hope this helps", "Feel free to"). Cut them.

## Before committing

Read the page aloud. Then search it for `—`, `–`, `not just`, `simply`, `just `, `**` and the words above, and check every screenshot and button name against the current app.
