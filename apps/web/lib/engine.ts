import type { Comparison } from './types';

type WasmEngine = { default: (url: string) => Promise<unknown>; compare_other_path: (state: string) => string; evaluate_report: (state: string) => string };
let engine: Promise<WasmEngine> | undefined;

async function loadEngine(): Promise<WasmEngine> {
  if (!engine) {
    engine = (async () => {
      // The built module is served as an asset, so WebAssembly is instantiated only in the browser.
      const path = '/engine/record_wasm.js';
      const module: WasmEngine = await import(/* webpackIgnore: true */ /* turbopackIgnore: true */ path);
      await module.default('/engine/record_wasm_bg.wasm');
      return module;
    })().catch(error => { engine = undefined; throw error; });
  }
  return engine;
}

export async function compareInBrowser(state: unknown): Promise<Comparison> {
  const module = await loadEngine();
  return JSON.parse(module.compare_other_path(JSON.stringify(state))) as Comparison;
}
