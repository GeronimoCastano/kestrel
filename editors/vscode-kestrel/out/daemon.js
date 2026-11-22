'use strict';

// One long-lived `kestrel serve` process per workspace folder. Checking the
// standard library is the expensive part of a cold check; after the first
// request the process keeps it warm, and later keystrokes cost a few
// milliseconds instead of seconds.
//
// Requests are coalesced: while a check runs, newer text replaces whatever was
// waiting, so typing quickly never builds a queue of checks for old text.

const { spawn } = require('child_process');
const readline = require('readline');

const PROTOCOL = 1;

class CheckDaemon {
  constructor(compiler, root, options = {}) {
    this.compiler = compiler;
    this.root = root;
    this.timeoutMs = options.timeoutMs || 30000;
    this.log = options.log || (() => {});
    this.state = 'new';
    this.failure = null;
    this.child = null;
    this.ready = null;
    this.resolveReady = null;
    this.rejectReady = null;
    this.pending = null;
    this.inFlight = null;
    this.nextId = 1;
    this.stderr = [];
  }

  // Starts the process once. The first request pays the cold standard-library
  // check; every request after it reuses the same process.
  ensureStarted() {
    if (this.state === 'ready') return Promise.resolve();
    if (this.state === 'failed') return Promise.reject(this.failure);
    if (this.state === 'closed') return Promise.reject(new Error('The checker server closed.'));
    if (this.ready) return this.ready;
    this.ready = new Promise((resolve, reject) => {
      this.resolveReady = resolve;
      this.rejectReady = reject;
    });
    this.state = 'starting';
    try {
      this.child = spawn(this.compiler, ['serve', '--root', this.root], {
        cwd: this.root,
        stdio: ['pipe', 'pipe', 'pipe']
      });
    } catch (error) {
      this.fail(error);
      return this.ready;
    }
    // Keep the process alive while a request is in flight, but let an idle
    // server go when the extension host (or a test runner) has nothing else to
    // do. The server exits by itself when its input closes.
    this.child.stderr.unref?.();
    const lines = readline.createInterface({ input: this.child.stdout });
    lines.on('line', line => this.receive(line));
    this.child.stderr.on('data', chunk => {
      const text = String(chunk);
      this.stderr.push(text);
      if (this.stderr.join('').length > 12000) this.stderr.shift();
      this.log(text.trimEnd());
    });
    this.child.stdin.on('error', () => {});
    this.child.on('error', error => this.fail(error));
    this.child.on('close', code => {
      if (this.state === 'starting') {
        const detail = this.stderr.join('').trim();
        this.fail(new Error(detail || `The checker server exited with ${code}.`));
      } else if (this.state === 'ready') {
        this.fail(new Error(`The checker server stopped with ${code}.`));
      }
    });
    this.write({ id: 0, op: 'hello' });
    return this.ready;
  }

  refChild() {
    if (!this.child) return;
    this.child.ref();
    this.child.stdin.ref?.();
    this.child.stdout.ref?.();
  }

  unrefChild() {
    if (!this.child) return;
    this.child.unref();
    this.child.stdin.unref?.();
    this.child.stdout.unref?.();
  }

  // The latest request wins. An older waiting request resolves as cancelled so
  // its caller can simply drop the result.
  check(request) {
    return this.send({ op: 'check', ...request });
  }

  // Live goals use the same warm process and the same latest-request-wins rule.
  goals(request) {
    return this.send({ op: 'goals', ...request });
  }

  // Quick fixes use the checker's own diagnostics, so the editor never has to
  // guess which methods or cases are missing.
  actions(request) {
    return this.send({ op: 'actions', ...request });
  }

  send(request) {
    if (this.state === 'failed') return Promise.reject(this.failure);
    if (this.state === 'closed') return Promise.reject(new Error('The checker server closed.'));
    this.refChild();
    return new Promise((resolve, reject) => {
      if (this.pending) this.pending.resolve({ cancelled: true });
      this.pending = { request, resolve, reject, timer: null };
      this.ensureStarted().then(() => this.pump(), reject);
    });
  }

  pump() {
    if (this.state !== 'ready' || this.inFlight || !this.pending) return;
    const entry = this.pending;
    this.pending = null;
    entry.id = this.nextId;
    this.nextId += 1;
    this.inFlight = entry;
    this.refChild();
    entry.timer = setTimeout(() => this.timeout(entry), this.timeoutMs);
    this.write({
      id: entry.id,
      ...entry.request,
      path: entry.request.path,
      text: entry.request.text,
      notebook: Boolean(entry.request.notebook)
    });
  }

  timeout(entry) {
    if (this.inFlight !== entry) return;
    this.inFlight = null;
    entry.reject(new Error(`The checker took longer than ${this.timeoutMs} ms.`));
    this.pump();
    if (!this.inFlight && !this.pending) this.unrefChild();
  }

  receive(line) {
    let message;
    try {
      message = JSON.parse(line);
    } catch (error) {
      this.log(`Ignoring a malformed checker reply: ${line}`);
      return;
    }
    if (message.id === 0) {
      if (this.state !== 'starting') return;
      if (!message.ok) {
        this.fail(new Error(message.error || 'The checker rejected the editor protocol.'));
        return;
      }
      if (message.protocol !== PROTOCOL) {
        this.fail(new Error(
          `The checker speaks editor protocol ${message.protocol}, but this extension needs ${PROTOCOL}. Rebuild the compiler.`
        ));
        return;
      }
      this.state = 'ready';
      this.resolveReady();
      this.pump();
      if (!this.inFlight && !this.pending) this.unrefChild();
      return;
    }
    const entry = this.inFlight;
    if (!entry || entry.id !== message.id) return;
    clearTimeout(entry.timer);
    this.inFlight = null;
    if (message.ok) entry.resolve(message);
    else entry.reject(new Error(message.error || 'The checker failed.'));
    this.pump();
    if (!this.inFlight && !this.pending) this.unrefChild();
  }

  write(message) {
    if (!this.child || !this.child.stdin.writable) return;
    try {
      this.child.stdin.write(`${JSON.stringify(message)}\n`);
    } catch (error) {
      this.fail(error);
    }
  }

  fail(error) {
    if (this.state === 'failed' || this.state === 'closed') return;
    this.state = 'failed';
    this.failure = error;
    if (this.rejectReady) this.rejectReady(error);
    const waiting = this.pending;
    this.pending = null;
    if (waiting) {
      clearTimeout(waiting.timer);
      waiting.reject(error);
    }
    if (this.inFlight) {
      clearTimeout(this.inFlight.timer);
      const running = this.inFlight;
      this.inFlight = null;
      running.reject(error);
    }
    if (this.child) {
      this.child.kill();
      this.child = null;
    }
  }

  // Stops the server. The promise settles once its process has closed: Windows
  // keeps the server's working folder busy until then, so a caller that removes
  // the folder waits for it.
  dispose() {
    if (this.state === 'closed') return this.closed ?? Promise.resolve();
    this.state = 'closed';
    this.closed = Promise.resolve();
    if (this.child) {
      const child = this.child;
      this.closed = child.exitCode !== null || child.signalCode !== null
        ? Promise.resolve()
        : new Promise(resolve => child.once('close', () => resolve()));
      // An idle server is unreferenced; hold it so the caller's process lives
      // until the promise settles.
      this.refChild();
      this.write({ id: 0, op: 'shutdown' });
      child.kill();
      this.child = null;
    }
    const stopped = new Error('The checker server closed.');
    if (this.pending) this.pending.reject(stopped);
    if (this.inFlight) this.inFlight.reject(stopped);
    this.pending = null;
    this.inFlight = null;
    return this.closed;
  }
}

module.exports = { CheckDaemon, PROTOCOL };
