package com.cantos.ui.preview

import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.intl.LocaleList

/** Fixed Vietnamese typography specimen, independent from English developer controls. */
internal fun vietnameseSpecimenText(value: String): AnnotatedString = AnnotatedString(
    text = value,
    spanStyles = if (value.isEmpty()) emptyList() else listOf(
        AnnotatedString.Range(SpanStyle(localeList = LocaleList("vi-VN")), 0, value.length),
    ),
)
