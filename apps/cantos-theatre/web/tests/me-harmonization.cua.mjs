/** Compiled Theatre Web with a signed-in fixture. Run from the Me page. */
export async function meLayout(tab) {
  const facts = await tab.playwright.evaluate(() => {
    const account = document.querySelector('#account');
    const details = account.querySelector('.theatre-account-details');
    const profile = details.querySelector('.theatre-account-profile');
    const activity = details.querySelector('#theatre-account-activity-heading').closest('.cui-surface');
    return {
      viewport: document.documentElement.clientWidth,
      scrollWidth: document.documentElement.scrollWidth,
      gap: activity.getBoundingClientRect().top - profile.getBoundingClientRect().bottom,
      rowGap: Number.parseFloat(getComputedStyle(details).rowGap),
      choosers: account.querySelectorAll('[role="combobox"]').length,
    };
  });
  if (Math.abs(facts.gap - facts.rowGap) > 1 || facts.scrollWidth > facts.viewport + 1 || facts.choosers !== 2) {
    throw Object.assign(new Error('Me groups or preference controls regressed'), { facts });
  }
  return facts;
}

async function assertFocus(tab, id, tag) {
  const target = tab.playwright.locator(`#${id}:focus`);
  await target.waitFor({ state: 'visible' });
  const facts = await tab.playwright.evaluate(() => {
    const element = document.activeElement;
    const box = element.getBoundingClientRect();
    return {
      id: element.id,
      tag: element.tagName,
      visible: box.bottom > 0 && box.top < document.documentElement.clientHeight,
      hash: location.hash,
    };
  });
  if (facts.id !== id || facts.tag !== tag || !facts.visible) {
    throw Object.assign(new Error('Navigation lost its focus destination'), { facts });
  }
  return facts;
}

async function backToMe(tab) {
  await tab.back();
  await tab.playwright.locator('#account').waitFor({ state: 'visible' });
  // The route has rendered, but its scheduled landmark focus may still be pending.
  await tab.playwright.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve(true)))));
}

export async function meShortcuts(tab) {
  const observations = [];
  for (const section of ['saved', 'history']) {
    const hash = section === 'saved' ? '#library' : '#history';
    const id = `theatre-library-${section}-heading`;
    await tab.playwright.locator(`#theatre-account-open-${section}`).press('Enter');
    const first = await assertFocus(tab, id, 'H2');
    await backToMe(tab);
    await tab.forward();
    const restored = await assertFocus(tab, id, 'H2');
    if (first.hash !== hash || restored.hash !== hash) throw new Error('Shortcut changed its route');
    observations.push({ section, first, restored });
    await backToMe(tab);
  }
  return observations;
}
