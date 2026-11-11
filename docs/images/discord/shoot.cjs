// Draws the Discord connector mockups (mockups.html) into the PNGs next to
// it: `node docs/images/discord/shoot.cjs` with Playwright's Chromium.
const { chromium } = require('playwright');

const shots = {
  s1: '1-conversation',
  s2: '2-permission-question',
  s3: '3-questions-modal',
  s4: '4-plan',
  s5: '5-delivery-dashboard',
  s6: '6-node-worker',
};

(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 1000 } });
  await page.goto('file://' + __dirname + '/mockups.html');
  for (const [id, name] of Object.entries(shots)) {
    await page.locator('#' + id + ' .window').screenshot({ path: `${__dirname}/${name}.png` });
  }
  await browser.close();
})();
