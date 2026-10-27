package com.cantos.theatre.features.membership.presentation

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import com.cantos.theatre.components.TopBarThemeButton
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.ui.AccessibleName
import com.cantos.ui.BackButton
import com.cantos.ui.Badge
import com.cantos.ui.Button
import com.cantos.ui.ButtonState
import com.cantos.ui.ButtonVariant
import com.cantos.ui.CantosTypography
import com.cantos.ui.DisabledReason
import com.cantos.ui.Logo
import com.cantos.ui.LogoTone
import com.cantos.ui.Space
import com.cantos.ui.StatusTone
import com.cantos.ui.Surface
import com.cantos.ui.SurfaceTone
import com.cantos.ui.SurfaceVariant
import com.cantos.ui.tokenColor
import com.cantos.ui.tokenDp
import com.cantos.ui.rememberCantosIconVector

/** App composition over shared design-v2 primitives; no Membership decisions or state owner. */
@Composable
fun MembershipHeader(
    locale: ShellLocale,
    theme: ShellTheme,
    onBack: () -> Unit,
    onToggleTheme: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val copy = MembershipCopy(locale)
    Row(
        modifier.fillMaxWidth().padding(horizontal = tokenDp("space.section"), vertical = tokenDp("space.sm")),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(tokenDp("space.inset")),
    ) {
        BackButton(AccessibleName.of(copy.text(MembershipText.Back)), onBack, controlModifier = Modifier.testTag("back"))
        Box(Modifier.weight(1f)) {
            Logo(
                modifier = Modifier.width(tokenDp("size.authLogo")).heightIn(max = tokenDp("size.backControl")).testTag("header-title"),
                tone = if (theme == ShellTheme.Dark) LogoTone.Dark else LogoTone.Light,
                accessibleName = AccessibleName.of(copy.text(MembershipText.AppBrand)),
            )
        }
        TopBarThemeButton(
            theme, copy.text(MembershipText.ToggleTheme), onToggleTheme,
            Modifier.testTag("membership-theme"),
        )
    }
}

@Composable
internal fun MembershipHeading(copy: MembershipCopy, title: MembershipText, body: MembershipText, eyebrow: MembershipText = MembershipText.Brand) {
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
        Text(copy.text(eyebrow).uppercase(), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.tertiary)
        Text(copy.text(title), modifier = Modifier.testTag("membership-heading").semantics { heading() }, style = CantosTypography.authHeading)
        Text(copy.text(body), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
internal fun MembershipSuccessSymbol() {
    androidx.compose.material3.Surface(
        modifier = Modifier.size(tokenDp("size.statusSymbol")),
        color = tokenColor("color.successContainer"),
        contentColor = tokenColor("color.success"),
        shape = RoundedCornerShape(tokenDp("shape.statusSymbol")),
    ) {
        Box(contentAlignment = Alignment.Center) { Icon(rememberCantosIconVector("check"), null, Modifier.size(tokenDp("size.authIcon"))) }
    }
}

@Composable
internal fun MembershipReceipt(copy: MembershipCopy, email: String?) {
    MembershipCard {
        Badge(copy.text(MembershipText.ReceiptBadge), tone = StatusTone.Success)
        Text(copy.text(MembershipText.Brand), style = MaterialTheme.typography.labelLarge)
        email?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant) }
        Text(copy.text(MembershipText.ReceivedFixture), style = MaterialTheme.typography.bodySmall, modifier = Modifier.testTag("membership-receipt"))
    }
}

internal enum class MembershipPeriod(
    val label: MembershipText,
    val title: MembershipText,
    val body: MembershipText,
    val rate: MembershipText,
    val action: MembershipText,
    val tone: SurfaceTone,
    val actionVariant: ButtonVariant,
) {
    Monthly(MembershipText.Monthly, MembershipText.MonthTitle, MembershipText.MonthBody, MembershipText.MonthRate,
        MembershipText.MonthAction, SurfaceTone.Surface, ButtonVariant.AuthSecondary),
    Annual(MembershipText.Annual, MembershipText.YearTitle, MembershipText.YearBody, MembershipText.YearRate,
        MembershipText.YearAction, SurfaceTone.PrimaryContainer, ButtonVariant.AuthPrimary),
}

/** Proposed periods remain unavailable; displaying a card never grants a plan or paid access. */
@Composable
internal fun MembershipPlanCard(copy: MembershipCopy, period: MembershipPeriod, modifier: Modifier = Modifier) {
    Surface(modifier.fillMaxWidth().testTag("membership-period-${period.name}"), tone = period.tone,
        variant = SurfaceVariant.Membership, inset = Space.Lg) {
        Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.lg"))) {
            Badge(copy.text(period.label), tone = StatusTone.Info)
            Text(copy.text(period.title), style = CantosTypography.membershipTitle)
            Text(copy.text(period.body), style = MaterialTheme.typography.bodyMedium)
            Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                Text(copy.text(MembershipText.PriceUnpublished), style = MaterialTheme.typography.titleLarge)
                Text(copy.text(period.rate), style = MaterialTheme.typography.bodySmall)
            }
            Button(copy.text(period.action), onPress = {}, variant = period.actionVariant,
                state = ButtonState.Disabled(DisabledReason.of(copy.text(MembershipText.PlanDetailsUnavailable))),
                controlModifier = Modifier.fillMaxWidth())
        }
    }
}

@Composable
internal fun MembershipCard(content: @Composable () -> Unit) {
    Surface(Modifier.fillMaxWidth(), tone = SurfaceTone.Surface, variant = SurfaceVariant.Membership, inset = Space.Lg) {
        Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) { content() }
    }
}

@Composable
internal fun MembershipAction(
    label: String,
    tag: String,
    variant: ButtonVariant = ButtonVariant.AuthPrimary,
    state: ButtonState = ButtonState.Enabled,
    onPress: () -> Unit,
) {
    Button(label, onPress, variant = variant, state = state,
        icon = if (variant == ButtonVariant.AuthPrimary) "arrow_forward" else null,
        controlModifier = Modifier.fillMaxWidth().testTag(tag))
}
