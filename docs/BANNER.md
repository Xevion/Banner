# Banner

Notes on the internal workings of Ellucian's Banner system, as observed against UTSA's instance.
None of this is documented upstream; it is what the API actually does.

## Sessions

- Sessions are generated on demand as a random string: `{5 random characters}{milliseconds since
  epoch}`.
- They expire after inactivity. The delay is in the original HTML, as the `content` attribute of
  `meta[name="maxInactiveInterval"]`, read at runtime by the page's own JavaScript. Upstream
  advertises 30 minutes; we treat a session as dead at 25, on the assumption that the published
  value is a ceiling and not a promise.
- An inactivity dialog offers to extend the session through `keepAliveURL` (see
  `meta[name="keepAliveURL"]`). That endpoint does not appear to care whether the session is or ever
  was valid: it returns `200 OK` with `I am Alive` regardless, so it cannot be used to test
  validity.
- Searching with an invalid session (or none) returns `200 OK` with an empty result envelope rather
  than an error:

```json
{
  "success": true,
  "totalCount": 0,
  "data": null, // always an array when the session is valid, even if empty
  "pageOffset": 0,
  "pageMaxSize": 10,
  "sectionsFetchedCount": 0,
  "pathMode": "registration", // normally "search"
  "searchResultsConfigs": null, // normally an array
  "ztcEncodedImage": null // normally a static string in base64
}
```

The `pathMode` flip to `registration` is the most reliable tell. `success: true` is not a signal
of anything.

- Sessions appear to be term-scoped in practice: the term is selected against the session before a
  search, so one session per term being worked is the model we use.

## Open questions

Behavior we have not pinned down, roughly in order of how much it would change:

- How does an _expired_ session differ from an invalid or never-issued one? Both look empty; it is
  unclear whether `pathMode` distinguishes them.
- How many sessions can be held concurrently, and is there a limit or a cost to holding them?
- Which endpoints besides search are affected by the term selection made against a session?
- Nullability, broadly. Much of the response surface is nullable and it is rarely clear whether a
  null is "absent", "not applicable", or "not populated for this term". Each field resolved this
  way is a field that can stop being an `Option`.
- Meeting schedule types: what distinguishes `AFF`, `AIN`, `AHB` and the rest.
- Is `partOfTerm` always populated, and what does it mean across different result shapes?

## Settled

Things that were once open and now have answers in the data:

- **CRNs repeat across terms.** They are unique only within a term, so `(crn, term_code)` is the
  identity of a section, and a bare CRN is not a key.
- **Sections carry multiple meeting times.** A section's schedule is a list, not a single
  time-and-place, and a course can meet on different days at different locations within one term.
  Flattening it loses real sections.
- **Sections carry multiple instructors**, distinguished by a primary indicator rather than
  ordering.
