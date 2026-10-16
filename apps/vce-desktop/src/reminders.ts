export type ReminderEvent = {
  id: string;
  title: string;
  date: string;
  time?: string;
  annual: boolean;
  lead_days: number;
  enabled: boolean;
  channel?: string;
};

export type DueReminder = ReminderEvent & { dueDate: string; missed: boolean };

const dayPattern = /^\d{4}-\d{2}-\d{2}$/;
const timePattern = /^([01]\d|2[0-3]):[0-5]\d$/;

function validDate(date: string): boolean {
  if (!dayPattern.test(date)) return false;
  const parsed = new Date(`${date}T12:00:00Z`);
  return Number.isFinite(parsed.getTime()) && parsed.toISOString().slice(0, 10) === date;
}

function addDays(date: string, days: number): string {
  const parsed = new Date(`${date}T12:00:00Z`);
  parsed.setUTCDate(parsed.getUTCDate() + days);
  return parsed.toISOString().slice(0, 10);
}

function annualOccurrence(original: string, year: number): string {
  let occurrence = `${year}-${original.slice(5)}`;
  if (occurrence.endsWith('-02-29') && !validDate(occurrence)) occurrence = `${year}-02-28`;
  return occurrence;
}

export function currentDayAndTime(now: Date, timezone: string): { day: string; time: string } {
  const parts = new Intl.DateTimeFormat('en-US', {
    timeZone: timezone, year: 'numeric', month: '2-digit', day: '2-digit',
    hour: '2-digit', minute: '2-digit', hourCycle: 'h23',
  }).formatToParts(now);
  const value = (type: string) => parts.find(part => part.type === type)?.value || '';
  return { day: `${value('year')}-${value('month')}-${value('day')}`, time: `${value('hour')}:${value('minute')}` };
}

export function listReminderEvents(profile: any): ReminderEvent[] {
  const listed: ReminderEvent[] = Array.isArray(profile.reminders) ? profile.reminders : [];
  const contacts = profile.consents?.contacts_storage && Array.isArray(profile.contacts) ? profile.contacts : [];
  const contactDates: ReminderEvent[] = contacts.flatMap((person: any) =>
    (['birthday', 'anniversary'] as const).filter(kind => person[kind]).map(kind => ({
      id: `${person.id}-${kind}`,
      title: `${person.name}: ${kind}`,
      date: person[kind],
      time: '09:00',
      annual: true,
      lead_days: 0,
      enabled: true,
      channel: 'in_app',
    })));
  return [...listed, ...contactDates];
}

export function dueReminders(
  events: ReminderEvent[], now: Date, timezone: string, lastCheckedDay: string | null,
  delivered: ReadonlySet<string>,
): DueReminder[] {
  const { day, time } = currentDayAndTime(now, timezone);
  const earliest = lastCheckedDay && validDate(lastCheckedDay) && lastCheckedDay <= day
    ? (lastCheckedDay < addDays(day, -7) ? addDays(day, -7) : lastCheckedDay)
    : day;
  const due: DueReminder[] = [];
  for (let cursor = earliest; cursor <= day; cursor = addDays(cursor, 1)) {
    for (const event of events) {
      if (!event.enabled || !validDate(event.date) || !Number.isInteger(event.lead_days)
          || event.lead_days < 0 || event.lead_days > 366 || !event.id) continue;
      const at = event.time || '09:00';
      if (!timePattern.test(at) || (cursor === day && time < at)) continue;
      const target = addDays(cursor, event.lead_days);
      const occurrence = event.annual ? annualOccurrence(event.date, Number(target.slice(0, 4))) : event.date;
      if (occurrence !== target || target < event.date) continue;
      const key = `${event.id}:${cursor}`;
      if (!delivered.has(key)) due.push({ ...event, dueDate: cursor, missed: cursor < day });
    }
  }
  return due;
}

export function deliveryKey(reminder: DueReminder): string { return `${reminder.id}:${reminder.dueDate}`; }
