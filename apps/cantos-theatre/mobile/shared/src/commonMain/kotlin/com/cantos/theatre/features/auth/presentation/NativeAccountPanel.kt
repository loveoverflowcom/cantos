package com.cantos.theatre.features.auth.presentation

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import com.cantos.theatre.application.identity.AccountView
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.application.identity.toAccountView
import com.cantos.theatre.presentation.model.ShellLocale

/** Production account controls borrow the process owner; navigation never replaces it. */
@Composable
fun NativeAccountPanel(controller: NativeAccountController?, locale: ShellLocale, onOpenLibrary: () -> Unit) {
    val state = controller?.state?.collectAsState()?.value
    val account = state.toAccountView()
    val copy = AuthCopy(locale)
    Column(Modifier.fillMaxWidth().testTag("native-account"), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(copy.text(if (account == AccountView.SignedIn) AuthText.SignedHeading else AuthText.GuestHeading))
        AccountStatus(account, copy)
        when (account) {
            AccountView.SignedIn -> {
                Button(onOpenLibrary, modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp).testTag("native-open-library")) { Text(copy.text(AuthText.OpenLibrary)) }
                TextButton({ controller?.logout() }, modifier = Modifier.heightIn(min = 48.dp).testTag("native-logout")) { Text(copy.text(AuthText.Logout)) }
            }
            AccountView.Pending -> TextButton({ controller?.cancelLogin() }, modifier = Modifier.heightIn(min = 48.dp).testTag("native-cancel-login")) { Text(copy.text(AuthText.CancelLogin)) }
            AccountView.CleanupRequired -> Button({ controller?.logout() }, modifier = Modifier.heightIn(min = 48.dp).testTag("native-retry-cleanup")) { Text(copy.text(AuthText.RetryCleanup)) }
            AccountView.Unavailable -> Text(copy.text(AuthText.UnavailableReason))
            AccountView.SignedOut, is AccountView.Failed -> {
                Text(copy.text(AuthText.ProviderHelp))
                Button({ controller?.beginLogin() }, modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp).testTag("native-begin-login")) { Text(copy.text(AuthText.CantosIdLogin)) }
                TextButton({ controller?.restore() }, modifier = Modifier.heightIn(min = 48.dp).testTag("native-restore-session")) { Text(copy.text(AuthText.RestoreSession)) }
                Text(copy.text(AuthText.SignupUnavailable), modifier = Modifier.testTag("native-registration-unavailable"))
            }
        }
    }
}
