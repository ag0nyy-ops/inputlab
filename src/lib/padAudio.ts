// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// the controller's own speaker and microphone. over usb a dualsense shows up to the os as an audio
// device ("wireless controller"), so the webview's audio apis can test it. nothing is recorded to
// disk or sent anywhere: the mic is only read for a level meter, and a short test clip is kept in
// memory until it is played back.

export interface AudioDev {
  kind: string;
  label: string;
  deviceId: string;
}

export interface PadAudioDevices {
  input: AudioDev | null;
  output: AudioDev | null;
}

/** playstation controllers list themselves as "wireless controller", and chromium adds the usb id */
const PAD_AUDIO = /wireless controller|dualsense|dualshock|ps5 controller|054c:/i;

/** pick the controller's microphone and speaker out of the os's audio devices */
export function matchPadAudio(devs: AudioDev[]): PadAudioDevices {
  const pick = (kind: string) => devs.find((d) => d.kind === kind && PAD_AUDIO.test(d.label) && d.deviceId !== 'default' && d.deviceId !== 'communications') ?? null;
  return { input: pick('audioinput'), output: pick('audiooutput') };
}

/** ask for the microphone (needed before the os will name its audio devices), then look for the pad */
export async function findPadAudio(): Promise<{ devices: PadAudioDevices; note: string | null }> {
  let note: string | null = null;
  try {
    const s = await navigator.mediaDevices.getUserMedia({ audio: true });
    s.getTracks().forEach((t) => t.stop());
  } catch (e) {
    note = `microphone access was refused or there is no microphone (${(e as Error).name}). the device names stay hidden until it is allowed.`;
  }
  const list = await navigator.mediaDevices.enumerateDevices();
  return { devices: matchPadAudio(list), note };
}

const rawMic = (id: string): MediaStreamConstraints => ({ audio: { deviceId: { exact: id }, echoCancellation: false, noiseSuppression: false, autoGainControl: false } });

/** live microphone level: 0..1 on a -60..0 dBFS scale */
export class MicMeter {
  private stream: MediaStream | null = null;
  private ctx: AudioContext | null = null;
  private timer = 0;

  async start(id: string, onLevel: (level: number, db: number) => void): Promise<void> {
    this.stop();
    this.stream = await navigator.mediaDevices.getUserMedia(rawMic(id));
    this.ctx = new AudioContext();
    const an = this.ctx.createAnalyser();
    an.fftSize = 1024;
    // deliberately not connected to any output: the mic must never feed the speaker
    this.ctx.createMediaStreamSource(this.stream).connect(an);
    const buf = new Float32Array(an.fftSize);
    this.timer = window.setInterval(() => {
      an.getFloatTimeDomainData(buf);
      let sum = 0;
      for (const v of buf) sum += v * v;
      const db = 20 * Math.log10(Math.max(1e-6, Math.sqrt(sum / buf.length)));
      onLevel(Math.max(0, Math.min(1, (db + 60) / 60)), db);
    }, 50);
  }

  stop(): void {
    clearInterval(this.timer);
    this.stream?.getTracks().forEach((t) => t.stop());
    void this.ctx?.close();
    this.stream = null;
    this.ctx = null;
  }
}

/** record `ms` of the controller's microphone into memory */
export async function recordClip(id: string, ms: number): Promise<Blob> {
  const stream = await navigator.mediaDevices.getUserMedia(rawMic(id));
  try {
    const rec = new MediaRecorder(stream);
    const chunks: Blob[] = [];
    rec.ondataavailable = (e) => e.data.size && chunks.push(e.data);
    const done = new Promise<void>((r) => (rec.onstop = () => r()));
    rec.start();
    await new Promise((r) => setTimeout(r, ms));
    rec.stop();
    await done;
    return new Blob(chunks, { type: rec.mimeType });
  } finally {
    stream.getTracks().forEach((t) => t.stop());
  }
}

/** sound out of the controller: its speaker (channels 1 and 2) and its haptic motors (channels 3 and 4) */
export class PadSpeaker {
  private ctx = new AudioContext();
  /** how many channels the os gives this output: 2 = stereo only, 4 = speaker plus haptics */
  channels = 2;

  async open(outputId: string): Promise<void> {
    const c = this.ctx as unknown as { setSinkId?: (id: string) => Promise<void> };
    if (!c.setSinkId) throw new Error('this webview cannot choose an audio output');
    await c.setSinkId(outputId);
    await this.ctx.resume();
    this.channels = this.ctx.destination.maxChannelCount;
  }

  /** a sine through the speaker: both sides, or only one */
  tone(freq: number, ms: number, side: 'both' | 'left' | 'right', level = 0.4): void {
    this.ctx.destination.channelCount = 2;
    const osc = this.ctx.createOscillator();
    const g = this.ctx.createGain();
    osc.frequency.value = freq;
    g.gain.value = 0;
    osc.connect(g);
    if (side === 'both') g.connect(this.ctx.destination);
    else {
      const m = this.ctx.createChannelMerger(2);
      g.connect(m, 0, side === 'left' ? 0 : 1);
      m.connect(this.ctx.destination);
    }
    this.envelope(osc, g, ms, level);
  }

  /** a low sine on the haptic channels. false when the os only gives this device two channels */
  haptic(freq: number, ms: number, level: number): boolean {
    if (this.channels < 4) return false;
    this.ctx.destination.channelCount = 4;
    this.ctx.destination.channelInterpretation = 'discrete';
    const osc = this.ctx.createOscillator();
    const g = this.ctx.createGain();
    osc.frequency.value = freq;
    g.gain.value = 0;
    osc.connect(g);
    const m = this.ctx.createChannelMerger(4);
    g.connect(m, 0, 2);
    g.connect(m, 0, 3);
    m.connect(this.ctx.destination);
    this.envelope(osc, g, ms, level);
    return true;
  }

  async play(clip: Blob): Promise<void> {
    this.ctx.destination.channelCount = 2;
    const buf = await this.ctx.decodeAudioData(await clip.arrayBuffer());
    const src = this.ctx.createBufferSource();
    src.buffer = buf;
    src.connect(this.ctx.destination);
    src.start();
  }

  close(): void {
    void this.ctx.close();
  }

  /** short fades either side so the start and end do not click */
  private envelope(osc: OscillatorNode, g: GainNode, ms: number, level: number): void {
    const t = this.ctx.currentTime;
    const end = t + ms / 1000;
    g.gain.setValueAtTime(0, t);
    g.gain.linearRampToValueAtTime(level, t + 0.03);
    g.gain.setValueAtTime(level, Math.max(t + 0.03, end - 0.05));
    g.gain.linearRampToValueAtTime(0, end);
    osc.start(t);
    osc.stop(end + 0.02);
  }
}
