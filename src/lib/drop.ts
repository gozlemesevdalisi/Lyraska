import type { DropOutcome } from "./backend";

/** Pencereye bırakma sonrasında kısa süre gösterilen bildirim. */
export interface DropNotice {
  text: string;
  /** Eklenemeyen bir şey var: bildirim hata rengiyle ve daha uzun gösterilir. */
  problem: boolean;
}

/** Bildirimin ekranda kalma süresi (ms). */
export const DROP_NOTICE_MS = 5000;
export const DROP_PROBLEM_MS = 9000;

/** Bırakılanların sonucunu tek satırlık, sade bir Türkçe bildirime çevirir. */
export function dropSummary(outcome: DropOutcome): DropNotice {
  const parts: string[] = [];
  if (outcome.addedTracks > 0) parts.push(`${outcome.addedTracks} şarkı kütüphaneye eklendi`);
  if (outcome.addedFolders > 0) {
    parts.push(`${outcome.addedFolders} klasör kütüphaneye eklendi, şarkıları taranıyor`);
  }
  const [first, ...rest] = outcome.problems;
  if (outcome.already > 0) {
    const alone = parts.length === 0 && !first;
    if (alone) parts.push(outcome.already === 1 ? "zaten kütüphanede" : "hepsi zaten kütüphanede");
    else if (outcome.already === 1) parts.push("biri zaten kütüphanedeydi");
    else parts.push(`${outcome.already} tanesi zaten kütüphanedeydi`);
  }
  if (first) parts.push(rest.length > 0 ? `${first} (ve ${rest.length} sorun daha)` : first);
  if (parts.length === 0) {
    return { text: "Bırakılanlarda şarkı ya da klasör bulunamadı.", problem: true };
  }
  const text = parts.join(" · ");
  return { text: text.charAt(0).toLocaleUpperCase("tr") + text.slice(1), problem: !!first };
}
