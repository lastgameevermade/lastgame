// Plays the samples the game makes (48 kHz mono). Keeps at most a quarter of a
// second queued, so sound never lags far behind the picture.
class Player extends AudioWorkletProcessor {
  constructor() {
    super();
    this.queue = [];
    this.at = 0;
    this.port.onmessage = (e) => {
      this.queue.push(e.data);
      let total = this.queue.reduce((s, a) => s + a.length, 0) - this.at;
      while (total > 12000 && this.queue.length > 1) {
        total -= this.queue.shift().length - this.at;
        this.at = 0;
      }
    };
  }
  process(_, outputs) {
    const out = outputs[0][0];
    for (let i = 0; i < out.length; i++) {
      const head = this.queue[0];
      if (!head) { out[i] = 0; continue; }
      out[i] = head[this.at++];
      if (this.at >= head.length) { this.queue.shift(); this.at = 0; }
    }
    return true;
  }
}
registerProcessor('lastgame-audio', Player);
