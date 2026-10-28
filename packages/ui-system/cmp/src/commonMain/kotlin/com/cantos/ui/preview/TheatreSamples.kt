package com.cantos.ui.preview

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import com.cantos.ui.AccessibleName
import com.cantos.ui.Avatar
import com.cantos.ui.AvatarSize
import com.cantos.ui.Button
import com.cantos.ui.ButtonState
import com.cantos.ui.ButtonVariant
import com.cantos.ui.CantosExpression
import com.cantos.ui.CantosShapes
import com.cantos.ui.CantosTheme
import com.cantos.ui.DisabledReason
import com.cantos.ui.FieldHelp
import com.cantos.ui.FieldState
import com.cantos.ui.FilterChip
import com.cantos.ui.IconButton
import com.cantos.ui.Logo
import com.cantos.ui.LogoTone
import com.cantos.ui.LogoVariant
import com.cantos.ui.LocalUiPreferences
import com.cantos.ui.PreviewExample
import com.cantos.ui.Select
import com.cantos.ui.TextField
import com.cantos.ui.TextFieldVariant
import com.cantos.ui.Theme
import com.cantos.ui.parseTokenColor
import com.cantos.ui.tokenColor
import com.cantos.ui.tokenDp
import com.cantos.ui.generated.CantosTokens

/** Registered Theatre-expression states all start with this prefix; the dispatch in PreviewApp.kt keys on it. */
internal const val TheatreStatePrefix = "theatre-"

/**
 * Theatre expression specimens (token v0.7.0): the production components under an explicit
 * expression scope, with local state only. Nothing here calls a service, stores a secret or
 * implies an app composition; the surrounding gallery keeps its own expression.
 */
@Composable
internal fun TheatreSpecimen(example: PreviewExample, state: String, onEvent: (String) -> Unit, modifier: Modifier) {
    val outer = LocalUiPreferences.current
    CantosTheme(outer.copy(expression = CantosExpression.Theatre)) {
        Column(
            modifier.fillMaxWidth().background(tokenColor("color.background"), CantosShapes.container).padding(tokenDp("space.md")),
            verticalArrangement = Arrangement.spacedBy(tokenDp("space.md")),
        ) {
            when (example) {
                PreviewExample.UiRoot -> ExpressionSpecimen()
                PreviewExample.Button -> GroupSpecimen(onEvent)
                PreviewExample.IconButton -> if (state == "theatre-docked") DockedSpecimen(onEvent) else TonalSpecimen(onEvent)
                PreviewExample.FilterChip -> SelectionSpecimen()
                PreviewExample.TextField -> when (state) {
                    "theatre-trailing-eye" -> EyeSpecimen()
                    "theatre-docked-search" -> SearchSpecimen(onEvent)
                    else -> FieldMatrixSpecimen()
                }
                PreviewExample.Select -> UtilitySelectSpecimen()
                PreviewExample.Avatar -> AvatarSpecimen()
                PreviewExample.Logo -> HeaderSpecimen(outer.theme, onEvent)
                else -> FieldHelp(PreviewToolCopy.theatreMissing(example.id, state))
            }
        }
    }
}

@Composable
private fun ExpressionSpecimen() {
    val hero = tokenColor("color.theatreHero")
    val ink = tokenColor("color.theatreOnHero")
    Column(
        Modifier.fillMaxWidth().background(hero, CantosShapes.container).padding(tokenDp("space.lg")),
        verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
    ) {
        Text(vietnameseSpecimenText(PreviewToolCopy.TheatreFixture.HeroTitle), color = ink, style = MaterialTheme.typography.headlineMedium)
        Text(vietnameseSpecimenText(PreviewToolCopy.TheatreFixture.HeroSubtitle), color = ink, style = MaterialTheme.typography.bodyLarge)
    }
    FieldHelp(
        PreviewToolCopy.theatreHero(CantosTokens.number("type.theatreHeading.weight").toInt()),
    )
}

@Composable
private fun GroupSpecimen(onEvent: (String) -> Unit) {
    val reason = DisabledReason.of(PreviewToolCopy.TheatreFixture.Sending)
    FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Button(PreviewToolCopy.TheatreFixture.SignIn, { onEvent(PreviewToolCopy.TheatreSignInEvent) }, icon = "login")
        Button(PreviewToolCopy.TheatreFixture.SignUp, { onEvent(PreviewToolCopy.TheatreSignUpEvent) }, variant = ButtonVariant.Tonal, icon = "person_add")
        Button(PreviewToolCopy.TheatreFixture.Confirm, {}, state = ButtonState.Loading(reason), icon = "check")
    }
    Button(PreviewToolCopy.TheatreFixture.LongAction, { onEvent(PreviewToolCopy.TheatreLongActionEvent) }, icon = "mail", modifier = Modifier.fillMaxWidth())
}

@Composable
private fun TonalSpecimen(onEvent: (String) -> Unit) {
    var dark by remember { mutableStateOf(false) }
    IconButton(
        AccessibleName.of(if (dark) PreviewToolCopy.TheatreFixture.SwitchLight else PreviewToolCopy.TheatreFixture.SwitchDark),
        if (dark) "light_mode" else "dark_mode",
        { dark = !dark; onEvent(PreviewToolCopy.theatreTheme(dark)) },
        variant = ButtonVariant.Tonal,
    )
    FieldHelp(PreviewToolCopy.TheatreTonalHelp)
}

@Composable
private fun DockedSpecimen(onEvent: (String) -> Unit) {
    IconButton(
        AccessibleName.of(PreviewToolCopy.TheatreFixture.Search), "search", { onEvent(PreviewToolCopy.TheatreDockedEvent) },
        variant = ButtonVariant.Primary, circular = true, controlModifier = Modifier.size(tokenDp("size.searchAction")),
    )
    FieldHelp(PreviewToolCopy.TheatreDockedHelp)
}

@Composable
private fun SelectionSpecimen() {
    var all by remember { mutableStateOf(true) }
    var novel by remember { mutableStateOf(false) }
    FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        FilterChip(PreviewToolCopy.TheatreFixture.AllTypes, all, { all = it })
        FilterChip(PreviewToolCopy.TheatreFixture.Novel, novel, { novel = it })
        FilterChip(PreviewToolCopy.TheatreFixture.LongChip, false, {},
            state = ButtonState.Disabled(DisabledReason.of(PreviewToolCopy.TheatreFixture.NoContent)))
    }
}

@Composable
private fun FieldMatrixSpecimen() {
    var idle by remember { mutableStateOf("") }
    var filled by remember { mutableStateOf(PreviewToolCopy.TheatreFixture.EmailValue) }
    var invalid by remember { mutableStateOf(PreviewToolCopy.TheatreFixture.EmailInvalid) }
    var long by remember { mutableStateOf(PreviewToolCopy.TheatreFixture.LongValue) }
    val focus = remember { FocusRequester() }
    TextField(PreviewToolCopy.TheatreFixture.Email, idle, { idle = it }, variant = TextFieldVariant.Floating,
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email))
    TextField(PreviewToolCopy.TheatreFixture.Email, filled, { filled = it }, variant = TextFieldVariant.Floating,
        controlModifier = Modifier.focusRequester(focus))
    TextField(PreviewToolCopy.TheatreFixture.Email, invalid, { invalid = it }, variant = TextFieldVariant.Floating, error = PreviewToolCopy.TheatreFixture.EmailError)
    TextField(PreviewToolCopy.TheatreFixture.Email, PreviewToolCopy.TheatreFixture.EmailValue, {}, variant = TextFieldVariant.Floating, state = FieldState.ReadOnly)
    TextField(PreviewToolCopy.TheatreFixture.Email, PreviewToolCopy.TheatreFixture.EmailValue, {}, variant = TextFieldVariant.Floating,
        state = FieldState.Disabled(DisabledReason.of(PreviewToolCopy.TheatreFixture.Unavailable)))
    TextField(PreviewToolCopy.TheatreFixture.Email, PreviewToolCopy.TheatreFixture.EmailValue, {}, variant = TextFieldVariant.Floating,
        state = FieldState.Loading(DisabledReason.of(PreviewToolCopy.TheatreFixture.Loading)))
    TextField(PreviewToolCopy.TheatreFixture.LongLabel, long, { long = it },
        variant = TextFieldVariant.Floating, help = PreviewToolCopy.TheatreFixture.LongHelp)
    FieldHelp(PreviewToolCopy.TheatreFieldHelp)
}

@Composable
private fun PasswordRow(label: String, initial: String, error: String?) {
    var value by remember { mutableStateOf(initial) }
    var visible by remember { mutableStateOf(false) }
    TextField(label, value, { value = it }, variant = TextFieldVariant.Floating, error = error,
        help = if (error == null) PreviewToolCopy.TheatreFixture.PasswordHelp else null,
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions.Default,
        visualTransformation = if (visible) VisualTransformation.None else PasswordVisualTransformation(),
        trailing = {
            IconButton(
                AccessibleName.of(if (visible) PreviewToolCopy.TheatreFixture.HidePassword else PreviewToolCopy.TheatreFixture.ShowPassword),
                if (visible) "visibility_off" else "visibility",
                { visible = !visible },
            )
        })
}

@Composable
private fun EyeSpecimen() {
    PasswordRow(PreviewToolCopy.TheatreFixture.NewPassword, PreviewToolCopy.TheatreFixture.PasswordValue, null)
    PasswordRow(PreviewToolCopy.TheatreFixture.RepeatPassword, PreviewToolCopy.TheatreFixture.RepeatValue, PreviewToolCopy.TheatreFixture.Mismatch)
    FieldHelp(PreviewToolCopy.TheatreEyeHelp)
}

@Composable
private fun SearchSpecimen(onEvent: (String) -> Unit) {
    var query by remember { mutableStateOf("") }
    var expanded by remember { mutableStateOf(false) }
    TextField(PreviewToolCopy.TheatreFixture.Search, query, { query = it; expanded = true }, variant = TextFieldVariant.FloatingSearch,
        controlModifier = Modifier.onFocusChanged { if (it.isFocused) expanded = true },
        dockExpanded = expanded, dockContent = {
            Button(PreviewToolCopy.TheatreCollapse, { expanded = false })
            Text(PreviewToolCopy.TheatreFixture.Search)
        },
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        keyboardActions = KeyboardActions(onSearch = { onEvent(PreviewToolCopy.theatreSearch(query, ime = true)) }),
        trailing = {
            IconButton(
                AccessibleName.of(PreviewToolCopy.TheatreFixture.Search), "search", { onEvent(PreviewToolCopy.theatreSearch(query, ime = false)) },
                variant = ButtonVariant.Primary, circular = true, controlModifier = Modifier.size(tokenDp("size.searchAction")),
            )
        })
    FieldHelp(PreviewToolCopy.TheatreSearchHelp)
}

@Composable
private fun UtilitySelectSpecimen() {
    var language by remember { mutableStateOf(0) }
    Select(PreviewToolCopy.TheatreFixture.Language, listOf(PreviewToolCopy.TheatreFixture.Vietnamese, PreviewToolCopy.TheatreFixture.English), language, { language = it },
        expandedDescription = PreviewToolCopy.TheatreFixture.LanguagesOpen, collapsedDescription = PreviewToolCopy.TheatreFixture.LanguagesClosed)
}

@Composable
private fun AvatarSpecimen() {
    Row(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.md")), verticalAlignment = Alignment.CenterVertically) {
        Avatar(PreviewToolCopy.TheatreFixture.Brand, size = AvatarSize.Control, accent = true, accessibleName = AccessibleName.of(PreviewToolCopy.TheatreFixture.Account))
        Avatar(PreviewToolCopy.TheatreFixture.Identity, size = AvatarSize.Control)
    }
    FieldHelp(PreviewToolCopy.TheatreAvatarHelp)
}

@Composable
private fun HeaderSpecimen(theme: Theme, onEvent: (String) -> Unit) {
    FlowRow(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalArrangement = Arrangement.spacedBy(tokenDp("space.md")),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Logo(LogoVariant.Symbol, Modifier.size(tokenDp("size.brandMark")),
                tone = if (theme == Theme.Dark) LogoTone.Dark else LogoTone.Light)
            Spacer(Modifier.width(tokenDp("space.inset")))
            Text(PreviewToolCopy.TheatreFixture.Brand, style = MaterialTheme.typography.titleLarge)
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            IconButton(AccessibleName.of(PreviewToolCopy.TheatreFixture.SwitchTheme), if (theme == Theme.Dark) "light_mode" else "dark_mode",
                { onEvent(PreviewToolCopy.TheatreThemeEvent) }, variant = ButtonVariant.Tonal)
            Spacer(Modifier.width(tokenDp("space.inset")))
            Avatar(PreviewToolCopy.TheatreFixture.Brand, size = AvatarSize.Control, accent = true,
                accessibleName = AccessibleName.of(PreviewToolCopy.TheatreFixture.Account))
        }
    }
    FieldHelp(PreviewToolCopy.TheatreHeaderHelp)
}
