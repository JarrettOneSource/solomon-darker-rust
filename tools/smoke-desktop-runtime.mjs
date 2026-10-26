// Exercise the packaged native executable through the platform accessibility APIs.
import { join } from 'node:path';
import { repositoryRoot, run } from './desktop-build.mjs';

await run(process.env.PYTHON || (process.platform === 'win32' ? 'python' : 'python3'),
  [join(repositoryRoot, 'tools/smoke-desktop-runtime.py')]);
