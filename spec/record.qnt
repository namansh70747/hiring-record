// Bounded abstraction of the original September proposal and supported-claim path.
// This model does not prove the Rust implementation or external services.
module record {
  var proposal: int // 0 open, 1 dismissed, 2 approved
  var stale: bool
  var rubric: int
  var ack: bool
  var hold: bool
  var support: bool
  var beforeHire: bool
  var allRejectsMet: bool
  var bothGroups: bool
  var hasSyncOperation: bool
  var predicted: bool
  var synced: bool

  action init = {
    nondet before = Set(true, false).oneOf()
    nondet rejects = Set(true, false).oneOf()
    nondet groups = Set(true, false).oneOf()
    nondet operation = Set(true, false).oneOf()
    all { proposal' = 0, stale' = false, rubric' = 0, ack' = false, hold' = false, support' = true,
      beforeHire' = before, allRejectsMet' = rejects, bothGroups' = groups, hasSyncOperation' = operation,
      predicted' = false, synced' = false }
  }
  action move(p: int, s: bool, a: bool, h: bool, u: bool, prediction: bool, sync: bool): bool = all {
    proposal' = p, stale' = s, rubric' = rubric, ack' = a, hold' = h, support' = u,
    beforeHire' = beforeHire, allRejectsMet' = allRejectsMet, bothGroups' = bothGroups,
    hasSyncOperation' = hasSyncOperation, predicted' = prediction, synced' = sync
  }
  action dismiss = all { proposal == 0, move(1, false, ack, hold, support, predicted, synced) }
  action approve = all { proposal == 0, move(2, true, ack, hold, support, predicted, synced) }
  action acknowledge = all { proposal == 2, not(ack), move(proposal, stale, true, hold, support, predicted, synced) }
  action confirmHold = all { not(hold), move(proposal, stale, ack, true, support, predicted, synced) }
  action removeSupport = all { support, move(proposal, stale, ack, hold, false, false, synced) }
  action makePrediction = all { beforeHire, not(allRejectsMet), bothGroups, support, move(proposal, stale, ack, hold, support, true, synced) }
  action makeSyncClaim = all { ack, hasSyncOperation, move(proposal, stale, ack, hold, support, predicted, true) }
  action unchanged = move(proposal, stale, ack, hold, support, predicted, synced)
  action step = any { dismiss, approve, acknowledge, confirmHold, removeSupport, makePrediction, makeSyncClaim, unchanged }
  val safety = all {
    proposal >= 0, proposal <= 2,
    stale == (proposal == 2),
    not(ack) or proposal == 2,
    rubric == 0,
    not(predicted) or (beforeHire and not(allRejectsMet) and bothGroups and support),
    not(synced) or (ack and hasSyncOperation)
  }
}
