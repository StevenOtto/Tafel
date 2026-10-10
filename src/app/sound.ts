/**
 * Confirmation tone for a fired gesture. Synthesised, so there is no asset
 * and no network; the wall camera user is often not looking at the screen.
 */

let context: AudioContext | null = null;

const audioContext = (): AudioContext => {
  if (!context) context = new AudioContext();
  return context;
};

const tone = (
  ctx: AudioContext,
  frequency: number,
  startAt: number,
  durationS: number,
): void => {
  const oscillator = ctx.createOscillator();
  const gain = ctx.createGain();
  oscillator.type = "sine";
  oscillator.frequency.value = frequency;
  gain.gain.setValueAtTime(0.0001, startAt);
  gain.gain.exponentialRampToValueAtTime(0.25, startAt + 0.01);
  gain.gain.exponentialRampToValueAtTime(0.0001, startAt + durationS);
  oscillator.connect(gain).connect(ctx.destination);
  oscillator.start(startAt);
  oscillator.stop(startAt + durationS);
};

/** Two rising notes: "done". */
export const playFireTone = (): void => {
  const ctx = audioContext();
  void ctx.resume();
  const now = ctx.currentTime;
  tone(ctx, 880, now, 0.12);
  tone(ctx, 1320, now + 0.12, 0.16);
};
