// Runs a Rust wasm32-wasip1 test binary under Node's WASI implementation.
//
// Cargo sets this as the runner for wasm32-wasip1 in .cargo/config.toml. It is
// how engine CI executes the determinism corpus on wasm32, not merely compiles
// it. The preopen is the whole filesystem because tests read fixtures through
// their compile-time CARGO_MANIFEST_DIR path.
import { WASI } from 'node:wasi';
import { readFileSync } from 'node:fs';
import { argv } from 'node:process';

const binary = argv[2];
if (!binary) {
  console.error('usage: node wasi-test-runner.mjs <test.wasm> [test args...]');
  process.exit(2);
}

const wasi = new WASI({
  version: 'preview1',
  args: argv.slice(2),
  env: {},
  preopens: { '/': '/' },
});

const module = await WebAssembly.compile(readFileSync(binary));
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
process.exit(wasi.start(instance) ?? 0);
