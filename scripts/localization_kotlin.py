"""App-owned Kotlin text projections of the accepted v1 resource catalog.

This deliberately does not implement ICU4X formatting. Numeric consumers remain with their
existing owner until a real Kotlin/native binding is qualified (ADR0050).
"""
import hashlib
import json
from pathlib import Path

from localization_catalog import require, segments, symbol

MAIN = Path("apps/cantos-theatre/mobile/shared/src/commonMain/kotlin/com/cantos/theatre")
TARGETS = {MAIN / "domain/preferences/UiLocale.kt", MAIN / "presentation/model/ShellLocale.kt",
           MAIN / "app/LocaleProjection.kt", MAIN / "presentation/model/generated/TheatreResources.kt"}
GROUPS = {"cmp_shell": "Shell", "cmp_auth": "Auth", "cmp_membership": "Membership",
          "cmp_live": "Live", "cmp_launch": "Launch"}
KOTLIN_RESERVED = set("as break class continue do else false for fun if in interface is null object package return super this throw true try typealias typeof val var when while by catch constructor delegate dynamic field file finally get import init param property receiver set setparam where actual abstract annotation companion const crossinline data enum expect external final infix inline inner internal lateinit noinline open operator out override private protected public reified sealed suspend tailrec vararg".split())


def quoted(value):
    # Kotlin interpolation must never interpret catalog text (including currency signs).
    return json.dumps(value, ensure_ascii=False).replace("$", r"\$")


def locale_source(catalog, package="com.cantos.theatre.domain.preferences", include_test=False,
                  type_name="UiLocale", saved=True):
    locales = [row for row in catalog["locales"] if include_test or not row["testOnly"]]
    require(all(row["symbol"] not in {"Default", "Companion", "UiLocale"} for row in locales), "Kotlin reserved locale symbol")
    lines = [f"package {package}", "", "/** UI resources only; content BCP-47 tags never pass through this registry. */",
             f"enum class {type_name}(val tag: String, val autonym: String, val direction: String) {{"]
    lines += [f"    {row['symbol']}({quoted(row['tag'])}, {quoted(row['autonym'])}, {quoted(row['dir'])})," for row in locales]
    fallback = next(row["symbol"] for row in locales if row["tag"] == catalog["fallback"])
    lines += ["    ;", "", "    companion object {", f"        val Default: {type_name} = {fallback}", "    }", "}", ""]
    if saved:
        lines += ["/** Canonical persisted values only: no OS negotiation or silent string-to-English mapping. */",
                  f"fun savedUiLocale(tag: String): {type_name}? = {type_name}.entries.firstOrNull {{ it.tag == tag }}", ""]
    return "\n".join(lines)


def source(catalog, groups=GROUPS, package="com.cantos.theatre.presentation.model.generated",
           locale_package="com.cantos.theatre.presentation.model", include_test=False, locale_type="ShellLocale"):
    for key in catalog["messages"]:
        owner, group, _ = key.split(".")
        if group.startswith("cmp_"):
            require(owner == "theatre" and group in GROUPS, f"Kotlin unbound namespace: {key}")
    locales = [row for row in catalog["locales"] if include_test or not row["testOnly"]]
    lines = [f"package {package}", "", f"import {locale_package}.{locale_type}", ""]
    for group, prefix in groups.items():
        messages = {key: row for key, row in catalog["messages"].items() if key.split(".")[1] == group}
        if not messages:
            continue
        for key, row in messages.items():
            require(not set(row["args"]) & KOTLIN_RESERVED, f"Kotlin reserved argument: {key}")
            require(all(kind == "text" for kind in row["args"].values()), f"Kotlin formatter unqualified: {key}")
            require(all("text" in pattern for pattern in row["translations"].values()), f"Kotlin plural unqualified: {key}")
        plain = {key: row for key, row in messages.items() if not row["args"]}
        if plain:
            lines += [f"enum class {prefix}Key {{"]
            lines += [f"    {symbol(key.split('.')[-1])}," for key in sorted(plain)]
            lines += ["    ;", "", f"    fun text(locale: {locale_type}): String = when (this) {{"]
            for key, row in sorted(plain.items()):
                lines += [f"        {symbol(key.split('.')[-1])} -> when (locale) {{"]
                lines += [f"            {locale_type}.{locale['symbol']} -> {quoted(row['translations'][locale['tag']]['text'])}" for locale in locales]
                lines += ["        }"]
            lines += ["    }", "}", ""]
        parameterized = {key: row for key, row in messages.items() if row["args"]}
        if parameterized:
            lines += [f"sealed interface {prefix}Message {{", f"    fun text(locale: {locale_type}): String", ""]
            for key, row in sorted(parameterized.items()):
                fields = ", ".join(f"val {name}: String" for name in sorted(row["args"]))
                lines += [f"    data class {symbol(key.split('.')[-1])}({fields}) : {prefix}Message {{",
                          f"        override fun text(locale: {locale_type}): String = when (locale) {{"]
                for locale in locales:
                    pieces = [quoted(value) if kind == "text" else value for kind, value in segments(row["translations"][locale["tag"]]["text"])]
                    # Start with a string even if the message consists solely of arguments.
                    expression = '"" + ' + " + ".join(pieces)
                    lines += [f"            {locale_type}.{locale['symbol']} -> {expression}"]
                lines += ["        }", "    }", ""]
            lines += ["}", ""]
    return "\n".join(lines)


def projections(catalog):
    digest = hashlib.sha256(json.dumps(catalog, sort_keys=True, ensure_ascii=False).encode()).hexdigest()
    header = f"// Generated by scripts/localization_catalog.py --resources; DO NOT EDIT. sha256: {digest}\n"
    locales = [row for row in catalog["locales"] if not row["testOnly"]]
    mapping = ["package com.cantos.theatre.app", "",
               "import com.cantos.theatre.application.preferences.PreferenceLocale",
               "import com.cantos.theatre.presentation.model.ShellLocale", "",
               "/** Read-only projection of the single preference authority; generated from the registry. */",
               "fun PreferenceLocale.toShellLocale(): ShellLocale = when (this) {"]
    mapping += [f"    PreferenceLocale.{row['symbol']} -> ShellLocale.{row['symbol']}" for row in locales]
    mapping += ["}", "", "fun ShellLocale.toPreferenceLocale(): PreferenceLocale = when (this) {"]
    mapping += [f"    ShellLocale.{row['symbol']} -> PreferenceLocale.{row['symbol']}" for row in locales]
    mapping += ["}", ""]
    return {
        MAIN / "domain/preferences/UiLocale.kt": header + locale_source(catalog),
        MAIN / "presentation/model/ShellLocale.kt": header + locale_source(catalog, "com.cantos.theatre.presentation.model", type_name="ShellLocale", saved=False),
        MAIN / "app/LocaleProjection.kt": header + "\n".join(mapping),
        MAIN / "presentation/model/generated/TheatreResources.kt": header + source(catalog),
    }
