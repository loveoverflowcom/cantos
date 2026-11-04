/** Compiled Leptos, explicit auth-preview fixture; never production auth or playback proof. */
export async function meLayout(tab) {
  await tab.getAXState({ emit: false });
  const facts = await tab.playwright.evaluate(() => {
    const box = selector => document.querySelector(selector)?.getBoundingClientRect().toJSON();
    const account = document.querySelector('#account');
    return {
      viewport: document.documentElement.clientWidth,
      accountWidth: account?.getBoundingClientRect().width,
      accountScroll: account?.scrollWidth,
      profile: box('#account .theatre-account-profile'),
      activity: box('#account .theatre-account-activity'),
      membership: box('#account .theatre-account-membership'),
      gap: +getComputedStyle(document.querySelector('.theatre-account-listening')).rowGap.replace('px', ''),
      choosers: document.querySelectorAll('#account [role="combobox"]').length,
      alias: document.querySelector('.theatre-account-alias')?.textContent,
    };
  });
  if (!facts.profile || !facts.activity || !facts.membership) throw new Error('Signed-in Me fixture required');
  if (Math.abs(facts.activity.top - facts.profile.bottom - facts.gap) > 1) {
    throw Object.assign(new Error('Premium must not add a row-height gap below Profile'), { facts });
  }
  if (facts.accountScroll > facts.accountWidth + 1 || facts.choosers !== 2) {
    throw Object.assign(new Error('Me overflows or duplicates preference controls'), { facts });
  }
  return facts;
}

export async function meShortcuts(tab) {
  const observations = [];
  for (const section of ['saved', 'history']) {
    await tab.playwright.locator(`#theatre-account-open-${section}`).press('Enter');
    await tab.getAXState({ emit: false });
    const facts = await tab.playwright.evaluate(() => ({
      active: document.activeElement?.id,
      heading: document.activeElement?.tagName,
      visible: document.activeElement?.getBoundingClientRect().bottom > 0 && document.activeElement?.getBoundingClientRect().top < document.documentElement.clientHeight,
      libraryCurrent: !!document.querySelector('#theatre-navigation-rail a[href="#library"][aria-current="page"]'),
    }));
    if (facts.active !== `theatre-library-${section}-heading` || facts.heading !== 'H2' || !facts.visible) {
      throw Object.assign(new Error('Shortcut lost its section destination'), { section, facts });
    }
    observations.push({ section, facts, url: await tab.url() });
    await tab.back();
    await tab.getAXState({ emit: false });
  }
  await tab.forward();
  await tab.getAXState({ emit: false });
  const restored = await tab.playwright.evaluate(() => document.activeElement?.id);
  if (restored !== 'theatre-library-history-heading') throw new Error('Forward lost the History heading');
  await tab.back();
  await tab.getAXState({ emit: false });
  return { observations, restored };
}
