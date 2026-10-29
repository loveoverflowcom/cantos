# Browser regressions through CUA

`search-dock-navigation.cua.mjs` uses the documented CUA tab API to drive the actual compiled
Theatre Web. Run it in an initialized CUA REPL with a tab on a loaded Home catalogue; import the
module using the checkout's absolute `file://` path, then pass that tab and a visible work:

```js
await searchDockResultNavigation(tab, {
  title: "Ánh đèn cuối sân khấu",
  resultLabel: "Ánh đèn cuối sân khấu Story",
  workId: "10000000-0000-4000-8000-000000000001",
});
```

These example facts come from the checked discovery metadata fixture and an English interface.
For Vietnamese use its actual visible result label. A controlled HTTP fixture may serve the
catalogue, but the input, result button, route, renderer disposal and browser console are real.
The regression throws with a `report` when navigation fails or emits an error. Persist that
report for a red run; a returned `status: PASS` is the green result.

This is a browser-only regression, not a Node CLI, injected DOM event or native SSR assertion.
Ordinary CI does not start CUA or a browser. Record the source/build, browser, HTTP fixture or
live backend, parameters and result separately; native format/lint/tests cannot prove this
DOM dispatch lifetime. The source-bound [qualification record](../../../../docs/evidence/search-dock-navigation-lifetime.md)
records the observed red/green runs and remaining acceptance.
