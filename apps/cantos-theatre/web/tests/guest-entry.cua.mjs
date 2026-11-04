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

/** #691 header placement: actual selectors/callbacks, popup dismissal and stable hero bounds. */
export async function guestHeaderPreferences(tab) {
  await tab.getAXState({ emit: false });
  await tab.playwright.locator('#theatre-guest-theme').waitFor({ state: 'visible', timeoutMs: 20000 });
  const inspect = () => tab.playwright.evaluate(() => {
    const box = selector => document.querySelector(selector)?.getBoundingClientRect().toJSON();
    return {
      headerPreferences: document.querySelectorAll('header .theatre-guest-preferences').length,
      mainPreferences: document.querySelectorAll('main .theatre-guest-preferences').length,
      hero: box('#theatre-landing-heading'),
      locale: box('#theatre-guest-locale-trigger'),
      theme: box('#theatre-guest-theme'),
      ctas: [...document.querySelectorAll('.theatre-home-auth-actions button')].map(e => e.getBoundingClientRect().toJSON()),
      active: document.activeElement?.id,
      viewport: document.documentElement.clientWidth,
      scroll: document.documentElement.scrollWidth,
    };
  });
  const before = await inspect();
  const address = await tab.url();
  if (before.headerPreferences !== 1 || before.mainPreferences !== 0) {
    throw Object.assign(new Error('Preferences must be in the trailing header only'), { before });
  }
  for (const target of [before.locale, before.theme]) {
    if (!target || target.width < 47.5 || target.height < 47.5) throw new Error('Preference target smaller than 48 CSS px');
    if (target.left < 0 || target.right > before.viewport + .5) throw new Error('Header control exceeds the viewport');
  }
  await tab.playwright.locator('#theatre-guest-locale-trigger').click();
  await tab.getAXState({ emit: false });
  const opened = await inspect();
  if (before.hero && opened.hero?.y !== before.hero.y) throw new Error('Opening preferences shifted the hero');
  await tab.playwright.locator('#theatre-guest-locale-trigger').press('Escape');
  await tab.getAXState({ emit: false });
  const dismissed = await inspect();
  if (await tab.url() !== address) throw new Error('Chooser Escape cancelled the enclosing route');
  if (dismissed.active !== 'theatre-guest-locale-trigger') throw new Error('Escape lost the chooser focus');
  // Retain whole-page overflow as a separate observation: unchanged catalog content can
  // overflow at 200% root font even while every header target remains within the viewport.
  if (before.ctas.length === 2 && Math.abs(before.ctas[0].height - before.ctas[1].height) > .5) {
    throw Object.assign(new Error('Hero auth pair has unequal heights'), { before });
  }
  return { scenario: 'cantos.presentation.guest-navigation.run.preferences-player',
    before, opened, dismissed, chrome: await guestChrome(tab), status: 'PASS' };
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
