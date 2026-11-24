import { Then, When } from '@cucumber/cucumber';
import type { ThreadbearWorld } from '../support/world.ts';

const LANGUAGE_TIP = "The language for Threadbear's buttons, menus and messages";

type Screen = 'en' | 'ja';

type Language = { picked: string; screen: Screen };

type Label = { label: string; role: string | null };

function hasKey<K extends string>(value: unknown, key: K): value is Record<K, unknown> {
  return typeof value === 'object' && value !== null && key in value;
}

function isScreen(value: unknown): value is Screen {
  return value === 'en' || value === 'ja';
}

function languageIn(dump: unknown): Language | undefined {
  if (!hasKey(dump, 'settingsWindow') || !hasKey(dump.settingsWindow, 'language')) return undefined;
  const language = dump.settingsWindow.language;
  if (!hasKey(language, 'picked') || !hasKey(language, 'screen')) return undefined;
  const { picked, screen } = language;
  if (typeof picked !== 'string' || !isScreen(screen)) return undefined;
  return { picked, screen };
}

function labelsIn(text: string): Label[] {
  const parsed: unknown = JSON.parse(text);
  if (!Array.isArray(parsed)) throw new Error(`labels didn't answer a list: ${text}`);
  return parsed.flatMap((entry: unknown) => {
    if (!hasKey(entry, 'label') || typeof entry.label !== 'string') return [];
    const role = hasKey(entry, 'role') && typeof entry.role === 'string' ? entry.role : null;
    return [{ label: entry.label, role }];
  });
}

function screenNamed(word: string): Screen {
  if (word === 'English') return 'en';
  if (word === 'Japanese') return 'ja';
  throw new Error(`a screen is English or Japanese, not ${word}`);
}

async function tallSettings(world: ThreadbearWorld): Promise<void> {
  await world.app.do({ cmd: 'resize', settings: true, w: 880, h: 1600 });
}

When("I pick {string} from Settings' language menu", async function (this: ThreadbearWorld, choice: string) {
  await tallSettings(this);
  await this.app.do({ cmd: 'click', tipPrefix: LANGUAGE_TIP, settings: true });
  await this.app.do({ cmd: 'click', tip: choice, settings: true });
});

Then("Settings' language is {string} on a(n) {word} screen", async function (this: ThreadbearWorld, picked: string, word: string) {
  const screen = screenNamed(word);
  await this.until(`${picked} picked, the screen in ${word}`, (d) => {
    const language = languageIn(d);
    return language?.picked === picked && language.screen === screen;
  });
});

Then('Settings draws the button {string}', async function (this: ThreadbearWorld, words: string) {
  await tallSettings(this);
  const deadline = Date.now() + 15_000;
  for (;;) {
    const labels = labelsIn(await this.app.do({ cmd: 'labels', settings: true }));
    if (labels.some((l) => l.label === words && l.role === 'Button')) return;
    if (Date.now() > deadline) throw new Error(`Settings draws no "${words}" button; on screen: ${labels.map((l) => l.label).join(', ')}`);
    await new Promise((resolve) => setTimeout(resolve, 120));
  }
});
