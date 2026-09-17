import type { RecoveryContextIdentity, RecoveryContextSelection } from "../shared/contracts.ts";

export const MAX_RECOVERY_SELECTION_ENTRIES = 4096;

export function recoverySelectionProblem(selection: RecoveryContextSelection): string | null {
  if (selection.records.length > MAX_RECOVERY_SELECTION_ENTRIES)
    return "Recovery context selection exceeds its record bound.";
  const deaths = new Set<string>(), entries = new Set<string>();
  for (const record of selection.records) {
    if (!record.entries.length || deaths.has(record.death))
      return "Recovery context records must be distinct and nonempty.";
    deaths.add(record.death);
    for (const entry of record.entries) {
      if (entries.has(entry.id) || entries.size === MAX_RECOVERY_SELECTION_ENTRIES)
        return "Recovery context entries must be distinct and bounded.";
      entries.add(entry.id);
      if (selection.context.kind === "grave" &&
          (selection.context.death !== record.death || entry.current_storage !== "grave"))
        return "A grave context can select only its own grave entries.";
    }
  }
  return null;
}

export function sameRecoveryContext(left: RecoveryContextIdentity, right: RecoveryContextIdentity): boolean {
  if (left.interface !== right.interface) return false;
  return left.kind === "grave" ? right.kind === "grave" && left.death === right.death
    : right.kind === "death_office" && left.instance === right.instance;
}

export function sameRecoverySelection(left: RecoveryContextSelection, right: RecoveryContextSelection): boolean {
  return sameRecoveryContext(left.context, right.context) && left.records.length === right.records.length
    && left.records.every((record, index) => {
      const other = right.records[index]!;
      return record.death === other.death && record.entries.length === other.entries.length
        && record.entries.every((entry, offset) => {
          const next = other.entries[offset]!;
          return entry.id === next.id && entry.quantity === next.quantity
            && entry.current_storage === next.current_storage;
        });
    });
}
