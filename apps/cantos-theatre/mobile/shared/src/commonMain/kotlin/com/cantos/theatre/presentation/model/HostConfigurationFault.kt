package com.cantos.theatre.presentation.model

/**
 * Why a native host could not compose the live app (#298). A typed diagnostic the presentation
 * can name, never a raw exception: the host fails closed before any request, keeps no mock and
 * never selects another environment.
 */
enum class HostConfigurationFault {
    OriginEmpty,
    OriginMalformed,
    OriginNotHttps,
    OriginCleartextHostNotAllowed,
    /** The packaging declares no network capability, so no API request can ever be sent. */
    NetworkNotDeclared,
}
