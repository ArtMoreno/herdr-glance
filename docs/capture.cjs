// Capture the actual ratatui fixture buffers exported by the snapshot test.
// PLAYWRIGHT_PATH may point at an existing Playwright installation.
const { chromium } = require(process.env.PLAYWRIGHT_PATH || 'playwright');
const fs = require('node:fs');
const path = require('node:path');
(async () => {
  const browser = await chromium.launch({ headless: true, ...(process.env.CHROME_PATH ? { executablePath: process.env.CHROME_PATH } : {}) });
  try {
    const page = await browser.newPage({ viewport: { width: 1000, height: 680 }, deviceScaleFactor: 1 });
    for (const theme of ['catppuccin-mocha', 'dracula', 'tokyo-night', 'nord', 'gruvbox']) {
      const base = path.join(__dirname, 'screenshots', theme);
      await page.setContent('<html><body style="margin:0">' + fs.readFileSync(base + '.svg', 'utf8') + '</body></html>');
      await page.screenshot({ path: base + '.png' });
    }
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
