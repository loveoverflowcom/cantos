# Leptos preview renewal for the WebP dependency change

Scope is the existing shared gallery qualification. The Cargo lockfile changed for
Server codecs. The [prior qualification](../web-native-submit-737.md) retains its
original source and evidence. The shared gallery implementation, tokens, declarations
and resolved WASM dependency tree at baseline 988dd2c8 are identical to this change.

Both galleries were rebuilt on macOS arm64 with Rust 1.99.1, Trunk 0.21.14 and
CARGO_PROFILE_DEV_DEBUG=0. On the current tree, the existing verification command passed
the component tests, WASM lint and Trunk build. On the baseline, the same Trunk command
built into a separate directory. A recursive byte comparison matched every file in both
output directories. The WASM SHA-256 is
9cc91975ac5a20e447e422a2e947a5ec634da808d6deb9bd7703ca69138d0349.

- Structural check ran with cargo xtask ui-preview leptos verify. A normalized cargo tree
  for cantos-ui with the preview feature and wasm32-unknown-unknown had SHA-256
  8670cad9950a7972363011680da9abf0b584ecb83442a3f94ccd098ce5b49b0b on both sides.
- The current gallery mounted in Chrome 156.0.8078.80 over loopback. The CDP document had
  310 controls, ten recipe choices and no uncaught exceptions. The accessibility tree
  contained named buttons and comboboxes. Focus moved to the recipe component selector.
- Opened the 390 CSS px light and dark captures of the enabled Vietnamese button recipe.
  Both render the icon, wrapped label and corresponding themed colors. The scrollable
  workbench clips the fixed-width specimen at the side of this narrow viewport. Identical
  gallery bytes preserve the earlier broader accessibility and visual observations.

Reproduce the bundle comparison with the same debug settings in the base and current
checkouts. From packages/ui-system/leptos, run trunk build preview.html --locked --dist
with a different output directory for each checkout, then compare the directories with
diff -qr. Serve the current gallery on loopback and use Chrome's DevTools DOM and
Accessibility tree. Set the theme with the recipe selector and inspect the specimen at
390 CSS px. This renewal covers the shared gallery. Theatre and Studio product acceptance
and native devices have their own checks.
