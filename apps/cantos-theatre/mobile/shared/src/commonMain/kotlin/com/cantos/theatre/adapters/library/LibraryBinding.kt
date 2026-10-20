package com.cantos.theatre.adapters.library

import com.cantos.theatre.adapters.library.wire.AudioIdentity
import com.cantos.theatre.adapters.library.wire.AudioPresentation

/** Schema validity does not say a presentation describes the row it rides on; this does. */
internal object LibraryBinding {
    /**
     * Continue/History metadata is for the exact stored publication: same Work, unit and release.
     * A null presentation binds to nothing and is always acceptable.
     */
    fun exact(audio: AudioIdentity, presentation: AudioPresentation?): Boolean = presentation == null ||
        (presentation.work.work_id == audio.work_id && presentation.unit_id == audio.unit_id &&
            presentation.work.publication_id == audio.publication_id)
}
