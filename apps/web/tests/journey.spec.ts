import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

test('decision, history, counterfactual and report remain consistent', async ({page}) => {
  await page.goto('/record/');
  await expect(page.getByRole('button',{name:'Approve change A'})).toBeEnabled();
  await page.getByRole('button',{name:'Dismiss D'}).click();
  await expect(page.getByText('The current standard stays.')).toBeVisible();
  await expect(page.getByText('0 STALE')).toBeVisible();
  await page.getByRole('button',{name:'Compare paths'}).first().click();
  await expect(page.getByRole('dialog')).toContainText('Hypothetical approval');
  await page.getByRole('button',{name:'Close comparison'}).click();
  page.once('dialog', dialog=>dialog.accept());
  await page.getByRole('button',{name:'Reset scenario'}).click();
  await page.getByRole('button',{name:'Approve change A'}).click();
  await expect(page.getByText('8 STALE')).toBeVisible();
  await expect(page.getByText('R-AFTER-HIRE').first()).toBeVisible();
  await expect(page.getByText('R-REJECTS-MET').first()).toBeVisible();
  await expect(page.getByText(/Its rubric stays on standard/)).toContainText('A');
  await page.getByRole('link',{name:'View report'}).click();
  await expect(page.getByRole('heading',{name:'Integrity details'})).toBeVisible();
  await expect(page.getByText(/verify the JSON separately/)).toBeVisible();
});

test('a second visitor gets a separate scenario', async ({browser,baseURL})=>{
  const one=await browser.newContext();const two=await browser.newContext();
  const a=await one.newPage();const b=await two.newPage();
  await a.goto(`${baseURL}/record/`);await b.goto(`${baseURL}/record/`);
  await a.getByRole('button',{name:'Approve change A'}).click();
  await expect(a.getByText('8 STALE')).toBeVisible();
  await b.reload();await expect(b.getByRole('button',{name:'Approve change A'})).toBeEnabled();
  await one.close();await two.close();
});

test('keyboard commands and reduced motion',async({page})=>{
  await page.emulateMedia({reducedMotion:'reduce'});await page.goto('/record/');
  await page.getByRole('button',{name:/Commands/}).click();
  await expect(page.getByRole('dialog',{name:'Commands & shortcuts'})).toBeVisible();
  await page.keyboard.press('Escape');await expect(page.getByRole('dialog')).toHaveCount(0);
  await page.keyboard.press('d');await expect(page.getByText('The current standard stays.')).toBeVisible();
});

for(const route of ['/','/record/','/lab/','/verify/','/developers/']) {
  test(`accessibility smoke: ${route}`,async({page})=>{
    await page.goto(route);
    const result=await new AxeBuilder({page}).withTags(['wcag2a','wcag2aa','wcag21a','wcag21aa','wcag22aa']).analyze();
    expect(result.violations.map(v=>`${v.id}: ${v.nodes.length}`)).toEqual([]);
  });
}

test('person evidence drawer is accessible',async({page})=>{
  await page.goto('/record/');await page.getByRole('button',{name:/Ada Hired/}).click();
  await expect(page.getByRole('dialog',{name:'Ada evidence'})).toBeVisible();
  const result=await new AxeBuilder({page}).withTags(['wcag2a','wcag2aa','wcag21a','wcag21aa','wcag22aa']).analyze();
  expect(result.violations.map(v=>`${v.id}: ${v.nodes.length}`)).toEqual([]);
  await page.keyboard.press('Escape');await expect(page.getByRole('dialog')).toHaveCount(0);
});

test('quotes, conflicting facts and override survive a reload',async({page})=>{
  await page.goto('/record/');
  await page.getByRole('button',{name:'Offer $250 quote'}).click();
  await expect(page.getByText('USD 250.00 · offered')).toBeVisible();
  await page.getByRole('button',{name:'Accept',exact:true}).click();
  await expect(page.getByText('USD 250.00 · accepted')).toBeVisible();
  await page.getByLabel('Interpreted value').fill('June');
  await page.getByRole('button',{name:'Add sourced fact'}).click();
  await expect(page.getByText('Availability: June')).toBeVisible();
  await page.getByLabel('Interpreted value').fill('July');
  await page.getByRole('button',{name:'Add sourced fact'}).click();
  await expect(page.getByText('Availability: July')).toBeVisible();
  await page.getByLabel('Human reason').fill('Synthetic review by a human.');
  await page.getByRole('button',{name:'Record override'}).click();
  await expect(page.getByText('Synthetic review by a human.')).toBeVisible();
  await page.reload();
  await expect(page.getByText('Availability: June')).toBeVisible();
  await expect(page.getByText('Availability: July')).toBeVisible();
  await expect(page.getByText('USD 250.00 · accepted')).toBeVisible();
});

test('signed JSON verifies locally and detects a changed payload',async({page},testInfo)=>{
  const fs=await import('node:fs/promises');
  await page.goto('/record/');
  const pending=page.waitForEvent('download');
  await page.getByRole('button',{name:'Seal & export'}).click();
  const download=await pending;const path=testInfo.outputPath('bundle.json');await download.saveAs(path);
  await page.goto('/verify/');await page.locator('input[type=file]').setInputFiles(path);
  await expect(page.getByText('PASSED',{exact:true})).toHaveCount(2);
  const bundle=JSON.parse(await fs.readFile(path,'utf8'));bundle.payload=bundle.payload.replace('3 of 3 hires','4 of 3 hires');
  const changed=testInfo.outputPath('altered.json');await fs.writeFile(changed,JSON.stringify(bundle));
  await page.locator('input[type=file]').setInputFiles(changed);
  await expect(page.getByText('FAILED',{exact:true})).toHaveCount(1);
});

test('live updates reach another tab with the same session',async({page,context})=>{
  await page.goto('/record/');await expect(page.getByText('Connected',{exact:true})).toBeVisible();
  const second=await context.newPage();await second.goto('/record/');await expect(second.getByText('Connected',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Dismiss D'}).click();
  await expect(second.getByText('The current standard stays.')).toBeVisible({timeout:7000});
  await second.close();
});
