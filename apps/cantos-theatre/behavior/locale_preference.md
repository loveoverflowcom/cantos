# Draft: `cantos.presentation.locale-preference` (#169)

Module-owned inventory of [`cantos.presentation.locale-preference`](../../../specs/behavior/catalog.json). The [root inventory](../../../specs/behavior/inventory.md) keeps the shared authority framing; the registry owns the stable ID and lifecycle.

| Classification | Authority / fact | Model consequence |
| --- | --- | --- |
| Bounded implementation contract | [Preference controller](../mobile/shared/src/commonMain/kotlin/com/cantos/theatre/application/preferences/PreferencesController.kt), its [native tests](../mobile/shared/src/commonTest/kotlin/com/cantos/theatre/application/preferences/PreferencesControllerTest.kt) and [repair ledger](../../../docs/evidence/issue169-cmp-locale-acceptance.md) specify saved canonical locale over host seed/default, atomic refusal, retry and theme isolation. | `draft`; the model documents this slice and selects no library/catalog/formatting policy under #167. |
| Finite assumption | Two canonical UI locales, two themes and abstract typed native read/write results; reopening receives a supplied read result, not a proof that disk survived process death. | Named runs, reachability and controls cover the reducer/controller expectation only. No Kotlin observer is registered; native implementation/renderer tests, asynchronous storage, Android/iOS/runtime and accessibility acceptance stay separate. |
