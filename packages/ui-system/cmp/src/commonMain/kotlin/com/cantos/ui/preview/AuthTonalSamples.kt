package com.cantos.ui.preview

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import com.cantos.ui.AccessibleName
import com.cantos.ui.Button
import com.cantos.ui.ButtonState
import com.cantos.ui.ButtonVariant
import com.cantos.ui.CantosTypography
import com.cantos.ui.Checkbox
import com.cantos.ui.DisabledReason
import com.cantos.ui.FieldState
import com.cantos.ui.IconButton
import com.cantos.ui.TextField
import com.cantos.ui.TextFieldVariant
import com.cantos.ui.tonalAuthActions
import com.cantos.ui.tokenDp

private typealias T = PreviewToolCopy.AuthTonal

/**
 * Auth v4.3 Minimal Expressive recipe matrix: one title, tonal floating fields (idle, filled, error,
 * read-only, disabled, with an independent in-field eye), a primary action in each state and a quiet
 * route link. No state is carried by a stroke. Package-only anatomy, not an adopted auth screen.
 */
@Composable
internal fun AuthTonalSpecimen(modifier: Modifier = Modifier, onEvent: (String) -> Unit = {}) {
    CompositionLocalProvider(tonalAuthActions()) {
        Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.lg"))) {
            Text(vietnameseSpecimenText(T.Title), style = CantosTypography.authHeading)
            Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
                var email by remember { mutableStateOf("") }
                TextField(T.Email, email, { email = it }, Modifier.fillMaxWidth(),
                    variant = TextFieldVariant.FloatingTonal,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email))
                TextField(T.Email, T.EmailValue, {}, Modifier.fillMaxWidth(), variant = TextFieldVariant.FloatingTonal)
                TextField(T.Email, T.EmailInvalid, {}, Modifier.fillMaxWidth(), variant = TextFieldVariant.FloatingTonal,
                    error = T.EmailError)
                TonalPasswordRow(T.NewPassword, T.NewPasswordValue, null)
                TonalPasswordRow(T.RepeatPassword, T.RepeatPasswordValue, T.Mismatch)
                TextField(T.Email, T.EmailValue, {}, Modifier.fillMaxWidth(),
                    variant = TextFieldVariant.FloatingTonal, state = FieldState.ReadOnly)
                TextField(T.Email, T.EmailValue, {}, Modifier.fillMaxWidth(), variant = TextFieldVariant.FloatingTonal,
                    state = FieldState.Disabled(DisabledReason.of(T.Locked)))
            }
            Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                Checkbox(T.Confirmation, false, {})
                Checkbox(T.Confirmation, true, {})
                Checkbox(T.Confirmation, true, {}, state = ButtonState.Disabled(DisabledReason.of(T.ConfirmationPending)))
                Button(T.SignIn, { onEvent("tonal:primary") }, variant = ButtonVariant.AuthPrimary, icon = "login",
                    controlModifier = Modifier.fillMaxWidth())
                Button(T.SignIn, {}, variant = ButtonVariant.AuthPrimary, icon = "login",
                    state = ButtonState.Loading(DisabledReason.of(T.Pending)),
                    controlModifier = Modifier.fillMaxWidth())
                Button(T.SignIn, {}, variant = ButtonVariant.AuthPrimary, icon = "login",
                    state = ButtonState.Disabled(DisabledReason.of(T.Unavailable)),
                    controlModifier = Modifier.fillMaxWidth())
                Button(T.Route, { onEvent("tonal:route") }, variant = ButtonVariant.Text,
                    icon = "person_add", controlModifier = Modifier.fillMaxWidth())
            }
        }
    }
}

@Composable
private fun TonalPasswordRow(label: String, initial: String, error: String?) {
    var value by remember { mutableStateOf(initial) }
    var visible by remember { mutableStateOf(false) }
    TextField(label, value, { value = it }, Modifier.fillMaxWidth(), variant = TextFieldVariant.FloatingTonal, error = error,
        visualTransformation = if (visible) VisualTransformation.None else PasswordVisualTransformation(),
        trailing = {
            IconButton(AccessibleName.of(if (visible) T.HidePassword else T.ShowPassword),
                if (visible) "visibility_off" else "visibility", { visible = !visible })
        })
}
