// L-24 accessibility: Web Audio sonification of the current view-centre
// direction for screen-reader / low-vision users.
//
// The cue maps the view centre's horizontal coordinates onto a continuous
// tone while enabled:
//   - azimuth  -> stereo pan  (East = right, West = left, North/South = centre)
//   - altitude -> pitch       (horizon = low, zenith = high; 2-octave span)
// so panning the sky with the keyboard "moves" the tone, giving an audible
// sense of where the camera points without reading the screen.
//
// It is intentionally self-contained in the frontend (no renderer change) and
// opt-in (default off). The `AudioContext` is created lazily on first enable
// so we satisfy the browser autoplay/user-gesture policy.

const MIN_FREQ_HZ = 220; // A3 at the horizon (alt = -90 clamps here too)
const MAX_FREQ_HZ = 880; // A5 at the zenith
const TONE_GAIN = 0.05; // quiet drone; never startling
const RAMP_SECONDS = 0.08; // smooth parameter glides, no zipper noise

type AudioCtor = typeof AudioContext;

function audioContextCtor(): AudioCtor | null {
  if (typeof window === "undefined") return null;
  const w = window as unknown as {
    AudioContext?: AudioCtor;
    webkitAudioContext?: AudioCtor;
  };
  return w.AudioContext ?? w.webkitAudioContext ?? null;
}

/// Map altitude in degrees [-90, 90] to a frequency in [MIN_FREQ_HZ, MAX_FREQ_HZ].
function altitudeToFrequency(altitudeDeg: number): number {
  const t = (clamp(altitudeDeg, -90, 90) + 90) / 180; // 0..1
  // Exponential (musical) interpolation so equal altitude steps sound even.
  return MIN_FREQ_HZ * (MAX_FREQ_HZ / MIN_FREQ_HZ) ** t;
}

/// Map azimuth in degrees (0 = N, 90 = E) to a stereo pan in [-1, 1].
function azimuthToPan(azimuthDeg: number): number {
  return clamp(Math.sin((azimuthDeg * Math.PI) / 180), -1, 1);
}

function clamp(v: number, lo: number, hi: number): number {
  return Math.max(lo, Math.min(hi, v));
}

/// Sonifies the view-centre azimuth/altitude with a single panned oscillator.
/// `setEnabled` starts/stops the tone; `update` retargets pitch + pan.
export class AzAltSonifier {
  private ctx: AudioContext | null = null;
  private osc: OscillatorNode | null = null;
  private gain: GainNode | null = null;
  private panner: StereoPannerNode | null = null;
  private enabled = false;
  private lastAz = 0;
  private lastAlt = 0;

  /// True only when the platform exposes a usable Web Audio API.
  static isSupported(): boolean {
    return audioContextCtor() !== null;
  }

  setEnabled(enabled: boolean): void {
    if (enabled === this.enabled) return;
    this.enabled = enabled;
    if (enabled) {
      this.start();
    } else {
      this.stop();
    }
  }

  /// Retarget the tone to a new view centre. No-op while disabled.
  update(azimuthDeg: number, altitudeDeg: number): void {
    this.lastAz = azimuthDeg;
    this.lastAlt = altitudeDeg;
    if (!this.enabled || !this.ctx || !this.osc || !this.panner) return;
    const now = this.ctx.currentTime;
    this.osc.frequency.linearRampToValueAtTime(altitudeToFrequency(altitudeDeg), now + RAMP_SECONDS);
    this.panner.pan.linearRampToValueAtTime(azimuthToPan(azimuthDeg), now + RAMP_SECONDS);
  }

  private start(): void {
    const Ctor = audioContextCtor();
    if (!Ctor) return;
    if (!this.ctx) this.ctx = new Ctor();
    // A context created before a user gesture can start suspended.
    void this.ctx.resume?.();
    const ctx = this.ctx;
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    const panner = ctx.createStereoPanner();
    osc.type = "sine";
    osc.frequency.value = altitudeToFrequency(this.lastAlt);
    panner.pan.value = azimuthToPan(this.lastAz);
    // Fade in to avoid a click.
    gain.gain.value = 0;
    gain.gain.linearRampToValueAtTime(TONE_GAIN, ctx.currentTime + RAMP_SECONDS);
    osc.connect(gain).connect(panner).connect(ctx.destination);
    osc.start();
    this.osc = osc;
    this.gain = gain;
    this.panner = panner;
  }

  private stop(): void {
    if (!this.ctx || !this.osc || !this.gain) {
      this.osc = null;
      this.gain = null;
      this.panner = null;
      return;
    }
    const now = this.ctx.currentTime;
    const osc = this.osc;
    this.gain.gain.cancelScheduledValues(now);
    this.gain.gain.linearRampToValueAtTime(0, now + RAMP_SECONDS);
    // Stop slightly after the fade so we don't click.
    osc.stop(now + RAMP_SECONDS + 0.02);
    this.osc = null;
    this.gain = null;
    this.panner = null;
  }

  /// Release the AudioContext (call on unmount).
  dispose(): void {
    this.stop();
    void this.ctx?.close?.();
    this.ctx = null;
  }
}
