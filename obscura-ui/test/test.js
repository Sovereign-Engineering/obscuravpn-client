import { expect } from 'chai';
import { spawn, spawnSync } from 'child_process';
import { existsSync, mkdtempSync, promises, readFileSync, renameSync, rmdirSync, rmSync } from 'fs';
import { homedir, platform, tmpdir } from 'os';
import path, { join, resolve } from 'path';
import { Builder, By, Capabilities } from 'selenium-webdriver';
import { parse } from 'toml';
import { fileURLToPath } from 'url';

const onWindows = platform() === 'win32';
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const projectRoot = resolve(__dirname, '..');
const frontendBuild = resolve(projectRoot, 'build');
const srcTauri = resolve(projectRoot, 'src-tauri');
const cargoConfig = parse(readFileSync(resolve(srcTauri, 'Cargo.toml'), 'utf-8'));
const cargoName = cargoConfig.package.name;
// create the path to the expected application binary
const releaseApp = resolve(
  srcTauri,
  'target',
  'release',
  onWindows ? `${cargoName}.exe` : cargoName
);
// keep track of the webdriver instance we create, and tauri-driver process we start
export let driver, tauriDriver;

async function dirWalkTree(dirPath, prefix) {
  prefix = prefix === undefined ? '' : prefix;
  return await Promise.all((await promises.readdir(dirPath, { withFileTypes: true })).map(async dirent =>
    dirent.isDirectory() ? await dirWalkTree(resolve(dirPath, dirent.name), join(prefix, dirent.name)) : join(prefix, dirent.name)
  ));
}

function filecmp(file1, file2) {
  if (existsSync(file1) !== existsSync(file2)) return false;
  if (!existsSync(file1) && !existsSync(file2)) return true;
  const buf1 = readFileSync(file1);
  const buf2 = readFileSync(file2);
  return buf1.compare(buf2) === 0;
}

async function dircmp(dir1, dir2) {
  if (existsSync(dir1) !== existsSync(dir2)) return false;
  if (!existsSync(dir1) && !existsSync(dir2)) return true;
  let files1 = new Set((await dirWalkTree(dir1)).flat(Number.POSITIVE_INFINITY));
  let files2 = new Set((await dirWalkTree(dir2)).flat(Number.POSITIVE_INFINITY));
  // different amount of files between two directories
  if (files1.size !== files2.size) return false;
  for (let file of files2) {
    // relative file in one directory not present in other directory
    if (!files2.has(file)) return false;
    const file1 = resolve(dir1, file);
    const file2 = resolve(dir2, file);
    if (!filecmp(file1, file2)) return false;
  }
  return true;
}

// requires using function because of "this" usage
before(async function () {
  // set timeout to 2 minutes to allow the program to build if it needs to
  this.timeout(120_000);

  // ensure the frontend has been built
  const tmpDir = mkdtempSync(join(tmpdir(), 'vite-cache'));
  rmdirSync(tmpDir);
  try {
    renameSync(frontendBuild, tmpDir);
  } catch (error) { }

  console.log('INFO: building frontend');
  const pnpmCmd = onWindows ? 'pnpm.cmd' : 'pnpm';
  const frontend_build_result = spawnSync(pnpmCmd, ['build'], { cwd: projectRoot, stdio: 'pipe', encoding: 'utf-8' });
  console.log(frontend_build_result.stderr);
  if (frontend_build_result.error !== undefined) {
    console.log(frontend_build_result.error);
    rmdirSync(frontendBuild, { recursive: true });
    renameSync(tmpDir, frontendBuild);
    process.exit(frontend_build_result.errno);
  }
  // check if frontend wasn't updated
  if (await dircmp(frontendBuild, tmpDir)) {
    console.log('INFO: frontend built; using previous frontend build to speed up rust build');
    rmSync(frontendBuild, { recursive: true, force: true });
    // bring back previous build so that program builds faster
    renameSync(tmpDir, frontendBuild);
  } else {
    console.log('INFO: frontend built');
    rmSync(tmpDir, { recursive: true, force: true });
  }
  // ensure the release app has been built
  console.log('INFO: building program (rust)');
  const cargo_build_result = spawnSync('cargo', ['build', '--release'], { cwd: srcTauri, stdio: 'pipe', encoding: 'utf-8' });
  console.log(cargo_build_result.stderr);
  if (cargo_build_result.error?.errno !== undefined) {
    console.log(cargo_build_result.error);
    process.exit(cargo_build_result.error.errno);
  }
  console.log('INFO: program built');

  // start tauri-driver
  tauriDriver = spawn(
    resolve(homedir(), '.cargo', 'bin', 'tauri-driver'),
    [],
    { stdio: [null, process.stdout, process.stderr] }
  )

  const capabilities = new Capabilities();
  capabilities.set('tauri:options', { application: releaseApp });
  capabilities.setBrowserName('wry');

  // start the webdriver client
  driver = await new Builder()
    .withCapabilities(capabilities)
    .usingServer(`http://localhost:4444/`)
    .build();
})

after(async () => {
  // stop the webdriver session and kill the webdriver
  await driver.quit();
  tauriDriver.kill();
});

// Resources
// https://mochajs.org/
// https://www.chaijs.com/guide/
// https://www.selenium.dev/selenium/docs/api/javascript/module/selenium-webdriver/lib/webdriver_exports_WebElement.html

// Other sample tests
/*
describe('Hello Tauri', () => {
  it('should be cordial', async () => {
    const text = await driver.findElement(By.css('body > h1')).getText();
    expect(text).to.match(/^[hH]ello/);
  })

  it('should be excited', async () => {
    const text = await driver.findElement(By.css('body > h1')).getText();
    expect(text).to.match(/!$/);
  });

  it('should be easy on the eyes', async () => {
    // selenium returns color css values as rgb(r, g, b)
    const text = await driver
      .findElement(By.css('body'))
      .getCssValue('background-color');

    const rgb = text.match(/^rgb\((?<r>\d+), (?<g>\d+), (?<b>\d+)\)$/).groups;
    expect(rgb).to.have.all.keys('r', 'g', 'b');

    const luma = 0.2126 * rgb.r + 0.7152 * rgb.g + 0.0722 * rgb.b;
    expect(luma).to.be.lessThan(100);
  })
});
*/
