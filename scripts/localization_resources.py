"""Accepted versioned Web resource projections; the #505 fixture remains separate evidence.

Called by the existing localization/repository gate. Generation is offline and deterministic.
The catalog owns text and argument meaning; app adapters retain domain-to-key decisions.
"""
import hashlib
import json
from pathlib import Path
import re

from localization_catalog import CatalogError, exact_fields, load, quoted, require, segments, symbol

ROOT = Path(__file__).resolve().parents[1]
SOURCE = Path("packages/localization/catalog/v1/catalog.json")
TARGETS = {
    "studio": Path("apps/cantos-studio/web/src/localization/generated.rs"),
    "theatre": Path("apps/cantos-theatre/web/src/localization/generated.rs"),
}
IDENT = re.compile(r"[a-z][a-z0-9]*(?:_[a-z0-9]+)*\Z")
KEY = re.compile(r"(common|studio|theatre)\.([a-z][a-z0-9_]*)\.([a-z][a-z0-9_]*)\Z")
RUST_RESERVED = set("as break const continue crate else enum extern false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where while async await dyn abstract become box do final macro override priv typeof unsized virtual yield try".split())
PLURALS = {"zero", "one", "two", "few", "many", "other"}


def validate(catalog):
    exact_fields(catalog, {"schemaVersion", "icu4x", "cldr", "fallback", "locales", "messages"}, "resource catalog")
    require(catalog["schemaVersion"] == 1 and type(catalog["schemaVersion"]) is int, "resource schema version")
    require(catalog["icu4x"] == "2.3" and catalog["cldr"] == "48.2.1", "resource ICU4X/CLDR pin")
    tags, aliases, names = set(), set(), set()
    require(isinstance(catalog["locales"], list) and catalog["locales"], "resource locales empty")
    for locale in catalog["locales"]:
        exact_fields(locale, {"tag", "symbol", "autonym", "aliases", "testOnly", "dir", "cardinalCategories"}, "resource locale")
        require(isinstance(locale["tag"], str) and re.fullmatch(r"[a-z]{2,3}(?:-[A-Z][a-z]{3})?(?:-[A-Z]{2})?", locale["tag"]), "resource locale tag")
        require(isinstance(locale["symbol"], str) and re.fullmatch(r"[A-Z][A-Za-z0-9]*", locale["symbol"]), "resource locale symbol")
        require(locale["symbol"] != "Self", "resource reserved locale symbol")
        require(locale["tag"] not in tags and locale["symbol"] not in names, "resource duplicate locale")
        tags.add(locale["tag"])
        names.add(locale["symbol"])
        require(isinstance(locale["autonym"], str) and locale["autonym"].strip(), "resource empty autonym")
        require(type(locale["testOnly"]) is bool and isinstance(locale["dir"], str) and locale["dir"] in {"ltr", "rtl"}, "resource locale exposure/direction")
        require(isinstance(locale["aliases"], list), "resource aliases")
        for tag in [locale["tag"], *locale["aliases"]]:
            require(isinstance(tag, str) and tag.strip() == tag and tag and tag.lower() not in aliases, "resource ambiguous alias")
            aliases.add(tag.lower())
        categories = locale["cardinalCategories"]
        require(isinstance(categories, list) and all(isinstance(item, str) for item in categories) and len(set(categories)) == len(categories) and "other" in categories and set(categories) <= PLURALS, "resource plural categories")
    require(any(row["tag"] == catalog["fallback"] and not row["testOnly"] for row in catalog["locales"]), "resource fallback")
    require(isinstance(catalog["messages"], dict) and catalog["messages"], "resource messages empty")
    symbols = set()
    for key, message in catalog["messages"].items():
        require(isinstance(key, str) and KEY.fullmatch(key), f"resource semantic key: {key}")
        require(key.split(".")[1] not in RUST_RESERVED and symbol(key.split(".")[-1]) != "Self", "resource reserved generated identifier")
        require(symbol(key) not in symbols, "resource generated collision")
        symbols.add(symbol(key))
        exact_fields(message, {"args", "translations", "exposure"}, f"resource message {key}")
        require(isinstance(message["exposure"], str) and message["exposure"] in {"production", "preview"}, "resource message exposure")
        require(isinstance(message["args"], dict), "resource named arguments")
        for arg, kind in message["args"].items():
            require(isinstance(arg, str) and IDENT.fullmatch(arg) and isinstance(kind, str) and kind in {"text", "count"}, "resource argument type")
        require(isinstance(message["translations"], dict) and set(message["translations"]) == tags, f"resource locale completeness: {key}")
        selector = None
        for locale in catalog["locales"]:
            pattern = message["translations"][locale["tag"]]
            require(isinstance(pattern, dict), "resource pattern object")
            if set(pattern) == {"text"}:
                templates = [pattern["text"]]
            else:
                exact_fields(pattern, {"plural", "forms"}, "resource plural pattern")
                require(isinstance(pattern["plural"], str) and message["args"].get(pattern["plural"]) == "count", "resource plural count")
                require(isinstance(pattern["forms"], dict) and set(pattern["forms"]) == set(locale["cardinalCategories"]), "resource plural coverage")
                templates = pattern["forms"].values()
            current = ("plural", pattern["plural"]) if "plural" in pattern else ("text",)
            require(selector is None or selector == current, "resource selector drift")
            selector = current
            for template in templates:
                require({value for kind, value in segments(template) if kind == "arg"} == set(message["args"]), f"resource argument parity: {key}")
    for owner in TARGETS:
        keys = [(key.split('.')[1], symbol(key.split('.')[-1])) for key in catalog["messages"] if key.split('.')[0] in {"common", owner}]
        require(len(keys) == len(set(keys)), "resource consumer group collision")
    return catalog


def locale_source(catalog, include_test=False):
    locales = [row for row in catalog["locales"] if include_test or not row["testOnly"]]
    variants = ", ".join(("#[default] " if row["tag"] == catalog["fallback"] else "") + row["symbol"] for row in locales)
    lines = ["#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]", f"pub enum Locale {{ {variants} }}", "impl Locale {"]
    for method, field in [("lang", "tag"), ("autonym", "autonym"), ("dir", "dir")]:
        lines += [f"    pub const fn {method}(self) -> &'static str {{ match self {{"]
        lines += [f"        Self::{row['symbol']} => {quoted(row[field])}," for row in locales]
        lines += ["    } }"]
    supported = ", ".join("Self::" + row["symbol"] for row in locales if include_test or not row["testOnly"])
    lines += [f"    pub fn supported() -> impl Iterator<Item=Self> {{ [{supported}].into_iter() }}",
              "    pub fn from_lang(tag: &str) -> Option<Self> { Self::resolve(tag).ok().map(|row| row.locale) }",
              "    pub fn resolve(requested: &str) -> Result<Resolution, LocaleError> {",
              "        let canonical = cantos_localization::canonical_locale(requested).map_err(|_| LocaleError { requested: requested.to_owned() })?;",
              "        let mut candidate = canonical.as_str();", "        loop {"]
    for row in locales:
        if row["testOnly"] and not include_test:
            continue
        checks = " || ".join(f"candidate.eq_ignore_ascii_case({quoted(tag)})" for tag in [row["tag"], *row["aliases"]])
        lines += [f"            if {checks} {{ return Ok(Resolution {{ locale: Self::{row['symbol']}, exact: requested.eq_ignore_ascii_case({quoted(row['tag'])}) }}); }}"]
    lines += ["            let Some((parent, _)) = candidate.rsplit_once('-') else { break; };",
              "            candidate = parent;", "        }",
              "        Err(LocaleError { requested: requested.to_owned() })", "    }", "}",
              "#[derive(Clone, Copy, Debug, PartialEq, Eq)]", "pub struct Resolution { pub locale: Locale, pub exact: bool }",
              "#[derive(Clone, Debug, PartialEq, Eq)]", "pub struct LocaleError { pub requested: String }",
              "#[derive(Clone)]", "pub struct LocaleMap<T> { " + ", ".join(f"value_{n}: T" for n in range(len(locales))) + " }",
              "impl<T> LocaleMap<T> {",
              "    pub fn try_new<E>(mut create: impl FnMut(Locale) -> Result<T, E>) -> Result<Self, E> {",
              "        Ok(Self { " + ", ".join(f"value_{n}: create(Locale::{row['symbol']})?" for n, row in enumerate(locales)) + " })", "    }",
              "    pub fn get(&self, locale: Locale) -> &T { match locale {"]
    lines += [f"        Locale::{row['symbol']} => &self.value_{n}," for n, row in enumerate(locales)]
    lines += ["    } }", "}"]
    lines += ["impl std::fmt::Display for Locale { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.lang()) } }"]
    return lines


def pattern_expression(pattern, message, kotlin=False):
    def template_expression(template):
        pieces = [quoted(value) + ".to_owned()" if kind == "text" else f"value_{value}.clone()" for kind, value in segments(template)]
        return "[" + ", ".join(pieces) + "].concat()"
    if "text" in pattern:
        return template_expression(pattern["text"])
    cases = [f"cantos_localization::PluralCategory::{symbol(category)} => {template_expression(template)}," for category, template in sorted(pattern["forms"].items())]
    return "match formatter.cardinal(&arg_" + pattern["plural"] + ".to_string())? { " + " ".join(cases) + " _ => return Err(cantos_localization::FormatError::DataUnavailable), }"


def projections(catalog, include_test=False):
    digest = hashlib.sha256(json.dumps(catalog, sort_keys=True, ensure_ascii=False).encode()).hexdigest()
    outputs = {}
    for owner, target in TARGETS.items():
        messages = {key: row for key, row in catalog["messages"].items() if key.split('.')[0] in {"common", owner} and not key.split('.')[1].startswith("cmp_")}
        lines = [f"// Generated by scripts/localization_catalog.py --resources; DO NOT EDIT. sha256: {digest}", *locale_source(catalog, include_test)]
        groups = sorted({key.split('.')[1] for key in messages})
        for group in groups:
            members = {key: row for key, row in messages.items() if key.split('.')[1] == group and not row["args"]}
            if not members:
                continue
            if all(row["exposure"] == "preview" for row in members.values()):
                lines += ['#[cfg(feature = "preview")]'] if owner == "theatre" else ["#[cfg(debug_assertions)]"]
            lines += [f"pub mod {group} {{", "    use super::Locale;", "    #[derive(Clone, Copy, Debug, PartialEq, Eq)]", "    pub enum Key {"]
            for key, row in sorted(members.items()):
                if row["exposure"] == "preview":
                    lines += ['        #[cfg(feature = "preview")]'] if owner == "theatre" else ["        #[cfg(debug_assertions)]"]
                lines += ["        " + symbol(key.split('.')[-1]) + ","]
            lines += ["    }", "    impl Key { pub const ALL: &'static [Self] = &["]
            for key, row in sorted(members.items()):
                if row["exposure"] == "preview":
                    lines += ['        #[cfg(feature = "preview")]'] if owner == "theatre" else ["        #[cfg(debug_assertions)]"]
                lines += ["        Self::" + symbol(key.split('.')[-1]) + ","]
            lines += ["    ]; }", "    pub fn text(key: Key, locale: Locale) -> &'static str { match key {"]
            for key, row in sorted(members.items()):
                if row["exposure"] == "preview":
                    lines += ['        #[cfg(feature = "preview")]'] if owner == "theatre" else ["        #[cfg(debug_assertions)]"]
                lines += ["        Key::" + symbol(key.split('.')[-1]) + " => match locale {"]
                for locale in [row for row in catalog["locales"] if include_test or not row["testOnly"]]:
                    pattern = row["translations"][locale["tag"]]
                    value = pattern.get("text", pattern.get("forms", {}).get("other"))
                    lines += [f"            Locale::{locale['symbol']} => {quoted(value)},"]
                lines += ["        },"]
            lines += ["    } }", "}"]
        parameterized = {key: row for key, row in messages.items() if row["args"]}
        if parameterized:
            borrowed = any("text" in row["args"].values() for row in parameterized.values())
            lines += ["pub enum Message<'a> {" if borrowed else "pub enum Message {"]
            for key, row in sorted(parameterized.items()):
                if row["exposure"] == "preview":
                    lines += ['    #[cfg(feature = "preview")]'] if owner == "theatre" else ["    #[cfg(debug_assertions)]"]
                fields = ", ".join(f"arg_{name}: " + ("u64" if kind == "count" else "&'a str") for name, kind in sorted(row["args"].items()))
                lines += [f"    {symbol(key)} {{ {fields} }},"]
            lines += ["}", "impl Message<'_> {" if borrowed else "impl Message {", "    pub fn render(&self, locale: Locale) -> Result<String, cantos_localization::FormatError> {", "        match self {"]
            for key, row in sorted(parameterized.items()):
                if row["exposure"] == "preview":
                    lines += ['            #[cfg(feature = "preview")]'] if owner == "theatre" else ["            #[cfg(debug_assertions)]"]
                fields = ", ".join("arg_" + name for name in sorted(row["args"]))
                lines += [f"            Self::{symbol(key)} {{ {fields} }} => {{"]
                if "count" in row["args"].values():
                    lines += ["                let formatter = cantos_localization::Formatter::try_new(locale.lang())?;"]
                for name, kind in sorted(row["args"].items()):
                    value = f"formatter.number(&arg_{name}.to_string())?" if kind == "count" else f"(*arg_{name}).to_owned()"
                    lines += [f"                let value_{name} = {value};"]
                lines += ["                Ok(match locale {"]
                for locale in [row for row in catalog["locales"] if include_test or not row["testOnly"]]:
                    lines += [f"                    Locale::{locale['symbol']} => {pattern_expression(row['translations'][locale['tag']], row)},"]
                lines += ["                })", "            },"]
            lines += ["        }", "    }", "}"]
        outputs[target] = "\n".join(lines) + "\n"
    outputs[Path("packages/localization/catalog/v1/manifest.json")] = json.dumps({"schemaVersion":1, "catalogSha256":digest, "icu4x":catalog["icu4x"], "cldr":catalog["cldr"], "locales":catalog["locales"], "messages":{key:row["args"] for key,row in sorted(catalog["messages"].items())}}, ensure_ascii=False, sort_keys=True, indent=2) + "\n"
    return outputs



def expected_outputs(root=ROOT):
    from localization_kotlin import projections as kotlin_projections, source, locale_source
    catalog = validate(load(root / SOURCE))
    outputs = projections(catalog)
    outputs.update(kotlin_projections(catalog))
    fixture = validate(load(root / "packages/localization/catalog/tests/third-locale.json"))
    fixture_source = projections(fixture, include_test=True)[TARGETS["studio"]]
    outputs[Path("packages/localization/rust/tests/generated/third_locale.rs")] = fixture_source
    # Named text-only projection of the shared Japanese fixture. Counts/plurals are explicitly
    # excluded here: the Kotlin/native ICU4X binding has not been qualified, not faked in tests.
    text_fixture = {**fixture, "messages": {key: row for key, row in fixture["messages"].items()
                                         if all(kind == "text" for kind in row["args"].values())}}
    fixture_package = "com.cantos.theatre.localization.fixture"
    fixture_root = Path("apps/cantos-theatre/mobile/shared/src/commonTest/kotlin/com/cantos/theatre/localization/fixture")
    header = "// Generated text-only test projection; not release resources or native formatter evidence.\n"
    outputs[fixture_root / "ThirdLocale.kt"] = header + locale_source(fixture, fixture_package, include_test=True)
    outputs[fixture_root / "ThirdLocaleResources.kt"] = header + source(text_fixture, {"probe": "Probe"}, fixture_package, fixture_package, include_test=True, locale_type="UiLocale")
    return outputs

def repository_failures(root=ROOT):
    if not (root / SOURCE).exists():
        return ["localization resources: canonical catalog missing"]
    try:
        return [f"localization resources: generated drift: {name}" for name, value in expected_outputs(root).items() if not (root / name).is_file() or (root / name).read_text() != value]
    except (ValueError, OSError) as error:
        return [f"localization resources: {error}"]


def write(root=ROOT):
    for name, value in expected_outputs(root).items():
        (root / name).parent.mkdir(parents=True, exist_ok=True)
        (root / name).write_text(value, encoding="utf-8")
