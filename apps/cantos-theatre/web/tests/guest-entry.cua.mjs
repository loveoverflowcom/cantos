/** Actual compiled Leptos browser checks. Controlled HTTP responses are allowed; no UI injection. */
export async function guestChrome(tab) {
  await tab.getAXState({ emit: false });
  const facts = await tab.playwright.evaluate(() => ({
    navigation: document.querySelectorAll('#theatre-navigation-rail, #theatre-rail-toggle, #theatre-navigation-menu, #theatre-navigation-drawer').length,
    overflow: document.body.style.overflow,
    mode: document.querySelector('.theatre-live-app')?.getAttribute('data-navigation'),
    audio: document.querySelectorAll('#theatre-audio-engine').length,
    preferences: document.querySelectorAll('.theatre-guest-preferences').length,
  }));
  if (facts.navigation !== 0 || facts.overflow === 'hidden' || facts.mode !== 'none') {
    throw Object.assign(new Error('Guest must have no application navigation or scroll lock'), { facts });
  }
  return { ...facts, url: await tab.url() };
}

// Shared scenario: cantos.presentation.guest-navigation.run.session-root
export async function guestPrivateEntry(tab, { origin }) {
  await tab.goto(`${origin}#library`);
  await tab.getAXState({ emit: false });
  await tab.playwright.locator('#theatre-landing').waitFor({ state: 'visible', timeoutMs: 20000 });
  const initial = await guestChrome(tab);
  if (!initial.url.endsWith('#landing')) throw new Error('Private guest entry must replace to Landing');
  await tab.playwright.locator('#theatre-home-login').click();
  await tab.getAXState({ emit: false });
  if (!(await tab.url()).endsWith('#login?from=library')) throw new Error('Typed Library intent was lost');
  await tab.playwright.locator('#theatre-auth-back').click();
  await tab.getAXState({ emit: false });
  await tab.playwright.locator('#theatre-landing').waitFor({ state: 'visible', timeoutMs: 20000 });
  return { scenario: 'cantos.presentation.guest-navigation.run.session-root', initial,
    cancelled: await guestChrome(tab), status: 'PASS' };
}

// Shared scenario: cantos.presentation.guest-navigation.run.repeated-navigation
export async function repeatedGuestAddress(tab, { workId }) {
  await tab.getAXState({ emit: false });
  await tab.playwright.locator(`#theatre-public-work-link-${workId}`).click();
  await tab.getAXState({ emit: false });
  await tab.playwright.locator('.theatre-topbar .theatre-brand').click();
  await tab.getAXState({ emit: false });
  await tab.playwright.locator('#theatre-landing').waitFor({state: 'visible', timeoutMs: 20000});
  const before = await tab.url();
  await tab.playwright.locator('.theatre-topbar .theatre-brand').click();
  await tab.getAXState({ emit: false });
  await tab.playwright.locator('.theatre-topbar .theatre-brand').click();
  await tab.getAXState({ emit: false });
  const after = await tab.url();
  if (after !== before) throw new Error('Repeated Landing navigation changed the address');
  await tab.back();
  await tab.getAXState({ emit: false });
  if (!(await tab.url()).endsWith(`#work/${workId}`)) {
    throw new Error('One Back after repeated Landing navigation must reach the preceding Work');
  }
  await tab.forward();
  await tab.getAXState({ emit: false });
  return { scenario: 'cantos.presentation.guest-navigation.run.repeated-navigation', before, after,
    chrome: await guestChrome(tab), status: 'PASS' };
}

/** Start at an unconfirmed logout dialog after the old account opener has been removed. */
export async function unconfirmedLogoutFocus(tab) {
  await tab.getAXState({emit:false});
  await tab.playwright.locator('#theatre-logout-cancel').click();
  await tab.getAXState({emit:false});
  await tab.playwright.locator('#theatre-main:focus').waitFor({state:'visible',timeoutMs:10000});
  return {scenario:'cantos.presentation.guest-navigation.run.logout-expiry', status:'PASS',
    focus:await tab.playwright.evaluate(()=>document.activeElement?.id), chrome:await guestChrome(tab)};
}
