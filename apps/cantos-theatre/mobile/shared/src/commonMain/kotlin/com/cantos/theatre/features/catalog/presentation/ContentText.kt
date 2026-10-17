package com.cantos.theatre.features.catalog.presentation

import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.intl.LocaleList

/** Preserve creator text exactly; the source language annotates shaping/assistive renderer text. */
internal fun contentText(value: String, contentLanguage: String): AnnotatedString {
    if (value.isEmpty() || contentLanguage.isBlank()) return AnnotatedString(value)
    return AnnotatedString(
        text = value,
        spanStyles = listOf(AnnotatedString.Range(SpanStyle(localeList = LocaleList(contentLanguage)), 0, value.length)),
    )
}
