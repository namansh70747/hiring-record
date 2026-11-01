import { ed25519 } from '@noble/curves/ed25519.js';
import { blake3 } from '@noble/hashes/blake3.js';
import { bytesToHex, hexToBytes, utf8ToBytes } from '@noble/hashes/utils.js';
import type { Event, Report } from './types';

export type Bundle = { format: string; algorithm: string; public_key: string; payload: string; signature: string };
export type Payload = { format: string; role_id: string; standard_id: string; report: Report; events: Event[]; disclosure: string; source_references?: {id:string;person:string;kind:string;timestamp:string}[] };
export type Result = { signatureValid: boolean; chainValid: boolean; publicKey: string; payload: Payload };
const kinds: Record<string,string> = { DomainCommandV2:'domain_command_v2', ProposalDismissed:'proposal_dismissed', StandardApproved:'standard_approved', ReceiptAcknowledged:'receipt_acknowledged', HoldConfirmed:'hold_confirmed', SupportRemoved:'support_removed' };

export function verifyChain(events: Event[]): boolean {
  let previous = 'genesis';
  for (let i = 0; i < events.length; i++) {
    const event = events[i];
    const kind = kinds[event.kind];
    if (!kind || event.seq !== i + 1 || event.prev_hash !== previous) return false;
    const canonical = `hr-event/v1\nseq=${event.seq}\ntype=${kind}\npayload=${event.payload}\nprev=${event.prev_hash}`;
    if (bytesToHex(blake3(utf8ToBytes(canonical))) !== event.hash) return false;
    previous = event.hash;
  }
  return true;
}

export function verifyBundle(input: unknown): Result {
  if (!input || typeof input !== 'object') throw new Error('The file is not a bundle.');
  const bundle = input as Partial<Bundle>;
  if (bundle.format !== 'hiring-record-bundle/v1' || bundle.algorithm !== 'Ed25519') throw new Error('Unsupported bundle format or algorithm.');
  if (typeof bundle.public_key !== 'string' || !/^[a-f0-9]{64}$/i.test(bundle.public_key) || typeof bundle.signature !== 'string' || !/^[a-f0-9]{128}$/i.test(bundle.signature) || typeof bundle.payload !== 'string') throw new Error('The bundle has an invalid key, signature or payload.');
  const payload: Payload = JSON.parse(bundle.payload);
  if (!['hiring-record-payload/v1','hiring-record-payload/v2','hiring-record-payload/v3'].includes(payload.format) || !Array.isArray(payload.events) || !payload.report || typeof payload.role_id !== 'string') throw new Error('The payload format is invalid.');
  let signatureValid = false;
  try { signatureValid = ed25519.verify(hexToBytes(bundle.signature), utf8ToBytes(bundle.payload), hexToBytes(bundle.public_key)); } catch { signatureValid = false; }
  return { signatureValid, chainValid: verifyChain(payload.events), publicKey: bundle.public_key, payload };
}
