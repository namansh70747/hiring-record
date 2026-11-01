import * as restate from '@restatedev/restate-sdk';

// Fictional lagging system only. No external ATS, email or calendar integration.
const receiptDelivery = restate.workflow({
  name: 'ReceiptDelivery',
  handlers: {
    run: async (ctx, envelope) => {
      const job = JSON.parse(envelope.payload);
      await ctx.sleep({ milliseconds: Math.min(Math.max(job.delay_ms ?? 15000, 0), 60000) });
      const result = await ctx.run('acknowledge-receipt', async () => {
        const response = await fetch(`${process.env.RECORD_API_URL ?? 'http://host.docker.internal:8080'}/internal/workflow/complete`, {
          method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(envelope),
          signal: AbortSignal.timeout(10000),
        });
        if (response.status === 401) throw new restate.TerminalError('Callback signature is no longer valid');
        if (!response.ok) throw new Error(`Callback HTTP ${response.status}`);
        return response.json();
      });
      return { synthetic: true, ...result };
    },
  },
});
restate.serve({ services: [receiptDelivery], port: 9080 });
