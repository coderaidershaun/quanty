import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import http from 'node:http';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { once } from 'node:events';
import { setTimeout as delay } from 'node:timers/promises';

async function waitFor(check, message) {
  const deadline = performance.now() + 15000;
  while (!check()) {
    assert.ok(performance.now() < deadline, message);
    await delay(25);
  }
}

async function painted(page) {
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
}

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const home = await mkdtemp(path.join(os.tmpdir(), 'quanty-browser-'));
const output = process.env.ARTIFACT_DIR || home;
await mkdir(output, { recursive: true });
const child = spawn(process.env.QUANTY_BINARY || 'target/debug/quanty', [
  '--home', home, '--listen', '127.0.0.1:0', '--fixture', 'black-scholes', '--trusted-integration',
], { env: { PATH: process.env.PATH }, stdio: ['ignore', 'pipe', 'pipe'] });
process.on('exit', () => child.kill('SIGTERM'));
let errors = '';
child.stderr.on('data', data => { errors += data; });
const upstream = await new Promise((resolve, reject) => {
  const deadline = setTimeout(() => reject(new Error(`server startup timed out: ${errors}`)), 30000);
  deadline.unref();
  child.stdout.on('data', data => {
    const match = String(data).match(/listening on (http:\/\/\S+)/);
    if (match) { clearTimeout(deadline); resolve(new URL(match[1])); }
  });
  child.once('exit', code => { clearTimeout(deadline); reject(new Error(`server exited ${code}: ${errors}`)); });
});
const prefix = '/apps/test-id';
const requests = [];
const proxy = http.createServer((request, response) => {
  requests.push(request.url);
  if (!request.url.startsWith(`${prefix}/`)) { response.writeHead(404).end(); return; }
  const headers = { ...request.headers, host: upstream.host,
    'x-forwarded-proto': 'http', 'x-forwarded-prefix': prefix };
  if (headers.origin) headers.origin = upstream.origin;
  const forwarded = http.request(new URL(request.url.slice(prefix.length), upstream),
    { method: request.method, headers }, result => {
      response.writeHead(result.statusCode, result.headers); result.pipe(response);
    });
  forwarded.on('error', error => response.destroy(error));
  request.pipe(forwarded);
});
proxy.on('upgrade', (request, socket, head) => {
  requests.push(request.url);
  if (!request.url.startsWith(`${prefix}/`)) { socket.destroy(); return; }
  const connection = net.connect(Number(upstream.port), upstream.hostname, () => {
    const headers = { ...request.headers, host: upstream.host, origin: upstream.origin,
      'x-forwarded-proto': 'http', 'x-forwarded-prefix': prefix };
    connection.write(`${request.method} ${request.url.slice(prefix.length)} HTTP/1.1\r\n` +
      Object.entries(headers).map(([key, value]) => `${key}: ${value}\r\n`).join('') + '\r\n');
    connection.write(head); connection.pipe(socket); socket.pipe(connection);
  });
  connection.on('error', () => socket.destroy());
  socket.on('error', () => connection.destroy());
  socket.on('close', () => connection.destroy());
});
proxy.listen(0, '127.0.0.1');
await once(proxy, 'listening');
const browser = await chromium.launch({ executablePath: process.env.CHROME_BINARY,
  headless: true, args: ['--enable-unsafe-swiftshader'] });
try {
  for (const [label, url] of [['root', upstream.href],
    ['mounted', `http://127.0.0.1:${proxy.address().port}${prefix}/`]]) {
    const page = await browser.newPage({ viewport: { width: 1280, height: 850 } });
    const network = [], failures = [], sockets = [], frames = [];
    page.on('response', response => network.push({ url: response.url(), status: response.status() }));
    page.on('pageerror', error => failures.push(String(error)));
    page.on('websocket', socket => { sockets.push(socket.url()); socket.on('framereceived', frame => frames.push(frame.payload)); });
    await page.goto(url);
    try { await page.locator('#loading_text').waitFor({ state: 'detached', timeout: 45000 }); }
    catch (error) { console.error(label, await page.locator('#loading_text').textContent(), network, failures); throw error; }
    await waitFor(() => frames.length > 0, 'the fixture must answer over the browser socket');
    await painted(page);
    assert.equal(failures.length, 0, failures.join('\n'));
    assert.ok(network.some(row => row.url.endsWith('api/facts') && row.status === 200));
    assert.ok(sockets.some(socket => socket.endsWith('api/ws')));
    assert.ok(frames.length > 0, 'the fixture must answer over the browser socket');
    const beforeAsk = frames.length;
    await page.mouse.click(300, 90);
    await page.keyboard.type('Explain Black-Scholes');
    await painted(page);
    await page.mouse.click(1195, 90);
    await waitFor(() => frames.slice(beforeAsk).some(frame => JSON.parse(String(frame)).payload?.Answer?.result?.Ok),
      'the fixture must answer a browser question');
    await painted(page);
    await page.mouse.click(284, 198);
    await waitFor(() => network.some(row => row.url.includes('/content/') && row.status === 200),
      'the fixture figure must load through the visible app base');
    await painted(page);
    await page.screenshot({ path: path.join(output, `${label}-figure.png`) });
    assert.ok(network.some(row => row.url.includes('/content/') && row.status === 200), 'the fixture figure must load through the visible app base');
    const id = randomUUID();
    await mkdir(path.join(home, 'data/exports', id), { recursive: true });
    await writeFile(path.join(home, 'data/exports', id, 'sample.bin'), Buffer.from([0, 1, 254, 255]));
    const resources = await page.evaluate(async id => {
      const picture = await fetch('content/option-volatility-and-pricing/chapter-1/page-num-1/page.png');
      const upload = await fetch('api/upload', { method: 'POST', headers: { 'x-file-name': 'sample.pdf' }, body: '%PDF-1.7\n' });
      const download = await fetch(`api/download/${id}/sample.bin`);
      return { picture: picture.status, upload: upload.status, download: download.status, bytes: [...new Uint8Array(await download.arrayBuffer())] };
    }, id);
    assert.deepEqual(resources, { picture: 200, upload: 200, download: 200, bytes: [0, 1, 254, 255] });
    assert.ok(await page.locator('canvas').evaluate(canvas => canvas.width > 100 && canvas.height > 100));
    assert.equal(failures.length, 0, failures.join('\n'));
    await page.screenshot({ path: path.join(output, `${label}.png`) });
    await writeFile(path.join(output, `${label}-network.json`), JSON.stringify({ network, sockets, frameCount: frames.length, failures }, null, 2));
    await page.close();
  }
  assert.ok(requests.every(url => url.startsWith(`${prefix}/`)), JSON.stringify(requests));
  console.log('root and mounted WASM fixture passed');
} finally {
  await browser.close();
  proxy.closeAllConnections(); proxy.close();
  child.kill('SIGTERM');
  await rm(home, { recursive: true, force: true });
}
