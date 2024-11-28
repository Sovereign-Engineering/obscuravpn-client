import { By } from 'selenium-webdriver';
import { driver } from './test.js';
import { expect } from 'chai';

describe('Home', () => {
  it('successfully loads ', () => { });

  it('allows toggling the theme', async () => {
    let bgBeforeToggle = await driver.findElement(By.css('.mantine-Header-root')).getCssValue('background-color');
    await driver.findElement(By.css('#toggle-theme')).click();
    expect(await driver.findElement(By.css('.mantine-Header-root')).getCssValue('background-color')).not.equal(bgBeforeToggle);
  }).timeout(10_000);
});
