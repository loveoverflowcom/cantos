package com.cantos.theatre.domain.identity

/** Server-generated pseudonym. Never a verified personal name, credential or actor identifier. */
data class AccountPresentation(val alias: String, val avatarSeed: String) {
    init {
        require(alias.isNotBlank() && alias.length <= 64 && alias.none(Char::isISOControl))
        require(avatarSeed.length == 64 && avatarSeed.all { it in '0'..'9' || it in 'a'..'f' })
    }
}
