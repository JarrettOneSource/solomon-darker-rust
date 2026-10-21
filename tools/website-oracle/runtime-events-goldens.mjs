// Actual loopback HTTP + public Source publisher. Retry and overflow observations
// are recorded only after the endpoint has accepted the first real request.
import { createServer } from 'node:http'
import { loadKernel, writeFixture } from './oracle.mjs'

const { createRuntimeEventPublisher } = await loadKernel('host/runtime-event-publisher.ts')
const secret = 'runtime-event-fixture-secret-000000000000'
const configuration = []
for (const endpoint of [
  'http://127.0.0.1/events', 'HTTP://LOCALHOST/events', 'http://127.1/events',
  'http://0x7f000001/events', 'http://2130706433/events', 'http://[::1]/events',
  'https://localhost/events', 'http://example.test/events', 'http://127.0.0.2/events',
  'not a URL',
]) {
  try {
    const publisher = createRuntimeEventPublisher(endpoint, secret)
    await publisher.close()
    configuration.push({ endpoint, secret, accepted: true, normalized: new URL(endpoint).href })
  } catch (error) { configuration.push({ endpoint, secret, accepted: false, error: error.message }) }
}
for (const candidate of ['short', '\ud83e\uddd9'.repeat(15), '\ud83e\uddd9'.repeat(16)]) {
  const endpoint = 'http://localhost/events'
  try {
    const publisher = createRuntimeEventPublisher(endpoint, candidate)
    await publisher.close()
    configuration.push({ endpoint, secret: candidate, accepted: true, normalized: new URL(endpoint).href })
  } catch (error) { configuration.push({ endpoint, secret: candidate, accepted: false, error: error.message }) }
}

const requests = []
let statusFor = () => 200
let holdFirst = null
let releaseFirst = null
const server = createServer(async (request, response) => {
  const chunks = []
  for await (const chunk of request) chunks.push(chunk)
  const body = Buffer.concat(chunks).toString('utf8')
  const document = JSON.parse(body)
  requests.push({ path: request.url, method: request.method,
    authorization: request.headers.authorization, contentType: request.headers['content-type'], body, document })
  const status = statusFor(document)
  if (holdFirst) {
    const held = holdFirst
    holdFirst = null
    await held
  }
  response.writeHead(status).end()
})
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
const endpoint = `http://127.0.0.1:${server.address().port}/runtime-events?source=rust`
const entry = id => ({ component: 'game-host', event: 'fixture.accepted', message: '\u2728 Accepted',
  occurredAtUtc: '2026-10-21T12:00:00.000Z', details: { id, value: .25 } })
const until = async predicate => {
  const end = Date.now() + 10_000
  while (!predicate()) {
    if (Date.now() >= end) throw new Error('runtime-event endpoint did not observe the source request')
    await new Promise(resolve => setTimeout(resolve, 1))
  }
}
const cases = []
try {
  let failures = 2
  statusFor = () => failures-- > 0 ? 503 : 200
  let publisher = createRuntimeEventPublisher(endpoint, secret)
  for (const id of [1, 2, 3]) publisher.publish(entry(id))
  await publisher.close()
  publisher.publish(entry(99))
  cases.push({ name: 'ordered-retry-and-closed-admission', entries: [1, 2, 3], requests: requests.splice(0) })

  statusFor = () => 500
  publisher = createRuntimeEventPublisher(endpoint, secret)
  publisher.publish(entry(4))
  await publisher.close()
  cases.push({ name: 'five-attempt-exhaustion', entries: [4], requests: requests.splice(0) })

  statusFor = () => 200
  holdFirst = new Promise(resolve => { releaseFirst = resolve })
  publisher = createRuntimeEventPublisher(endpoint, secret)
  publisher.publish(entry(0))
  await until(() => requests.length === 1)
  for (let id = 1; id <= 260; id += 1) publisher.publish(entry(id))
  releaseFirst()
  await publisher.close()
  cases.push({ name: 'bounded-overflow-in-flight-shift', entries: [0, 260], requestIds: requests.map(row => row.document.details.id) })
  writeFixture('runtime-events-goldens.json', 'runtime-events-goldens.mjs',
    'host/runtime-event-publisher.ts (actual loopback HTTP)', { configuration, cases })
} finally {
  server.closeAllConnections()
  await new Promise(resolve => server.close(resolve))
}
