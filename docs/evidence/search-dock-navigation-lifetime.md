# Search-field callback lifetimes

Source-bound repair of the Theatre Web failure found after the AVIF release. Base:
`develop@18197b991088d9fea0d2894e898aa027801bdddc`. The final PR owns the source SHA and
validation results. This record does not claim deployment or native-device acceptance.

## Reproduction and causal evidence

On production source `101d6c40`, open Home, focus search and click its actual result button.
Navigation reaches the work, but the browser reports `closure invoked recursively or after
being dropped`. Reloading and directly choosing the named dock result reproduces it without
any covered-chip ambiguity. The same actual CSR application on the pinned base reproduces
against controlled HTTP metadata fixtures at loopback; only backend data/delays are synthetic.

The parent `SearchForm` observes input clicks with a delegated bubbling listener. The dock's
button dispatches navigation and disposes that parent before the renderer finishes walking
ancestors of the same click. In the pinned Tachys0.1.9 renderer, delegated dispatch invokes a
node callback and then follows `parent_node`; its event state owns the callback's lifetime.
The dispatcher can therefore reach the detached panel with its callback already invalidated.

Temporary fixed-label callback/cleanup tracing showed result activation, result return and
`SearchForm` cleanup before the error, with no surviving parent-click callback entry. Removing
only focusout still failed, ruling out that initial candidate. Restoring focusout unchanged and
moving only the parent input-click observer to capture produced this order: parent click
begins/ends → result activation/return → cleanup, with no error. All diagnostic edits were
removed before the final repair and the same browser regression was rerun.

The extended keyboard campaign also found a distinct failure: activate the conditional clear
button with Enter, then submit another search. Clearing removed the focused `IconButton`
before focus returned to the input. A temporary panic hook located the disposed reads at
`button.rs:322` (description) and `button.rs:360` (tooltip visibility). Moving focus before
the signal write lets blur close the tooltip while its owner is alive. The same tracked CUA
regression fails before that ordering change and passes five times afterward, with the
diagnostic hook removed. Pointer clear retains its existing behavior.

## Repair boundary

The input-click presentation observer uses `on:click:capture`, so it completes while its panel
is still alive, before a descendant can navigate. It still opens the dock only when the target
is the search input, including a click on an already-focused input after collapse. It does not
stop propagation, defer navigation, swallow exceptions, leak callbacks or add a timer.

The conditional clear action returns focus before clearing its draft. Query timing, request
generation/access fences, route/history ownership, focusout dismissal, composition handling
and shared components/styles are unchanged. There is no dependency,
API, auth, storage, migration or mobile source change.

## Regression and verification scopes

The tracked [CUA browser regression](../../apps/cantos-theatre/web/tests/search-dock-navigation.cua.mjs)
drives the real input and result button, waits for the real work heading, checks the URL and
requires no new browser errors. It failed on the unmodified base and passed with the capture
observer. This executes the browser dispatch/disposal path; a native SSR flag assertion or a
fake event router would not establish it. The [runner notes](../../apps/cantos-theatre/web/tests/README.md)
explain execution; ordinary CI does not automatically run CUA.

`searchClearKeyboardLifetime` additionally drives the actual clear button with Enter, checks
the empty draft and restored input focus, then submits and waits for an excluded work to
return. Five direct-result cycles and five keyboard-clear cycles pass on the final local
candidate with zero console errors. Native Theatre library tests pass 558/558 and shared UI
tests pass 166/166 with package-qualified features; those SSR checks are separate from CSR
evidence. Initial native attempts ran out of disk before execution; an incorrectly unqualified
preview feature enabled CSR in SSR tests and was excluded, then rerun with the correct scope.

After integrating develop `0dd0f63e`, the clean candidate `c3bee765` passes five cycles of each
tracked browser regression, the fresh Web build and strict WASM lint. Correct-feature native
tests in an opt-level1/debug0 test profile with assertions retained pass Theatre565/565 and
shared UI168/168. Its default-profile link ran out of disk before execution; that failed attempt
is retained separately. These counts do not imply live deployment or device acceptance.

The bounded campaign additionally checks repeated direct-result navigation, Enter submission,
draft clear/apply, chip focus, focus/blur and dock dismissal/reopen, Work→Home→new search,
Back/Forward, rapid route changes and delayed stale responses. Exact counts, source/build,
opened captures, native checks and any failures/gaps are retained in the task evidence and PR.
Synthetic HTTP acceptance is distinct from the post-merge live journey.

Residual acceptance: other browser engines, actual Vietnamese IME/touch/virtual keyboard,
screen readers, authenticated flows/audio and native devices are not established by this
campaign. A confirmed static deployment is separate from browser acceptance and APK
distribution. Failed observations remain recorded rather than being relabelled as passes.
