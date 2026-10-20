package com.cantos.theatre.adapters.library

import com.cantos.theatre.adapters.library.wire.AudioPresentation
import com.cantos.theatre.adapters.library.wire.ContinueItem
import com.cantos.theatre.adapters.library.wire.ContinuePage
import com.cantos.theatre.adapters.library.wire.HistoryItem
import com.cantos.theatre.adapters.library.wire.HistoryPage
import com.cantos.theatre.adapters.library.wire.PresentedContinueItem
import com.cantos.theatre.adapters.library.wire.PresentedContinuePage
import com.cantos.theatre.adapters.library.wire.PresentedHistoryItem
import com.cantos.theatre.adapters.library.wire.PresentedHistoryPage
import com.cantos.theatre.adapters.library.wire.PresentedSavedItem
import com.cantos.theatre.adapters.library.wire.PresentedSavedPage
import com.cantos.theatre.adapters.library.wire.SavedItem
import com.cantos.theatre.adapters.library.wire.SavedPage
import com.cantos.theatre.adapters.library.wire.WorkPresentation

/** Test doubles build rows once and attach the API's required, nullable presentation per page read. */
internal fun ContinuePage.presented(presentation: (ContinueItem) -> AudioPresentation? = { null }) =
    PresentedContinuePage(items.map { PresentedContinueItem(it.progress, it.resume, presentation(it)) }, next_cursor)

internal fun SavedPage.presented(presentation: (SavedItem) -> WorkPresentation? = { null }) =
    PresentedSavedPage(items.map { PresentedSavedItem(it.saved, presentation(it)) }, next_cursor)

internal fun HistoryPage.presented(presentation: (HistoryItem) -> AudioPresentation? = { null }) =
    PresentedHistoryPage(state, items.map { PresentedHistoryItem(it.audio, it.started_at_ms, it.listener_order, it.progress, it.resume, presentation(it)) }, next_cursor)

internal fun presentationOf(work: String, publication: String, unit: String, workTitle: String, unitTitle: String, kind: String = "story") =
    AudioPresentation(WorkPresentation(work, publication, workTitle, kind, "vi-VN"), unit, unitTitle)
